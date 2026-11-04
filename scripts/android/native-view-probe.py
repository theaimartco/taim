#!/usr/bin/env python3
"""Dedicated-emulator native hierarchy, input, texture pixels and lifecycle acceptance."""
import argparse
from datetime import datetime,timezone
import hashlib,json,os,re,subprocess,zipfile
from pathlib import Path
from counter import ROOT,require,run,verify_apk_package

PACKAGE='com.theaimartco.taim.nativeviewprobe'
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serial',required=True);parser.add_argument('--apk',required=True,type=Path)
    parser.add_argument('--web',action='store_true',help='Also verify the built-in WebView adapter and restrictive default policy')
    parser.add_argument('--sensors',action='store_true',help='Also verify real bounded accelerometer snapshots and listener cleanup on the dedicated emulator')
    parser.add_argument('--output',type=Path,default=ROOT/'target/android/runtime-smoke'/datetime.now(timezone.utc).strftime('native-view-%Y%m%dT%H%M%S%fZ'))
    args=parser.parse_args();sdk=Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '')
    adb=[str(require(sdk/'platform-tools/adb')),'-s',args.serial]
    def call(*values):return subprocess.check_output(adb+list(values),text=True,timeout=300).strip()
    if not any(call('shell','getprop',prop)=='1' for prop in ['ro.kernel.qemu','ro.boot.qemu']):parser.error('Use a dedicated emulator for input/lifecycle acceptance')
    if int(call('shell','getprop','ro.build.version.sdk'))<28:parser.error('Acceptance fixture requires API28+')
    verify_apk_package(sdk,args.apk,PACKAGE);out=args.output;out.mkdir(parents=True,exist_ok=False)
    classes=out/'classes';dex=out/'dex';classes.mkdir();dex.mkdir()
    tools=sdk/'build-tools'/os.environ.get('TAIM_ANDROID_BUILD_TOOLS','35.0.0');jar=require(sdk/'platforms/android-35/android.jar');source=ROOT/'scripts/android/native-view-test'
    run('javac','--release','8','-classpath',jar,'-d',classes,source/'NativeViewDriver.java')
    run(tools/'d8','--min-api','28','--lib',jar,'--output',dex,*sorted(classes.rglob('*.class')))
    unsigned=out/'unsigned.apk';aligned=out/'aligned.apk';apk=out/'native-view-driver.apk'
    run(tools/'aapt2','link','-I',jar,'--manifest',source/'AndroidManifest.xml','-o',unsigned)
    with zipfile.ZipFile(unsigned,'a',compression=zipfile.ZIP_DEFLATED) as archive:archive.write(dex/'classes.dex','classes.dex')
    run(tools/'zipalign','-f','4',unsigned,aligned)
    run(tools/'apksigner','sign','--ks',require(ROOT/'target/android/taim-development.keystore'),'--ks-key-alias','taim-development','--ks-pass','pass:android','--key-pass','pass:android','--out',apk,aligned)
    report={'status':'running','serial':args.serial,'started_utc':datetime.now(timezone.utc).isoformat(),'apk_sha256':hashlib.sha256(args.apk.read_bytes()).hexdigest(),'driver_sha256':hashlib.sha256(apk.read_bytes()).hexdigest()}
    try:
        report['device']={name:call('shell','getprop',prop) for name,prop in [('sdk','ro.build.version.sdk'),('model','ro.product.model'),('abi','ro.product.cpu.abi'),('fingerprint','ro.build.fingerprint')]};report['device']['page_size']=int(call('shell','getconf','PAGE_SIZE'))
        call('install','--no-incremental','-r',str(args.apk.resolve()));call('install','--no-incremental','-r',str(apk.resolve()));call('shell','am','force-stop',PACKAGE)
        options=['-e','web','true'] if args.web else []
        if args.sensors:options+=['-e','sensors','true']
        result=call('shell','am','instrument','-w','-r',*options,'com.theaimartco.taim.nativeviewdriver/.NativeViewDriver');(out/'instrumentation.txt').write_text(result+'\n')
        expected=34+(7 if args.web else 0)+(9 if args.sensors else 0)
        if 'INSTRUMENTATION_CODE: -1' not in result or ('checks='+str(expected)) not in result:raise RuntimeError('Native embedding integration failed; inspect instrumentation.txt')
        report.update(status='passed',checks=expected,web=args.web,sensors=args.sensors)
    except Exception as error:report.update(status='failed',error=str(error));raise
    finally:
        report['finished_utc']=datetime.now(timezone.utc).isoformat();(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
