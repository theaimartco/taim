#!/usr/bin/env python3
"""Dedicated-emulator foreground-location tests using synthetic OS mock providers.

No physical-device execution or real location collection. Temporarily enables
location and the fixture's mock-location app-op, then restores both settings.
The isolated driver removes its GPS/network test providers in finally.
"""
import argparse
from datetime import datetime,timezone
import hashlib,json,os,re,subprocess,zipfile
from pathlib import Path
from counter import ROOT,require,run,verify_apk_package

PACKAGE='com.theaimartco.taim.locationprobe'
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serial',required=True);parser.add_argument('--apk',required=True,type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'target/android/runtime-smoke'/datetime.now(timezone.utc).strftime('location-%Y%m%dT%H%M%S%fZ'))
    args=parser.parse_args();sdk=Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '')
    adb=[str(require(sdk/'platform-tools/adb')),'-s',args.serial]
    def call(*values):return subprocess.check_output(adb+list(values),text=True,timeout=300).strip()
    if not any(call('shell','getprop',prop)=='1' for prop in ['ro.kernel.qemu','ro.boot.qemu']):parser.error('Use a dedicated emulator; this fixture temporarily replaces location providers')
    if int(call('shell','getprop','ro.build.version.sdk'))<28:parser.error('Fixture requires API28+; framework API26 runtime is a separate acceptance gate')
    verify_apk_package(sdk,args.apk,PACKAGE);out=args.output;out.mkdir(parents=True,exist_ok=False)
    classes=out/'classes';dex=out/'dex';classes.mkdir();dex.mkdir()
    tools=sdk/'build-tools'/os.environ.get('TAIM_ANDROID_BUILD_TOOLS','35.0.0');jar=require(sdk/'platforms/android-35/android.jar');source=ROOT/'scripts/android/location-test'
    run('javac','--release','8','-classpath',jar,'-d',classes,source/'LocationDriver.java')
    run(tools/'d8','--min-api','28','--lib',jar,'--output',dex,*sorted(classes.rglob('*.class')))
    unsigned=out/'unsigned.apk';aligned=out/'aligned.apk';apk=out/'location-driver.apk'
    run(tools/'aapt2','link','-I',jar,'--manifest',source/'AndroidManifest.xml','-o',unsigned)
    with zipfile.ZipFile(unsigned,'a',compression=zipfile.ZIP_DEFLATED) as archive:archive.write(dex/'classes.dex','classes.dex')
    run(tools/'zipalign','-f','4',unsigned,aligned)
    run(tools/'apksigner','sign','--ks',require(ROOT/'target/android/taim-development.keystore'),'--ks-key-alias','taim-development','--ks-pass','pass:android','--key-pass','pass:android','--out',apk,aligned)
    report={'status':'running','serial':args.serial,'started_utc':datetime.now(timezone.utc).isoformat(),'apk_sha256':hashlib.sha256(args.apk.read_bytes()).hexdigest(),'driver_sha256':hashlib.sha256(apk.read_bytes()).hexdigest()}
    original_enabled=None;original_mock=None
    try:
        report['device']={name:call('shell','getprop',prop) for name,prop in [('sdk','ro.build.version.sdk'),('model','ro.product.model'),('abi','ro.product.cpu.abi'),('fingerprint','ro.build.fingerprint')]};report['device']['page_size']=int(call('shell','getconf','PAGE_SIZE'))
        call('install','--no-incremental','-r',str(args.apk.resolve()));call('install','--no-incremental','-r',str(apk.resolve()));call('shell','am','force-stop',PACKAGE)
        original_enabled=call('shell','cmd','location','is-location-enabled','--user','0')
        if original_enabled not in ('true','false'):raise RuntimeError('Cannot determine original emulator location setting')
        mock=call('shell','appops','get',PACKAGE,'android:mock_location');match=re.search(r': (allow|ignore|deny|default|foreground)\b',mock);original_mock=match.group(1) if match else 'default'
        report['original_location_enabled']=original_enabled;report['original_mock_mode']=original_mock
        for permission in ['android.permission.ACCESS_FINE_LOCATION','android.permission.ACCESS_COARSE_LOCATION']:call('shell','pm','revoke',PACKAGE,permission)
        call('shell','appops','set',PACKAGE,'android:mock_location','allow');call('shell','cmd','location','set-location-enabled','true','--user','0')
        result=call('shell','am','instrument','-w','-r','com.theaimartco.taim.locationdriver/.LocationDriver');(out/'instrumentation.txt').write_text(result+'\n')
        if 'INSTRUMENTATION_CODE: -1' not in result or 'checks=16' not in result:raise RuntimeError('Location integration failed; inspect instrumentation.txt')
        report.update(status='passed',checks=16)
    except Exception as error:report.update(status='failed',error=str(error));raise
    finally:
        try:
            if original_enabled in ('true','false'):call('shell','cmd','location','set-location-enabled',original_enabled,'--user','0')
            if original_mock is not None:call('shell','appops','set',PACKAGE,'android:mock_location',original_mock)
            if original_enabled in ('true','false') and call('shell','cmd','location','is-location-enabled','--user','0')!=original_enabled:raise RuntimeError('Original location setting was not restored')
            report['settings_restored']=True
        except Exception as cleanup:report.update(status='failed',cleanup_error=str(cleanup));raise
        finally:
            report['finished_utc']=datetime.now(timezone.utc).isoformat();(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
