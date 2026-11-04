#!/usr/bin/env python3
"""Isolated camera/media protocol verification on an explicit test device.

Installs only the dedicated Taim media fixture and its driver/responder APK.
Responders generate synthetic images/video-protocol bytes; no camera or microphone
is accessed. The final check opens the real system picker and cancels it.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import zipfile
from counter import ROOT, require, run, verify_apk_package

PACKAGE = 'com.theaimartco.taim.mediaprobe'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serial', required=True)
    parser.add_argument('--apk', required=True, type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'target/android/runtime-smoke'/datetime.now(timezone.utc).strftime('media-%Y%m%dT%H%M%S%fZ'))
    args = parser.parse_args()
    sdk = Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '')
    tools = sdk/'build-tools'/os.environ.get('TAIM_ANDROID_BUILD_TOOLS', '35.0.0')
    jar = require(sdk/'platforms/android-35/android.jar')
    verify_apk_package(sdk,args.apk,PACKAGE)
    out=args.output;out.mkdir(parents=True,exist_ok=False)
    classes=out/'classes';dex=out/'dex';classes.mkdir();dex.mkdir()
    source=ROOT/'scripts/android/media-test'
    run('javac','--release','8','-classpath',jar,'-d',classes,*sorted(source.glob('*.java')))
    run(tools/'d8','--min-api','26','--lib',jar,'--output',dex,*sorted(classes.rglob('*.class')))
    unsigned=out/'unsigned.apk';aligned=out/'aligned.apk';apk=out/'media-driver.apk'
    run(tools/'aapt2','link','-I',jar,'--manifest',source/'AndroidManifest.xml','-o',unsigned)
    with zipfile.ZipFile(unsigned,'a',compression=zipfile.ZIP_DEFLATED) as archive:
        archive.write(dex/'classes.dex','classes.dex')
    run(tools/'zipalign','-f','4',unsigned,aligned)
    run(tools/'apksigner','sign','--ks',require(ROOT/'target/android/taim-development.keystore'),'--ks-key-alias','taim-development','--ks-pass','pass:android','--key-pass','pass:android','--out',apk,aligned)
    adb=[str(require(sdk/'platform-tools/adb')),'-s',args.serial]
    def call(*values): return subprocess.check_output(adb+list(values),text=True,timeout=300).strip()
    report={'status':'running','serial':args.serial,'started_utc':datetime.now(timezone.utc).isoformat(),
            'apk_sha256':hashlib.sha256(args.apk.read_bytes()).hexdigest(),
            'driver_sha256':hashlib.sha256(apk.read_bytes()).hexdigest(),
            'limitations':['External-UID responders verify protocol/grants, not hardware or video encoding','Only real system-picker cancellation is automated; physical selection/camera/viewer matrix remains pending']}
    try:
        report['device']={name:call('shell','getprop',prop) for name,prop in [('sdk','ro.build.version.sdk'),('model','ro.product.model'),('abi','ro.product.cpu.abi'),('fingerprint','ro.build.fingerprint')]}
        report['device']['page_size']=int(call('shell','getconf','PAGE_SIZE'))
        call('install','--no-incremental','-r',str(args.apk.resolve()))
        call('install','--no-incremental','-r',str(apk.resolve()))
        call('shell','am','force-stop',PACKAGE)
        result=call('shell','am','instrument','-w','-r','com.theaimartco.taim.mediadriver/.MediaDriver')
        (out/'instrumentation.txt').write_text(result+'\n')
        if 'INSTRUMENTATION_CODE: -1' not in result or 'checks=18' not in result:
            raise RuntimeError('Media integration failed; inspect instrumentation.txt')
        report.update(status='passed',checks=18)
    except Exception as error:
        report.update(status='failed',error=str(error));raise
    finally:
        report['finished_utc']=datetime.now(timezone.utc).isoformat()
        (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))

if __name__=='__main__':main()
