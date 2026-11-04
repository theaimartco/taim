#!/usr/bin/env python3
"""Build/check/package Android reference apps using Cargo and SDK tools, no Gradle required.

Creates development APKs only. Signing material and generated files stay under target/.
"""
import argparse
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[2]
PACKAGE = "com.theaimartco.taim.counter"
APPS = {
    "native-view-probe": ("android_native_view_probe", "com.theaimartco.taim.nativeviewprobe", "com.theaimartco.taim.TaimNativeProbeActivity"),
    "work-probe": ("android_work_probe", "com.theaimartco.taim.workprobe", "com.theaimartco.taim.TaimActivity"),
    "integrations-probe": ("android_integrations_probe", "com.theaimartco.taim.integrationsprobe", "com.theaimartco.taim.TaimActivity"),
    "biometric-probe": ("android_biometric_probe", "com.theaimartco.taim.biometricprobe", "com.theaimartco.taim.TaimActivity"),
    "location-probe": ("android_location_probe", "com.theaimartco.taim.locationprobe", "com.theaimartco.taim.TaimActivity"),
    "media-probe": ("android_media_probe", "com.theaimartco.taim.mediaprobe", "com.theaimartco.taim.TaimMediaProbeActivity"),
    "platform-probe": ("android_platform_probe", "com.theaimartco.taim.platformprobe", "com.theaimartco.taim.TaimProbeActivity"),
    "counter": ("android_counter", PACKAGE, "com.theaimartco.taim.TaimActivity"),
    "gpu-probe": ("android_gpu_probe", "com.theaimartco.taim.gpuprobe", "android.app.NativeActivity"),
}
TARGETS = {"arm64-v8a": ("aarch64-linux-android", "aarch64-linux-android"),
           "x86_64": ("x86_64-linux-android", "x86_64-linux-android")}


def run(*args, env=None):
    print("+", " ".join(str(a) for a in args), flush=True)
    subprocess.run([str(a) for a in args], cwd=ROOT, env=env, check=True)


def require(path):
    if not path.exists():
        raise RuntimeError(f"Missing {path}; see docs/android/README.md")
    return path


def verify_apk_package(sdk, apk, expected):
    """Reject the wrong APK before an acceptance run can attribute stale app state to it."""
    tool = require(sdk / "build-tools" / os.environ.get("TAIM_ANDROID_BUILD_TOOLS", "35.0.0") / "aapt2")
    badging = subprocess.check_output([str(tool), "dump", "badging", str(apk)], text=True)
    match = re.search(r"^package: name='([^']+)'", badging, re.M)
    actual = match.group(1) if match else None
    if actual != expected:
        raise RuntimeError(f"Expected APK package {expected}, found {actual!r}")


def verify_library(tools, library, native_host=False, background=False):
    """Reject wrong exports, load alignment and unpackaged native dependencies."""
    report = subprocess.check_output(
        [str(require(tools / "llvm-readelf")), "-l", "-d", "--dyn-syms", str(library)], text=True)
    loads = [line.split() for line in report.splitlines() if line.strip().startswith("LOAD ")]
    if not loads or any(int(fields[-1], 16) < 16384 for fields in loads):
        raise RuntimeError("Native library has LOAD segments below 16 KiB alignment")
    symbols = ["android_main", "ANativeActivity_onCreate"]
    if native_host:
        symbols += [f"Java_com_theaimartco_taim_TaimActivity_native{name}"
                    for name in ("Insets", "Edit", "Submit", "Secure", "SemanticAction", "ServiceResponse", "PlatformEvent")]
    if background:symbols+=['Java_com_theaimartco_taim_TaimWorkService_nativeRunWork']
    for symbol in symbols:
        if not re.search(r"\bGLOBAL\s+DEFAULT\s+\d+\s+" + symbol + r"\s*$", report, re.M):
            raise RuntimeError(f"Native library does not export {symbol}")
    needed = set(re.findall(r"\(NEEDED\).*?\[([^]]+)\]", report))
    system = {"libandroid.so", "libdl.so", "liblog.so", "libm.so", "libc.so"}
    if needed - system:
        raise RuntimeError(f"Native dependencies need packaging/review: {sorted(needed - system)}")
    print("Native exports, dependencies and 16 KiB LOAD alignment verified", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "build", "install", "run", "logs"])
    parser.add_argument("--abi", choices=TARGETS, default="arm64-v8a")
    parser.add_argument("--app", choices=APPS, default="counter",
                        help="Counter, platform/media fixture or independent Vulkan buffer-integrity probe")
    parser.add_argument("--release", action="store_true", help="Optimize Rust; APK still uses a development signature")
    parser.add_argument("--serial", help="Explicit adb device serial")
    args = parser.parse_args()
    crate, package, activity = APPS[args.app]

    sdk_value = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not sdk_value:
        raise RuntimeError("Set ANDROID_HOME to your Android SDK directory")
    sdk = require(Path(sdk_value).expanduser().resolve())
    adb = [require(sdk / "platform-tools/adb")]
    if args.serial:
        adb += ["-s", args.serial]
    if args.command == "logs":
        run(*adb, "logcat", "-v", "threadtime")
        return

    ndk_value = os.environ.get("ANDROID_NDK_HOME")
    if not ndk_value:
        raise RuntimeError("Set ANDROID_NDK_HOME to an installed NDK r28 or newer")
    ndk = require(Path(ndk_value).expanduser().resolve())
    version = (ndk / "source.properties").read_text()
    revision = next(line.split("=", 1)[1].strip() for line in version.splitlines() if line.startswith("Pkg.Revision"))
    if int(revision.split(".")[0]) < 28:
        raise RuntimeError("NDK r28+ required for the initial 16 KiB page-size packaging path")
    if sys.platform != "linux":
        raise RuntimeError("This initial script supports Linux hosts; Android platform code is host independent")
    tools = require(ndk / "toolchains/llvm/prebuilt/linux-x86_64/bin")
    target, clang_target = TARGETS[args.abi]
    env = os.environ.copy()
    linker = require(tools / f"{clang_target}26-clang")
    env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(linker)
    env[f"CC_{target.replace('-', '_')}"] = str(linker)
    env[f"AR_{target.replace('-', '_')}"] = str(require(tools / "llvm-ar"))
    # Keep the ELF LOAD segments compatible with Android 16 KiB page devices.
    flags_key = f"CARGO_TARGET_{target.upper().replace('-', '_')}_RUSTFLAGS"
    env[flags_key] = env.get(flags_key, "") + " -C link-arg=-Wl,-z,max-page-size=16384"
    cargo = ["cargo", "check" if args.command == "check" else "build",
             "--locked", "-p", crate, "--target", target]
    if args.release:
        cargo += ["--release"]
    run(*cargo, env=env)
    if args.command == "check":
        return

    # Cargo may use a configured/custom target directory; query it rather than guess.
    import json
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT, env=env))
    target_dir = Path(metadata["target_directory"])
    profile = "release" if args.release else "debug"
    library = require(target_dir / target / profile / f"lib{crate}.so")
    verify_library(tools, library, native_host=args.app != "gpu-probe",background=args.app=="work-probe")
    build_tools = require(sdk / "build-tools" / os.environ.get("TAIM_ANDROID_BUILD_TOOLS", "35.0.0"))
    android_jar = require(sdk / "platforms/android-35/android.jar")
    for name in ("aapt2", "zipalign", "apksigner"):
        require(build_tools / name)
    for name in ("keytool", "java"):
        if not shutil.which(name):
            raise RuntimeError(f"Install a JDK providing {name}")
    if args.app != "gpu-probe":
        require(build_tools / "d8")
        if not shutil.which("javac"):
            raise RuntimeError("Install a JDK providing javac")
    output = target_dir / "android" / args.abi / profile
    if args.app != "counter":
        output = output / args.app
    output.mkdir(parents=True, exist_ok=True)
    unsigned = output / "counter-unsigned.apk"
    aligned = output / "counter-aligned.apk"
    apk = output / {"native-view-probe":"taim-native-view-probe.apk","counter":"taim-counter.apk","platform-probe":"taim-platform-probe.apk","media-probe":"taim-media-probe.apk","location-probe":"taim-location-probe.apk","biometric-probe":"taim-biometric-probe.apk","integrations-probe":"taim-integrations-probe.apk","work-probe":"taim-work-probe.apk","gpu-probe":"taim-gpu-probe.apk"}[args.app]
    assets = ROOT / "examples" / crate / "android/assets"
    asset_args = ["-A",assets] if assets.is_dir() else []
    run(build_tools / "aapt2", "link", "-I", android_jar,
        "--manifest", ROOT / "examples" / crate / "android/AndroidManifest.xml",
        "-o", unsigned, *asset_args)
    dex = None
    if args.app != "gpu-probe":
        classes = output / "java-classes"
        dex_dir = output / "dex"
        for directory in (classes, dex_dir):
            if directory.exists():
                shutil.rmtree(directory)
            directory.mkdir()
        run("javac", "--release", "8", "-classpath", android_jar, "-d", classes,
            *sorted((ROOT / "winit/android/com/theaimartco/taim").glob("*.java")),
            *sorted((ROOT / "examples" / crate / "android/java").rglob("*.java")))
        run(build_tools / "d8", "--min-api", "26", "--lib", android_jar,
            "--output", dex_dir, *sorted(classes.rglob("*.class")))
        dex = require(dex_dir / "classes.dex")
    with zipfile.ZipFile(unsigned, "a", compression=zipfile.ZIP_STORED) as archive:
        archive.write(library, f"lib/{args.abi}/lib{crate}.so")
        if dex is not None:
            archive.write(dex, "classes.dex", compress_type=zipfile.ZIP_DEFLATED)
    run(build_tools / "zipalign", "-f", "-P", "16", "4", unsigned, aligned)
    keystore = target_dir / "android/taim-development.keystore"
    if not keystore.exists():
        run("keytool", "-genkeypair", "-keystore", keystore, "-storepass", "android",
            "-keypass", "android", "-alias", "taim-development", "-keyalg", "RSA",
            "-keysize", "2048", "-validity", "3650", "-dname", "CN=Taim Development")
    run(build_tools / "apksigner", "sign", "--ks", keystore,
        "--ks-key-alias", "taim-development", "--ks-pass", "pass:android",
        "--key-pass", "pass:android", "--out", apk, aligned)
    run(build_tools / "apksigner", "verify", apk)
    run(build_tools / "zipalign", "-c", "-P", "16", "4", apk)
    verify_apk_package(sdk, apk, package)
    print(f"Development APK: {apk}", flush=True)
    if args.command in ("install", "run"):
        run(*adb, "install", "-r", apk)
    if args.command == "run":
        run(*adb, "shell", "am", "start", "-W", "-n", f"{package}/{activity}")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError, OSError) as error:
        print(f"Android build: {error}", file=sys.stderr)
        sys.exit(1)
