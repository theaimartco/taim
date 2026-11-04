#!/usr/bin/env python3
"""Build the native module with NDK r28+, verify ELF exports/alignment, stage both ABIs."""
import json, os, re, shutil, subprocess, sys
from pathlib import Path
from artifact_paths import cdylib_output
root=Path(__file__).resolve().parents[1]
profile,out=sys.argv[1],Path(sys.argv[2])
if profile not in ('debug','release'): raise SystemExit('Invalid Rust profile')
ndk=Path(os.environ['ANDROID_NDK_HOME'])
revision=(ndk/'source.properties').read_text()
if int(re.search(r'Pkg.Revision\s*=\s*(\d+)',revision).group(1))<28: raise SystemExit('NDK r28+ required')
host='darwin-x86_64' if sys.platform=='darwin' else 'windows-x86_64' if sys.platform=='win32' else 'linux-x86_64'
tools=ndk/'toolchains/llvm/prebuilt'/host/'bin'
for abi,target,clang in [('arm64-v8a','aarch64-linux-android','aarch64-linux-android'),('x86_64','x86_64-linux-android','x86_64-linux-android')]:
    env=os.environ.copy();compiler=tools/(clang+'26-clang'+('.cmd' if sys.platform=='win32' else ''))
    env['CARGO_TARGET_'+target.upper().replace('-','_')+'_LINKER']=str(compiler)
    env['CC_'+target.replace('-','_')]=str(compiler);env['AR_'+target.replace('-','_')]=str(tools/'llvm-ar')
    key='CARGO_TARGET_'+target.upper().replace('-','_')+'_RUSTFLAGS'
    env[key]=env.get(key,'')+' -C link-arg=-Wl,-z,max-page-size=16384'
    cmd=['cargo','build','--manifest-path',str(root/'rust/Cargo.toml'),'--target',target]
    if profile=='release':cmd+=['--release']
    if os.environ.get('TAIM_ANDROID_MEASUREMENT')=='1':cmd+=['--features','measurement']
    subprocess.run(cmd,env=env,cwd=root,check=True)
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--no-deps','--format-version','1','--manifest-path',str(root/'rust/Cargo.toml')],cwd=root,env=env))
    library=cdylib_output(metadata,root/'rust/Cargo.toml',target,profile)
    report=subprocess.check_output([str(tools/'llvm-readelf'),'-l','-d','--dyn-syms',str(library)],text=True)
    loads=[line.split() for line in report.splitlines() if line.strip().startswith('LOAD ')]
    if not loads or any(int(fields[-1],16)<16384 for fields in loads): raise SystemExit('ELF LOAD alignment below 16 KiB')
    exports=['android_main','ANativeActivity_onCreate']+['Java_com_theaimartco_taim_TaimActivity_native'+name for name in ['Insets','Edit','Submit','Secure','SemanticAction','ServiceResponse','PlatformEvent']]
    if "@BACKGROUND_WORK@"=="true":exports+=['Java_com_theaimartco_taim_TaimWorkService_nativeRunWork']
    for symbol in exports:
        if not re.search(r'\bGLOBAL\s+DEFAULT\s+\d+\s+'+symbol+r'\s*$',report,re.M): raise SystemExit('Missing native export: '+symbol)
    needed=set(re.findall(r'\(NEEDED\).*?\[([^]]+)\]',report))
    if needed-{'libandroid.so','libdl.so','liblog.so','libm.so','libc.so'}: raise SystemExit('Unpackaged native dependencies: '+str(needed))
    # Stable Android loader name; the Cargo output name stays app-specific so
    # a shared CARGO_TARGET_DIR cannot substitute another generated project.
    (out/abi).mkdir(parents=True,exist_ok=True);shutil.copy2(library,out/abi/'libtaim_app.so')
