#!/usr/bin/env python3
"""Pinned Gradle bootstrap with verified distribution; no global Gradle installation needed."""
import hashlib, os, shutil, subprocess, sys, urllib.request, zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
version='8.13';expected='20f1b1176237254a6fc204d8434196fa11a4cfb387567519c61556e8710aed78'
cache=root/'.gradle-bootstrap';binary=cache/('gradle-'+version)/'bin'/('gradle.bat' if os.name=='nt' else 'gradle')
if not binary.exists():
    cache.mkdir(exist_ok=True);archive=cache/'distribution.zip';temporary=cache/'distribution.zip.part'
    digest=hashlib.sha256()
    with urllib.request.urlopen('https://services.gradle.org/distributions/gradle-'+version+'-bin.zip',timeout=60) as source,temporary.open('wb') as output:
        while block:=source.read(1024*1024):digest.update(block);output.write(block)
    if digest.hexdigest()!=expected:temporary.unlink();raise SystemExit('Gradle distribution checksum mismatch')
    temporary.replace(archive)
    with zipfile.ZipFile(archive) as contents:
        for entry in contents.infolist():
            if not (cache/entry.filename).resolve().is_relative_to(cache.resolve()):raise SystemExit('Unsafe Gradle archive path')
        contents.extractall(cache)
    binary.chmod(0o755);archive.unlink()
env=os.environ.copy()
if not env.get('JAVA_HOME') and Path('/usr/lib/jvm/java-21-openjdk-amd64').exists():env['JAVA_HOME']='/usr/lib/jvm/java-21-openjdk-amd64'
raise SystemExit(subprocess.run([str(binary),*sys.argv[1:]],cwd=root,env=env).returncode)
