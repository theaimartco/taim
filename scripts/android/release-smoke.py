#!/usr/bin/env python3
"""Verify the generated counter starter's non-debuggable release APK on a dedicated emulator.

Uses public UI/shell actions, real pixels and OS background process death; no
run-as access, instrumentation in the release app, external messages or uploads.
"""
import argparse,hashlib,json,os,re,subprocess,time
from datetime import datetime,timezone
from pathlib import Path
import xml.etree.ElementTree as ET
from io import BytesIO
from PIL import Image
from counter import ROOT,require,verify_apk_package
from release_pixels import capture_signature,compare_restoration

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serial',required=True);parser.add_argument('--apk',required=True,type=Path);parser.add_argument('--package',required=True)
    parser.add_argument('--memory-pressure',action='store_true',help='Also request foreground OS trim-memory and verify renderer recreation pixels')
    parser.add_argument('--expected-background',help='Six-digit RGB hex of the fixture background; catches stable RGB565 palette corruption as well as cross-process changes')
    parser.add_argument('--allow-backend-rounding',action='store_true',help='Allow at most one RGB level only across a log-proven Vulkan/GLES transition; requires an exact expected background')
    parser.add_argument('--output',type=Path,default=ROOT/'target/android/runtime-smoke'/datetime.now(timezone.utc).strftime('release-%Y%m%dT%H%M%S%fZ'))
    args=parser.parse_args()
    if args.expected_background and not re.fullmatch(r'[0-9a-fA-F]{6}',args.expected_background):parser.error('Expected background must be six RGB hex digits')
    expected_background=tuple(bytes.fromhex(args.expected_background)) if args.expected_background else None
    if args.allow_backend_rounding and expected_background is None:parser.error('Backend rounding requires --expected-background to keep the solid palette exact')
    if not re.fullmatch(r'[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+',args.package):parser.error('Invalid application package')
    sdk=Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '')
    adb=[str(require(sdk/'platform-tools/adb')),'-s',args.serial]
    def call(*items):return subprocess.check_output(adb+list(items),text=True,timeout=60).strip()
    if not any(call('shell','getprop',name)=='1' for name in ['ro.kernel.qemu','ro.boot.qemu']):parser.error('Use a dedicated emulator for this generated-counter fixture')
    verify_apk_package(sdk,args.apk,args.package)
    tools=sdk/'build-tools'/os.environ.get('TAIM_ANDROID_BUILD_TOOLS','35.0.0')
    badging=subprocess.check_output([str(require(tools/'aapt2')),'dump','badging',str(args.apk)],text=True)
    if 'application-debuggable' in badging:parser.error('Release acceptance rejects debuggable APKs')
    subprocess.run([str(require(tools/'apksigner')),'verify',str(args.apk)],check=True)
    subprocess.run([str(require(tools/'zipalign')),'-c','-P','16','4',str(args.apk)],check=True)
    out=args.output;out.mkdir(parents=True,exist_ok=False)
    report={'status':'running','serial':args.serial,'package':args.package,'debuggable':False,'apk_sha256':hashlib.sha256(args.apk.read_bytes()).hexdigest(),'started_utc':datetime.now(timezone.utc).isoformat(),'checks':[],'captures':[],'capture_attempts':[],'expected_background_rgb':expected_background}
    component=args.package+'/com.theaimartco.taim.TaimActivity';tree_path='/sdcard/taim-release-audit-'+str(time.time_ns())+'.xml'
    def tree():
        call('shell','uiautomator','dump',tree_path)
        return ET.fromstring(call('shell','cat',tree_path))
    def wait_node(label):
        until=time.monotonic()+25
        while time.monotonic()<until:
            for node in tree().iter('node'):
                if node.get('package')==args.package and label in [node.get('text'),node.get('content-desc')]:return node
            time.sleep(.15)
        raise RuntimeError('Missing release fixture node: '+label)
    def bounds(node):
        values=[int(v) for v in re.findall(r'-?\d+',node.attrib['bounds'])]
        if len(values)!=4 or values[2]<=values[0] or values[3]<=values[1]:raise RuntimeError('Invalid fixture bounds')
        return tuple(values)
    def pixels(label,file):
        # Semantics can arrive before the first presented frame/inset update.
        # Re-read bounds and require two identical nonblank captures. A stable
        # wrong palette still fails; this never relaxes pixel/color limits.
        deadline=time.monotonic()+8;previous=None;attempt=0;last_error='pixels did not settle'
        while time.monotonic()<deadline:
            attempt+=1;area=bounds(wait_node(label))
            data=subprocess.check_output(adb+['exec-out','screencap','-p'],timeout=30);(out/file).write_bytes(data)
            try:
                value,signature=capture_signature(Image.open(BytesIO(data)),area,expected_background)
                report['capture_attempts'].append(dict(signature,file=file,attempt=attempt))
            except ValueError as error:
                last_error=str(error);previous=None
                (out/(Path(file).stem+'-attempt-'+str(attempt)+'.png')).write_bytes(data)
                report['capture_attempts'].append(dict(file=file,attempt=attempt,bounds=list(area),error=last_error))
                time.sleep(.1);continue
            if previous==(value,area):
                report['captures'].append(dict(signature,file=file,label=label,attempts=attempt,settled_samples=2))
                return value,area
            previous=(value,area);time.sleep(.1)
        raise RuntimeError('Release capture did not settle: '+last_error)
    def process_id():
        result=subprocess.run(adb+['shell','pidof',args.package],capture_output=True,text=True,timeout=30)
        if result.returncode not in (0,1):raise RuntimeError('Cannot inspect release process: '+result.stderr)
        return result.stdout.strip()
    def renderer_backend(pid,file):
        data=call('logcat','-d','-v','brief','--pid='+pid);(out/file).write_text(data+'\n')
        selected=re.findall(r'Selected: AdapterInfo \{[^}]*backend: (\w+),',data)
        return selected[-1] if selected else None
    def open_app():return call('shell','am','start','-W','-a','android.intent.action.MAIN','-c','android.intent.category.LAUNCHER','-f','0x10200000','-n',component)
    def increment():
        x,y,right,bottom=bounds(wait_node('Increment'));call('shell','input','tap',str((x+right)//2),str((y+bottom)//2))
    try:
        report['device']={key:call('shell','getprop',prop) for key,prop in [('sdk','ro.build.version.sdk'),('model','ro.product.model'),('abi','ro.product.cpu.abi'),('fingerprint','ro.build.fingerprint')]};report['device']['page_size']=int(call('shell','getconf','PAGE_SIZE'));report['eviction_preparation']='Settings foreground removes previous-app protection before am kill; no root, run-as or force-stop for eviction'
        call('install','--no-incremental','-r',str(args.apk.resolve()));call('shell','am','force-stop',args.package);report['launch']=open_app()
        before,_=pixels('Count: 0','initial.png');report['checks'].append('non-debuggable release launches with real nonblank counter pixels')
        pid=process_id();report['initial_pid']=pid
        increment();one,_=pixels('Count: 1','incremented.png')
        if before==one:raise RuntimeError('Release touch did not change counter pixels')
        report['checks'].append('real touch increments release counter pixels')
        for cycle in range(2):
            call('shell','input','keyevent','KEYCODE_HOME');time.sleep(.35);open_app();retained,_=pixels('Count: 1','resume-'+str(cycle)+'.png')
            if process_id()!=pid or retained!=one:raise RuntimeError('Release surface recreation lost process/state/pixels')
            report['checks'].append('release suspend/resume '+str(cycle+1)+' retains process, state and rendered pixels')
        increment();two,_=pixels('Count: 2','before-death.png');report['checks'].append('release input remains functional after surface recreation')
        if args.memory_pressure:
            report['trim_memory']=call('shell','am','send-trim-memory',args.package,'RUNNING_CRITICAL')
            time.sleep(.3);trimmed,_=pixels('Count: 2','after-trim.png')
            if process_id()!=pid or trimmed!=two:raise RuntimeError('Release memory-pressure renderer recreation changed process/state/pixels')
            report['checks'].append('OS trim-memory retains release process, counter state and exact rendered pixels')
        baseline_backend=renderer_backend(pid,'initial-renderer.log')
        call('shell','input','keyevent','KEYCODE_HOME');time.sleep(.5)
        # Android protects its previous-app slot (adj 700). Move a different app
        # through foreground so this fixture becomes normally killable cached state.
        call('shell','am','start','-W','-a','android.settings.SETTINGS');call('shell','input','keyevent','KEYCODE_HOME');time.sleep(.3)
        call('shell','am','kill',args.package)
        until=time.monotonic()+8
        while time.monotonic()<until and process_id():time.sleep(.1)
        if process_id():raise RuntimeError('OS did not kill the background release process; no force-stop/run-as substitute is used')
        open_app();report['restored_pid']=process_id();restored,_=pixels('Count: 2','restored.png')
        if report['restored_pid']==pid:raise RuntimeError('OS process death did not create a new release process')
        restored_backend=renderer_backend(report['restored_pid'],'restored-renderer.log')
        report['restoration_pixels']=compare_restoration(two,restored,baseline_backend,restored_backend,args.allow_backend_rounding)
        report['checks'].append('OS background process death restores release counter state and validated rendered pixels into a new process')
        increment();three,_=pixels('Count: 3','restored-input.png')
        if three==restored:raise RuntimeError('Release input failed after process restoration')
        report['checks'].append('release input remains functional after process restoration');report['status']='passed'
    except Exception as error:report.update(status='failed',error=str(error));raise
    finally:
        call('shell','rm','-f',tree_path);report['finished_utc']=datetime.now(timezone.utc).isoformat();(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
