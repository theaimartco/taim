#!/usr/bin/env python3
"""Verify durable JobScheduler/Rust work on an explicit test device.

Only fixture jobs are forced. The fixture process is killed (not force-stopped)
between phases, then Android starts its JobService with no Activity. No outbound
network request is made; an assigned-network socket is only bound, not connected.
"""
import argparse,hashlib,json,os,re,subprocess,time,zipfile
from datetime import datetime,timezone
from pathlib import Path
from counter import ROOT,require,run,verify_apk_package
PACKAGE='com.theaimartco.taim.workprobe';DRIVER='com.theaimartco.taim.workdriver/.WorkDriver';BASE=0x54410000
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--serial',required=True);parser.add_argument('--apk',required=True,type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'target/android/runtime-smoke'/datetime.now(timezone.utc).strftime('work-%Y%m%dT%H%M%S%fZ'));args=parser.parse_args()
    sdk=Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '');verify_apk_package(sdk,args.apk,PACKAGE)
    out=args.output;out.mkdir(parents=True,exist_ok=False);classes=out/'classes';dex=out/'dex';classes.mkdir();dex.mkdir()
    tools=sdk/'build-tools'/os.environ.get('TAIM_ANDROID_BUILD_TOOLS','35.0.0');jar=require(sdk/'platforms/android-35/android.jar');source=ROOT/'scripts/android/work-test'
    run('javac','--release','8','-classpath',jar,'-d',classes,source/'WorkDriver.java');run(tools/'d8','--min-api','26','--lib',jar,'--output',dex,*sorted(classes.rglob('*.class')))
    unsigned=out/'unsigned.apk';aligned=out/'aligned.apk';apk=out/'work-driver.apk';run(tools/'aapt2','link','-I',jar,'--manifest',source/'AndroidManifest.xml','-o',unsigned)
    with zipfile.ZipFile(unsigned,'a',compression=zipfile.ZIP_DEFLATED) as archive:archive.write(dex/'classes.dex','classes.dex')
    run(tools/'zipalign','-f','4',unsigned,aligned);run(tools/'apksigner','sign','--ks',require(ROOT/'target/android/taim-development.keystore'),'--ks-key-alias','taim-development','--ks-pass','pass:android','--key-pass','pass:android','--out',apk,aligned)
    adb=[str(require(sdk/'platform-tools/adb')),'-s',args.serial]
    def call(*values,check=True):return subprocess.run(adb+list(values),capture_output=True,text=True,timeout=300,check=check).stdout.strip()
    report={'status':'running','serial':args.serial,'started_utc':datetime.now(timezone.utc).isoformat(),'apk_sha256':hashlib.sha256(args.apk.read_bytes()).hexdigest(),'driver_sha256':hashlib.sha256(apk.read_bytes()).hexdigest()}
    try:
        report['device']={name:call('shell','getprop',prop) for name,prop in [('sdk','ro.build.version.sdk'),('model','ro.product.model'),('abi','ro.product.cpu.abi'),('fingerprint','ro.build.fingerprint')]};report['device']['page_size']=int(call('shell','getconf','PAGE_SIZE'))
        call('install','--no-incremental','-r',str(args.apk.resolve()));call('install','--no-incremental','-r',str(apk.resolve()));call('shell','am','force-stop',PACKAGE)
        suite=call('shell','am','instrument','-w','-r','-e','phase','suite',DRIVER);(out/'suite.txt').write_text(suite+'\n')
        if 'INSTRUMENTATION_CODE: -1' not in suite or 'checks=14' not in suite:raise RuntimeError('Work suite failed; inspect suite.txt')
        generation=re.search(r'INSTRUMENTATION_RESULT: generation=([a-f0-9-]{36})',suite).group(1);old_pid=int(re.search(r'INSTRUMENTATION_RESULT: suite_pid=(\d+)',suite).group(1));report['suite_pid']=old_pid;report['generation']=generation
        call('shell','am','kill',PACKAGE);time.sleep(.3);pid=call('shell','pidof',PACKAGE,check=False)
        if pid:
            if pid!=str(old_pid):raise RuntimeError('Fixture process changed before planned headless execution')
            call('shell','run-as',PACKAGE,'kill','-9',pid)
        deadline=time.monotonic()+10
        while call('shell','pidof',PACKAGE,check=False):
            if time.monotonic()>deadline:raise RuntimeError('Fixture process did not terminate')
            time.sleep(.1)
        forced=call('shell','cmd','jobscheduler','run','-f',PACKAGE,str(BASE|17));(out/'headless-force.txt').write_text(forced+'\n')
        if 'Running job' not in forced:raise RuntimeError('OS did not start the headless job')
        deadline=time.monotonic()+20;worker=None
        while time.monotonic()<deadline:
            value=call('shell','run-as',PACKAGE,'cat','files/work-fixture-result-17-'+generation+'.json',check=False)
            try:worker=json.loads(value)
            except (ValueError,TypeError):pass
            if worker:break
            time.sleep(.2)
        if not worker or not worker.get('headless') or worker.get('pid')==old_pid:raise RuntimeError('Worker did not execute headlessly in a new process')
        (out/'headless-output.json').write_text(json.dumps(worker,indent=2)+'\n')
        verify=call('shell','am','instrument','-w','-r','-e','phase','verify','-e','generation',generation,'-e','oldPid',str(old_pid),DRIVER);(out/'verify.txt').write_text(verify+'\n')
        if 'INSTRUMENTATION_CODE: -1' not in verify or 'checks=3' not in verify:raise RuntimeError('Headless ledger verification failed')
        report.update(status='passed',suite_checks=14,headless_checks=3,worker_pid=worker['pid'])
    except Exception as error:report.update(status='failed',error=str(error));raise
    finally:report['finished_utc']=datetime.now(timezone.utc).isoformat();(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
