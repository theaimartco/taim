#!/usr/bin/env python3
"""Dedicated-emulator accessibility, runtime permission dialogs and persisted denial acceptance."""
import argparse
from datetime import datetime,timezone
import hashlib,json,os,re,subprocess,zipfile
from pathlib import Path
from counter import ROOT,require,run,verify_apk_package

PACKAGE='com.theaimartco.taim.platformprobe'
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--production',action='store_true',help='Also test semantic command ordering and secure input-connection lifecycle')
    parser.add_argument('--serial',required=True);parser.add_argument('--apk',required=True,type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'target/android/runtime-smoke'/datetime.now(timezone.utc).strftime('access-%Y%m%dT%H%M%S%fZ'))
    args=parser.parse_args();sdk=Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '')
    adb=[str(require(sdk/'platform-tools/adb')),'-s',args.serial]
    def call(*values):return subprocess.check_output(adb+list(values),text=True,timeout=300).strip()
    if not any(call('shell','getprop',prop)=='1' for prop in ['ro.kernel.qemu','ro.boot.qemu']):parser.error('Use a dedicated emulator for input/lifecycle acceptance')
    if int(call('shell','getprop','ro.build.version.sdk'))<28:parser.error('Acceptance fixture requires API28+')
    verify_apk_package(sdk,args.apk,PACKAGE);out=args.output;out.mkdir(parents=True,exist_ok=False)
    classes=out/'classes';dex=out/'dex';classes.mkdir();dex.mkdir()
    tools=sdk/'build-tools'/os.environ.get('TAIM_ANDROID_BUILD_TOOLS','35.0.0');jar=require(sdk/'platforms/android-35/android.jar');source=ROOT/'scripts/android/access-test'
    run('javac','--release','8','-classpath',jar,'-d',classes,source/'AccessDriver.java')
    run(tools/'d8','--min-api','28','--lib',jar,'--output',dex,*sorted(classes.rglob('*.class')))
    unsigned=out/'unsigned.apk';aligned=out/'aligned.apk';apk=out/'access-driver.apk'
    run(tools/'aapt2','link','-I',jar,'--manifest',source/'AndroidManifest.xml','-o',unsigned)
    with zipfile.ZipFile(unsigned,'a',compression=zipfile.ZIP_DEFLATED) as archive:archive.write(dex/'classes.dex','classes.dex')
    run(tools/'zipalign','-f','4',unsigned,aligned)
    run(tools/'apksigner','sign','--ks',require(ROOT/'target/android/taim-development.keystore'),'--ks-key-alias','taim-development','--ks-pass','pass:android','--key-pass','pass:android','--out',apk,aligned)
    report={'status':'running','production':args.production,'serial':args.serial,'started_utc':datetime.now(timezone.utc).isoformat(),'apk_sha256':hashlib.sha256(args.apk.read_bytes()).hexdigest(),'driver_sha256':hashlib.sha256(apk.read_bytes()).hexdigest()}
    try:
        report['device']={name:call('shell','getprop',prop) for name,prop in [('sdk','ro.build.version.sdk'),('model','ro.product.model'),('abi','ro.product.cpu.abi'),('fingerprint','ro.build.fingerprint')]};report['device']['page_size']=int(call('shell','getconf','PAGE_SIZE'))
        call('install','--no-incremental','-r',str(args.apk.resolve()));call('install','--no-incremental','-r',str(apk.resolve()));call('shell','am','force-stop',PACKAGE);call('shell','input','keyevent','KEYCODE_BACK')
        call('shell','pm','revoke',PACKAGE,'android.permission.CAMERA')
        call('shell','pm','clear-permission-flags',PACKAGE,'android.permission.CAMERA','user-set','user-fixed')
        options=['-e','production','true'] if args.production else []
        checks=33 if args.production else 27
        result=call('shell','am','instrument','-w','-r',*options,'com.theaimartco.taim.accessdriver/.AccessDriver');(out/'instrumentation.txt').write_text(result+'\n')
        if 'INSTRUMENTATION_CODE: -1' not in result or ('checks='+str(checks)) not in result:raise RuntimeError('Accessibility/permission integration failed; inspect instrumentation.txt')
        history=call('shell','am','instrument','-w','-r','-e','mode','history','com.theaimartco.taim.accessdriver/.AccessDriver');(out/'history.txt').write_text(history+'\n')
        if 'INSTRUMENTATION_CODE: -1' not in history or 'checks=2' not in history:raise RuntimeError('Persisted permission history/grant acceptance failed')
        call('shell','pm','revoke',PACKAGE,'android.permission.CAMERA')
        call('shell','pm','clear-permission-flags',PACKAGE,'android.permission.CAMERA','user-set','user-fixed')
        revoked=call('shell','am','instrument','-w','-r','-e','mode','revoked','com.theaimartco.taim.accessdriver/.AccessDriver');(out/'revoked.txt').write_text(revoked+'\n')
        if 'INSTRUMENTATION_CODE: -1' not in revoked or 'checks=1' not in revoked:raise RuntimeError('Permission revocation acceptance failed')
        report.update(status='passed',checks=checks,history_checks=2,revocation_checks=1)
    except Exception as error:report.update(status='failed',error=str(error));raise
    finally:
        report['finished_utc']=datetime.now(timezone.utc).isoformat();(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
