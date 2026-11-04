#!/usr/bin/env python3
"""Run the isolated Android platform fixture using a separate instrumentation APK.

This explicit test-device workload writes fixture clipboard data and its own
app-private secure-store key. It requests only the already-granted INTERNET
permission, exercises accessibility/autofill/back/memory handling, and does not
open external URLs, send messages, or prompt for camera/microphone access.
With --notifications it also changes only the fixture's POST_NOTIFICATIONS
permission, posts/updates two local fixture IDs, verifies taps and removes them.
"""
import argparse,hashlib,json,os,subprocess,zipfile
from datetime import datetime,timezone
from pathlib import Path
from counter import ROOT,require,run,verify_apk_package
PACKAGE='com.theaimartco.taim.platformprobe'
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--serial',required=True);parser.add_argument('--apk',type=Path,required=True)
    parser.add_argument('--notifications',action='store_true',help='Also exercise fixture-only denied/granted local notifications, taps and cleanup on API 33+')
    parser.add_argument('--output',type=Path,default=ROOT/'target/android/runtime-smoke'/datetime.now(timezone.utc).strftime('platform-%Y%m%dT%H%M%S%fZ'));args=parser.parse_args()
    sdk=Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '');tools=sdk/'build-tools'/os.environ.get('TAIM_ANDROID_BUILD_TOOLS','35.0.0');jar=require(sdk/'platforms/android-35/android.jar')
    verify_apk_package(sdk,args.apk,PACKAGE);out=args.output;out.mkdir(parents=True,exist_ok=False);classes=out/'classes';dex=out/'dex';classes.mkdir();dex.mkdir()
    source=ROOT/'scripts/android/platform-test';unsigned=out/'unsigned.apk';aligned=out/'aligned.apk';apk=out/'platform-driver.apk'
    run('javac','--release','8','-classpath',jar,'-d',classes,source/'PlatformDriver.java');run(tools/'d8','--min-api','26','--lib',jar,'--output',dex,*sorted(classes.rglob('*.class')))
    run(tools/'aapt2','link','-I',jar,'--manifest',source/'AndroidManifest.xml','-o',unsigned)
    with zipfile.ZipFile(unsigned,'a',compression=zipfile.ZIP_DEFLATED) as archive:archive.write(dex/'classes.dex','classes.dex')
    run(tools/'zipalign','-f','4',unsigned,aligned);run(tools/'apksigner','sign','--ks',require(ROOT/'target/android/taim-development.keystore'),'--ks-key-alias','taim-development','--ks-pass','pass:android','--key-pass','pass:android','--out',apk,aligned)
    adb=[str(require(sdk/'platform-tools/adb')),'-s',args.serial]
    def call(*values):return subprocess.check_output(adb+list(values),text=True,timeout=240)
    report={'status':'running','serial':args.serial,'notifications':args.notifications,'started_utc':datetime.now(timezone.utc).isoformat(),'apk_sha256':hashlib.sha256(args.apk.read_bytes()).hexdigest(),'driver_sha256':hashlib.sha256(apk.read_bytes()).hexdigest()}
    try:
        report['device']={name:call('shell','getprop',prop).strip() for name,prop in [('sdk','ro.build.version.sdk'),('model','ro.product.model'),('abi','ro.product.cpu.abi'),('fingerprint','ro.build.fingerprint')]}
        report['device']['page_size']=int(call('shell','getconf','PAGE_SIZE').strip())
        call('install','--no-incremental','-r',str(args.apk));call('install','--no-incremental','-r',str(apk));call('shell','am','force-stop',PACKAGE)
        if args.notifications:
            if int(call('shell','getprop','ro.build.version.sdk').strip())<33:raise RuntimeError('Notification permission fixture requires API 33+')
            call('shell','pm','revoke',PACKAGE,'android.permission.POST_NOTIFICATIONS')
        options=['-e','notifications','true'] if args.notifications else []
        result=call('shell','am','instrument','-w','-r',*options,'com.theaimartco.taim.platformdriver/.PlatformDriver');(out/'instrumentation.txt').write_text(result)
        checks=14 if args.notifications else 8
        if 'INSTRUMENTATION_CODE: -1' not in result or ('checks='+str(checks)) not in result:raise RuntimeError('Native platform instrumentation failed; inspect instrumentation.txt')
        if args.notifications:
            call('shell','am','force-stop',PACKAGE)
            cold=call('shell','am','instrument','-w','-r','-e','cold','true','com.theaimartco.taim.platformdriver/.PlatformDriver');(out/'cold-intent.txt').write_text(cold)
            if 'INSTRUMENTATION_CODE: -1' not in cold or 'checks=1' not in cold:raise RuntimeError('Notification cold-intent replay failed')
        report.update(status='passed',checks=checks,cold_intent_checks=1 if args.notifications else 0)
    except Exception as error:report.update(status='failed',error=str(error));raise
    finally:
        report['finished_utc']=datetime.now(timezone.utc).isoformat()
        (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
