#!/usr/bin/env python3
"""Paired ABBA Android counter/startup/memory observations; no implicit winner or fabricated latency.

Only installs/force-stops explicitly provided packages on an explicit test device.
Raw meminfo/gfxinfo and optional Perfetto traces are retained. Native renderer
frames are not inferred from HWUI gfxinfo; launch times are ActivityManager metadata.
"""
import argparse, hashlib, io, json, os, re, statistics, subprocess, time
from datetime import datetime, timezone
from pathlib import Path
from PIL import Image, ImageChops
from counter import require, verify_apk_package

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serial',required=True)
    for app in ('taim','flutter'):
        parser.add_argument('--'+app+'-apk',type=Path,required=True)
        parser.add_argument('--'+app+'-package',required=True)
        parser.add_argument('--'+app+'-activity',required=True)
        parser.add_argument('--'+app+'-tap',type=int,nargs=2,required=True)
        parser.add_argument('--'+app+'-region',type=int,nargs=4,required=True,metavar=('X','Y','WIDTH','HEIGHT'))
    parser.add_argument('--taim-ready-log',default='TAIM_ANDROID_PRESENT')
    parser.add_argument('--flutter-ready-log',default='TAIM_FLUTTER_BASELINE_READY')
    parser.add_argument('--trials',type=int,default=8)
    parser.add_argument('--allow-debug',action='store_true',help='Diagnostic runs only; never a release comparison')
    parser.add_argument('--trace',action='store_true',help='Collect a separate 5-second system Perfetto workload per app')
    parser.add_argument('--output',type=Path,default=Path('target/android/comparison')/datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%fZ'))
    args=parser.parse_args()
    if args.trials<4 or args.trials%4:parser.error('Use a positive multiple of four trials for ABBA pairing')
    sdk=Path(os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT') or '')
    adb=[str(require(sdk/'platform-tools/adb')),'-s',args.serial]
    def call(*values,binary=False):
        result=subprocess.run(adb+list(values),capture_output=True,text=not binary,timeout=45,check=True)
        return result.stdout if binary else result.stdout.strip()
    args.output.mkdir(parents=True,exist_ok=False)
    report={'status':'running','serial':args.serial,'diagnostic_debug':args.allow_debug,'metrics_definition':{'launch_total_ms':'ActivityManager launch metadata; not useful-frame or photon latency','stable_counter_upper_bound_ms':'ADB launch through stable counter screenshot; includes command/poll/capture overhead','touch_pixel_upper_bound_ms':'ADB touch through stable changed counter pixels; includes command/poll/capture overhead','pss_kib':'dumpsys meminfo TOTAL PSS; not GPU-allocation accounting'},'runs':[],'artifacts':{}}
    apps={}
    def crop(app):
        raw=call('exec-out','screencap','-p',binary=True)
        image=Image.open(io.BytesIO(raw)).convert('RGB');x,y,w,h=app['region']
        if min(x,y)<0 or min(w,h)<=0 or x+w>image.width or y+h>image.height:raise RuntimeError('Counter crop is outside screenshot')
        return image.crop((x,y,x+w,y+h)),raw
    def settled(app,previous=None):
        last=None;deadline=time.monotonic()+25
        while time.monotonic()<deadline:
            image,raw=crop(app);low,high=image.convert('L').getextrema()
            changed=previous is None or ImageChops.difference(image,previous).getbbox() is not None
            if high-low>20 and changed and last is not None and ImageChops.difference(image,last).getbbox() is None:return image,raw
            last=image;time.sleep(.1)
        raise RuntimeError('Counter pixels did not settle/change')
    try:
        report['device']=call('shell','getprop')
        report['page_size']=call('shell','getconf','PAGE_SIZE')
        report['host']={'adb':call('version')}
        for name in ('taim','flutter'):
            app={'apk':getattr(args,name+'_apk').resolve(),'package':getattr(args,name+'_package'),'activity':getattr(args,name+'_activity'),'tap':getattr(args,name+'_tap'),'region':getattr(args,name+'_region'),'ready':getattr(args,name+'_ready_log')}
            if not re.fullmatch(r'[A-Za-z0-9_.]+',app['package']) or not re.fullmatch(r'[A-Za-z0-9_.$]+',app['activity']):parser.error('Invalid package/activity')
            verify_apk_package(sdk,app['apk'],app['package'])
            tool=sdk/'build-tools'/os.environ.get('TAIM_ANDROID_BUILD_TOOLS','35.0.0')/'aapt2'
            badging=subprocess.check_output([str(tool),'dump','badging',str(app['apk'])],text=True)
            if 'application-debuggable' in badging and not args.allow_debug:raise RuntimeError('Release comparison refuses debuggable APK: '+name)
            report['artifacts'][name]={'sha256':hashlib.sha256(app['apk'].read_bytes()).hexdigest(),'bytes':app['apk'].stat().st_size,'badging':badging}
            call('install','--no-incremental','-r',str(app['apk']));apps[name]=app
        # ABBA ordering balances order effects; raw runs retain cache/system variance.
        order=(['taim','flutter','flutter','taim']*(args.trials//4))
        for trial,name in enumerate(order):
            app=apps[name];call('shell','am','force-stop',app['package'])
            started=time.monotonic();launch=call('shell','am','start','-W','-n',app['package']+'/'+app['activity'])
            if not re.search(r'Status:\s*ok',launch):raise RuntimeError('Activity launch failed: '+launch)
            deadline=time.monotonic()+25
            while True:
                pid=call('shell','pidof',app['package'])
                if pid and app['ready'] in call('logcat','-d','--pid='+pid):break
                if time.monotonic()>deadline:raise RuntimeError('Readiness marker missing: '+name)
                time.sleep(.1)
            before,raw=settled(app);first=(time.monotonic()-started)*1000
            prefix=f'{trial:03d}-{name}';(args.output/(prefix+'-initial.png')).write_bytes(raw)
            started=time.monotonic();call('shell','input','tap',*map(str,app['tap']));_,raw=settled(app,before);touch=(time.monotonic()-started)*1000
            (args.output/(prefix+'-touch.png')).write_bytes(raw)
            memory=call('shell','dumpsys','meminfo',app['package']);(args.output/(prefix+'-meminfo.txt')).write_text(memory)
            (args.output/(prefix+'-gfxinfo.txt')).write_text(call('shell','dumpsys','gfxinfo',app['package'],'framestats'))
            total=re.search(r'TotalTime:\s*(\d+)',launch);pss=re.search(r'TOTAL PSS:\s*(\d+)',memory)
            report['runs'].append({'app':name,'trial':trial,'launch_total_ms':int(total.group(1)) if total else None,'stable_counter_upper_bound_ms':first,'touch_pixel_upper_bound_ms':touch,'pss_kib':int(pss.group(1)) if pss else None,'launch':launch})
        if args.trace:
            for name,app in apps.items():
                remote='/data/local/tmp/taim-compare-'+name+'-'+str(time.time_ns())+'.perfetto-trace'
                call('shell','am','start','-W','-n',app['package']+'/'+app['activity'])
                call('shell','perfetto','--background','-o',remote,'-t','5s','sched','freq','idle','am','wm','gfx','view')
                started=time.monotonic()
                while time.monotonic()-started<5:call('shell','input','tap',*map(str,app['tap']));time.sleep(.1)
                time.sleep(.3);call('pull',remote,str(args.output/(name+'.perfetto-trace')));call('shell','rm',remote)
        report['medians']={name:{metric:statistics.median([run[metric] for run in report['runs'] if run['app']==name and run[metric] is not None]) for metric in ('stable_counter_upper_bound_ms','touch_pixel_upper_bound_ms','pss_kib') if any(run[metric] is not None for run in report['runs'] if run['app']==name)} for name in apps}
        report['status']='passed'
    except Exception as error:report.update(status='failed',error=str(error));raise
    finally:(args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(args.output/'report.json')
if __name__=='__main__':main()
