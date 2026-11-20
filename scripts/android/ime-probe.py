#!/usr/bin/env python3
"""Build/run the test-only IME against an already installed counter on a test device.

Temporarily changes the selected IME, restoring it in finally. Never deploy the
IME driver with a production app. Coordinate arguments are screenshot pixels.
"""
from __future__ import annotations
import argparse
import base64
from datetime import datetime, timezone
import json
import hashlib
import os
from pathlib import Path
import subprocess
import time
import zipfile
from counter import ROOT, PACKAGE as COUNTER_PACKAGE, APPS, require, run, verify_apk_package

PACKAGE = 'com.theaimartco.taim.imetest'
COMPONENT = PACKAGE + '/.TestIme'


def build(sdk: Path) -> Path:
    tools = require(sdk / 'build-tools' / os.environ.get('TAIM_ANDROID_BUILD_TOOLS', '35.0.0'))
    jar = require(sdk / 'platforms/android-35/android.jar')
    source = ROOT / 'scripts/android/ime-test'
    output = ROOT / 'target/android/ime-test'
    output.mkdir(parents=True, exist_ok=True)
    classes, dex = output / 'classes', output / 'dex'
    classes.mkdir(exist_ok=True)
    dex.mkdir(exist_ok=True)
    resources = output / 'resources.zip'
    unsigned, aligned, apk = (output / name for name in ('unsigned.apk', 'aligned.apk', 'test-ime.apk'))
    run('javac', '--release', '8', '-classpath', jar, '-d', classes, source / 'TestIme.java')
    run(tools / 'd8', '--min-api', '26', '--lib', jar, '--output', dex, *sorted(classes.rglob('*.class')))
    run(tools / 'aapt2', 'compile', '--dir', source / 'res', '-o', resources)
    run(tools / 'aapt2', 'link', '-I', jar, '--manifest', source / 'AndroidManifest.xml', '-o', unsigned, resources)
    with zipfile.ZipFile(unsigned, 'a', compression=zipfile.ZIP_DEFLATED) as archive:
        archive.write(dex / 'classes.dex', 'classes.dex')
    run(tools / 'zipalign', '-f', '4', unsigned, aligned)
    run(tools / 'apksigner', 'sign', '--ks', require(ROOT / 'target/android/taim-development.keystore'),
        '--ks-key-alias', 'taim-development', '--ks-pass', 'pass:android', '--key-pass', 'pass:android', '--out', apk, aligned)
    verify_apk_package(sdk, apk, PACKAGE)
    return apk


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['build', 'run'])
    parser.add_argument('--serial')
    parser.add_argument('--tap', type=int, nargs=2, help='Normal text field, initially empty')
    parser.add_argument('--secure-tap', type=int, nargs=2, help='Password field; coordinates while IME is shown')
    parser.add_argument('--multiline-tap', type=int, nargs=2, help='Multiline editor while test IME is shown')
    parser.add_argument('--composition-cycles', type=int, default=10, help='Repeated queued commit/preedit/finish regression cycles')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/android/runtime-smoke' / datetime.now(timezone.utc).strftime('ime-%Y%m%dT%H%M%S%fZ'))
    args = parser.parse_args()
    if not 1 <= args.composition_cycles <= 100:
        parser.error('--composition-cycles must be between 1 and 100')
    sdk = Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '')
    apk = build(sdk)
    if args.command == 'build':
        print(apk)
        return
    if not args.serial or not args.tap:
        parser.error('run requires --serial and --tap')
    adb = [str(require(sdk / 'platform-tools/adb')), '-s', args.serial]
    def call(*values):
        return subprocess.check_output(adb + list(values), text=True, timeout=30).strip()
    def command(operation, text='', right='', before=0, after=0):
        encode = lambda value: base64.b64encode(value.encode()).decode()
        # Absent extras represent empty strings without shell-dependent empty argv.
        values = ['shell', 'am', 'broadcast', '-a', PACKAGE + '.COMMAND', '-p', PACKAGE,
                  '--es', 'operation', operation, '--ei', 'before', str(before), '--ei', 'after', str(after)]
        for key, value in [('text', text), ('right', right)]:
            if value:
                values += ['--es', key, encode(value)]
        result = call(*values)
        if 'result=-1' not in result or 'data="ok"' not in result:
            raise RuntimeError(f'InputConnection operation {operation} failed: {result}')
        report['checks'].append(operation)
    def image(name):
        data = subprocess.check_output(adb + ['exec-out', 'screencap', '-p'], timeout=30)
        (args.output / (name + '.png')).write_bytes(data)
    args.output.mkdir(parents=True, exist_ok=True)
    if any(args.output.iterdir()):
        parser.error('output directory must be empty')
    previous = call('shell', 'settings', 'get', 'secure', 'default_input_method')
    enabled = [entry.split(';', 1)[0] for entry in call('shell', 'settings', 'get', 'secure', 'enabled_input_methods').split(':')]
    report = {'status': 'running', 'serial': args.serial, 'test_ime_sha256': hashlib.sha256(apk.read_bytes()).hexdigest(), 'checks': []}
    try:
        call('install', '--no-incremental', '-r', str(apk))
        deadline = time.monotonic() + 15
        while COMPONENT not in call('shell', 'ime', 'list', '-a', '-s').splitlines():
            if time.monotonic() >= deadline:
                raise RuntimeError('Installed test IME was not discovered')
            time.sleep(.2)
        call('shell', 'ime', 'enable', COMPONENT)
        call('shell', 'ime', 'set', COMPONENT)
        installed = call('shell', 'pm', 'path', COUNTER_PACKAGE).splitlines()
        if len(installed) != 1 or not installed[0].startswith('package:/data/app/'):
            raise RuntimeError('Expected one installed counter APK')
        installed_bytes = subprocess.check_output(adb + ['exec-out', 'cat', installed[0][8:]], timeout=30)
        report['counter_apk_sha256'] = hashlib.sha256(installed_bytes).hexdigest()
        report['android_fingerprint'] = call('shell', 'getprop', 'ro.build.fingerprint')
        report['composition_cycles'] = args.composition_cycles
        call('shell', 'am', 'force-stop', COUNTER_PACKAGE)
        call('shell', 'am', 'start', '-W', '-n', COUNTER_PACKAGE + '/' + APPS['counter'][2])
        deadline = time.monotonic() + 25
        while True:
            pid = call('shell', 'pidof', COUNTER_PACKAGE)
            if 'TAIM_ANDROID_PRESENT' in call('logcat', '-d', '--pid=' + pid):
                break
            if time.monotonic() > deadline:
                raise RuntimeError('Counter did not present')
            time.sleep(.2)
        call('shell', 'input', 'tap', *map(str, args.tap))
        time.sleep(1)
        command('commit', 'नमस्ते🌍')
        command('assert', 'नमस्ते🌍')
        time.sleep(.3)
        image('unicode')
        command('delete-codepoints', before=1)
        command('assert', 'नमस्ते')
        command('select', before=0, after=6)
        command('batch', 'क')
        command('finish')
        command('assert-all', 'aक')
        command('select-all')
        command('commit', 'مرحبا')
        command('compose', '日本')
        time.sleep(.3)
        image('composition')
        command('finish')
        command('assert', 'مرحبا日本')
        # A queued app snapshot must not overwrite newer native composition.
        # Allow a frame between compose and finish, then verify committed text.
        for _ in range(args.composition_cycles):
            command('select-all')
            command('commit', 'مرحبا')
            command('compose', '日本')
            time.sleep(.1)
            command('finish')
            time.sleep(.1)
            command('assert', 'مرحبا日本')
        command('save')
        if args.secure_tap:
            call('shell', 'input', 'tap', *map(str, args.secure_tap))
            time.sleep(.5)
            command('stale', 'SHOULD_NOT_APPEAR')
            command('commit', 'private-fixture')
            command('assert')
            command('compose', '日本')
            command('finish')
            command('delete-codepoints', before=1)
            command('assert')
            time.sleep(.3)
            image('secure')
            call('shell', 'input', 'tap', *map(str, args.tap))
            time.sleep(.5)
            command('assert-all', 'مرحبا日本')
        if args.multiline_tap:
            call('shell', 'input', 'tap', *map(str, args.multiline_tap))
            time.sleep(.5)
            command('select-all')
            command('commit', 'line one\nदूसरी 🌍\nمرحبا')
            command('assert-all', 'line one\nदूसरी 🌍\nمرحبا')
            command('delete-codepoints', before=1)
            command('compose', '日本')
            command('finish')
            command('assert-all', 'line one\nदूसरी 🌍\nمرحب日本')
            time.sleep(.3)
            image('multiline')
        report['status'] = 'passed'
    except Exception as error:
        report.update(status='failed', error=str(error))
        raise
    finally:
        try:
            if previous and previous != 'null':
                call('shell', 'ime', 'set', previous)
            if COMPONENT not in enabled:
                call('shell', 'ime', 'disable', COMPONENT)
        finally:
            (args.output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
