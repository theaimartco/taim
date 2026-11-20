#!/usr/bin/env python3
"""Generate a standalone API 36 Android Gradle/Rust project, including release AAB signing."""
import argparse, hashlib, json, re, shutil
from pathlib import Path
from xml.sax.saxutils import escape
ROOT=Path(__file__).resolve().parents[2]
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination',type=Path)
    parser.add_argument('--package',required=True)
    parser.add_argument('--deep-link-scheme')
    parser.add_argument('--permission',action='append',default=[],help='Explicit Android manifest permission; repeat for additional capabilities, no automatic runtime prompt')
    parser.add_argument('--notifications',action='store_true',help='Declare POST_NOTIFICATIONS; the app still requests permission explicitly')
    parser.add_argument('--internet',action='store_true',help='Declare INTERNET for explicit network/WebView use; inline HTML needs no network permission')
    parser.add_argument('--media',action='store_true',help='Register the private grant-only capture provider; no broad storage/camera permissions')
    parser.add_argument('--location',nargs='?',const='approximate',choices=['approximate','precise'],help='Opt into foreground coarse, or coarse+fine location permissions; never background location')
    parser.add_argument('--background-work',action='store_true',help='Register the system-bound persistent JobService and an activity-independent Rust worker entry')
    parser.add_argument('--biometric',action='store_true',help='Declare USE_BIOMETRIC for Services::authenticate')
    parser.add_argument('--bluetooth',action='store_true',help='Declare BLE central permissions (BLUETOOTH_SCAN neverForLocation/CONNECT on 12+, legacy BLUETOOTH/ADMIN/FINE_LOCATION up to 11)')
    parser.add_argument('--nfc',action='store_true',help='Declare NFC and an optional android.hardware.nfc feature')
    parser.add_argument('--geofencing',action='store_true',help='Register TaimGeofenceReceiver and declare fine + background location')
    parser.add_argument('--foreground-service',action='append',default=[],choices=['dataSync','mediaPlayback','location','connectedDevice','microphone','camera','health','remoteMessaging','shortService','specialUse'],help='Register TaimForegroundService with this type (repeatable) and its FOREGROUND_SERVICE_* permission')
    parser.add_argument('--fcm',type=Path,metavar='GOOGLE_SERVICES_JSON',help='Firebase Cloud Messaging extension; copies this google-services.json into the app module')
    parser.add_argument('--maps',action='store_true',help='Google Maps extension; the API key is read from TAIM_MAPS_API_KEY at build time')
    parser.add_argument('--billing',action='store_true',help='Google Play Billing extension')
    args=parser.parse_args()
    if not re.fullmatch(r'[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+',args.package):parser.error('Use a valid lowercase application package')
    if args.deep_link_scheme and not re.fullmatch(r'[a-z][a-z0-9+.-]*',args.deep_link_scheme):parser.error('Invalid URI scheme')
    if any(not re.fullmatch(r'android\.permission\.[A-Za-z0-9_.]+',name) for name in args.permission):parser.error('Use complete android.permission names')
    if args.fcm and not args.fcm.is_file():parser.error('--fcm needs an existing google-services.json')
    output=args.destination.expanduser().resolve()
    if output.exists() and any(output.iterdir()):parser.error('Destination must be empty')
    output.mkdir(parents=True,exist_ok=True)
    for source in (ROOT/'scripts/android/templates').rglob('*'):
        # Skip caches a local Gradle or Python run leaves in the templates.
        local={'__pycache__','.gradle','.cxx','build','.kotlin'}
        if not source.is_file() or local.intersection(source.relative_to(ROOT/'scripts/android/templates').parts) or source.suffix=='.pyc':continue
        relative=source.relative_to(ROOT/'scripts/android/templates')
        target=output/relative
        if source.name in ('build-rust.py','gradle-bootstrap.py','artifact_paths.py'):target=output/'tools'/source.name
        target.parent.mkdir(parents=True,exist_ok=True)
        deep='' if not args.deep_link_scheme else '<intent-filter><action android:name="android.intent.action.VIEW" /><category android:name="android.intent.category.DEFAULT" /><category android:name="android.intent.category.BROWSABLE" /><data android:scheme="'+escape(args.deep_link_scheme)+'" /></intent-filter>'
        permission_names=list(args.permission)
        if args.internet:permission_names.append('android.permission.INTERNET')
        if args.notifications:permission_names.append('android.permission.POST_NOTIFICATIONS')
        if args.location:permission_names.append('android.permission.ACCESS_COARSE_LOCATION')
        if args.location=='precise':permission_names.append('android.permission.ACCESS_FINE_LOCATION')
        if args.background_work:permission_names.extend(['android.permission.RECEIVE_BOOT_COMPLETED','android.permission.ACCESS_NETWORK_STATE'])
        if args.biometric:permission_names.append('android.permission.USE_BIOMETRIC')
        if args.nfc:permission_names.append('android.permission.NFC')
        if args.geofencing:permission_names.extend(['android.permission.ACCESS_COARSE_LOCATION','android.permission.ACCESS_FINE_LOCATION','android.permission.ACCESS_BACKGROUND_LOCATION'])
        if args.foreground_service:permission_names.append('android.permission.FOREGROUND_SERVICE')
        fgs_permission={'dataSync':'DATA_SYNC','mediaPlayback':'MEDIA_PLAYBACK','location':'LOCATION','connectedDevice':'CONNECTED_DEVICE','microphone':'MICROPHONE','camera':'CAMERA','health':'HEALTH','remoteMessaging':'REMOTE_MESSAGING','shortService':None,'specialUse':'SPECIAL_USE'}
        for kind in args.foreground_service:
            if fgs_permission[kind]:permission_names.append('android.permission.FOREGROUND_SERVICE_'+fgs_permission[kind])
        if args.fcm:permission_names.extend(['android.permission.INTERNET','android.permission.POST_NOTIFICATIONS'])
        if args.maps or args.billing:permission_names.append('android.permission.INTERNET')
        permissions=''.join('<uses-permission android:name="'+escape(name)+'" />' for name in sorted(set(permission_names)))
        if args.bluetooth:
            # neverForLocation: scan results are not used to derive location (Play policy). Remove it if they are.
            permissions+='<uses-permission android:name="android.permission.BLUETOOTH_SCAN" android:usesPermissionFlags="neverForLocation" /><uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />'
            permissions+='<uses-permission android:name="android.permission.BLUETOOTH" android:maxSdkVersion="30" /><uses-permission android:name="android.permission.BLUETOOTH_ADMIN" android:maxSdkVersion="30" />'
            if 'android.permission.ACCESS_FINE_LOCATION' not in permission_names:permissions+='<uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" android:maxSdkVersion="30" />'
            permissions+='<uses-feature android:name="android.hardware.bluetooth_le" android:required="false" />'
        if args.nfc:permissions+='<uses-feature android:name="android.hardware.nfc" android:required="false" />'
        extension_classes=[];extensions=''
        if args.geofencing:extensions+='<receiver android:name="com.theaimartco.taim.TaimGeofenceReceiver" android:exported="false" />'
        if args.foreground_service:extensions+='<service android:name="com.theaimartco.taim.TaimForegroundService" android:exported="false" android:foregroundServiceType="'+'|'.join(dict.fromkeys(args.foreground_service))+'" />'
        if args.fcm:
            extension_classes.append('com.theaimartco.taim.TaimFcm')
            extensions+='<service android:name="com.theaimartco.taim.TaimFcmService" android:exported="false"><intent-filter><action android:name="com.google.firebase.MESSAGING_EVENT" /></intent-filter></service>'
        if args.maps:
            extension_classes.append('com.theaimartco.taim.TaimMaps')
            extensions+='<meta-data android:name="com.google.android.geo.API_KEY" android:value="${taimMapsApiKey}" />'
        if args.billing:extension_classes.append('com.theaimartco.taim.TaimBilling')
        if extension_classes:extensions+='<meta-data android:name="com.theaimartco.taim.extensions" android:value="'+','.join(extension_classes)+'" />'
        dependencies=[]
        if args.fcm:dependencies+=['    implementation(platform("com.google.firebase:firebase-bom:35.1.0"))','    implementation("com.google.firebase:firebase-messaging")']
        if args.maps:dependencies.append('    implementation("com.google.android.gms:play-services-maps:20.1.0")')
        if args.billing:dependencies.append('    implementation("com.android.billingclient:billing:9.1.0")')
        media='<provider android:name="com.theaimartco.taim.TaimMediaProvider" android:authorities="${applicationId}.taim.media" android:exported="false" android:grantUriPermissions="true" />' if args.media else ''
        background='<service android:name="com.theaimartco.taim.TaimWorkService" android:exported="true" android:permission="android.permission.BIND_JOB_SERVICE"><meta-data android:name="com.theaimartco.taim.work.lib_name" android:value="taim_app" /></service>' if args.background_work else ''
        target.write_text(source.read_text().replace('@PACKAGE@',args.package).replace('@DEEP_LINK@',deep).replace('@PERMISSIONS@',permissions).replace('@MEDIA_PROVIDER@',media).replace('@BACKGROUND_SERVICE@',background).replace('@BACKGROUND_WORK@','true' if args.background_work else 'false')
            .replace('@EXTENSIONS@',extensions).replace('@DEPENDENCIES@','\n'.join(dependencies))
            .replace('@APP_PLUGINS@','; id("com.google.gms.google-services")' if args.fcm else '')
            .replace('@ROOT_PLUGINS@','; id("com.google.gms.google-services") version "4.4.2" apply false' if args.fcm else ''))
    java=output/'app/src/main/java/com/theaimartco/taim';java.mkdir(parents=True)
    for source in (ROOT/'winit/android/com/theaimartco/taim').glob('*.java'):shutil.copy2(source,java/source.name)
    for enabled,name in ((args.fcm,'fcm'),(args.maps,'maps'),(args.billing,'billing')):
        if enabled:
            for source in (ROOT/'scripts/android/extensions'/name).glob('*.java'):shutil.copy2(source,java/source.name)
    if args.fcm:shutil.copy2(args.fcm,output/'app/google-services.json')
    if args.fcm or args.maps or args.billing:
        # Play services/Firebase/Billing depend on AndroidX; the core host itself does not.
        properties=output/'gradle.properties';properties.write_text(properties.read_text().replace('android.useAndroidX=false','android.useAndroidX=true'))
    rust=output/'rust';(rust/'src').mkdir(parents=True)
    # Persist a project-specific crate name in the manifest. Moving/rebuilding
    # that source preserves its identity; separate generated copies cannot
    # collide even when they use the same Android application ID/cache.
    crate_name='taim_app_'+hashlib.sha256((args.package+'\0'+str(output)).encode()).hexdigest()[:16]
    (rust/'Cargo.toml').write_text('[package]\nname='+json.dumps(crate_name)+'\nversion="0.1.0"\nedition="2024"\n[lib]\ncrate-type=["cdylib"]\n[dependencies]\ntaim={path='+json.dumps(str(ROOT))+',default-features=false,features=["wgpu-android"]}\n[features]\nmeasurement=["taim/measurement"]\n[workspace]\n[profile.release]\nlto="fat"\ncodegen-units=1\nstrip="debuginfo"\n')
    # Cargo intentionally does not inherit dependency workspaces' patches.
    with (rust/'Cargo.toml').open('a') as manifest:
        manifest.write('\n[patch.crates-io]\nwgpu-hal={path='+json.dumps(str(ROOT/'vendor/wgpu-hal'))+'}\n')
    (rust/'src/lib.rs').write_text('''#[unsafe(no_mangle)]
pub fn android_main(activity: taim::android::AndroidApp) {
    use taim::prelude::*;
    let result=taim::app("Taim App",|cx| {
        let count=cx.restorable("count",1,0_i64).expect("unique restoration key");
        column(views![text(move ||format!("Count: {}",count.get())),button("Increment").on_press(move ||count.update(|value|*value+=1))]).padding(32.0).gap(16.0)
    }).run_android(activity);
    if let Err(error)=result {eprintln!("Taim Android failed: {error}");}
}
''')
    if args.background_work:
        with (rust/'src/lib.rs').open('a') as source:
            source.write('''
taim::android::background_worker!(handle_background_work);
fn handle_background_work(context: &mut taim::android::WorkContext<'_>) -> taim::android::WorkResult {
    // Replace this dispatcher with bounded, cancellation-aware, idempotent application work.
    let _ = context;
    taim::android::WorkResult::Failure(serde_json::json!({"code":"unconfiguredWorker"}))
}
''')
        with (rust/'Cargo.toml').open('a') as manifest:
            manifest.write('\n[target.\'cfg(target_os = "android")\'.dependencies]\nserde_json="1.0"\n')
    toolchain=ROOT/'rust-toolchain.toml'
    if toolchain.exists():shutil.copy2(toolchain,output/'rust-toolchain.toml')
    (output/'gradlew').write_text('#!/bin/sh\nexec python3 "$(dirname "$0")/tools/gradle-bootstrap.py" "$@"\n');(output/'gradlew').chmod(0o755)
    (output/'gradlew.bat').write_text('@python "%~dp0tools\\gradle-bootstrap.py" %*\r\n')
    (output/'.gitignore').write_text('.gradle/\n.gradle-bootstrap/\n**/build/\n**/target/\nlocal.properties\n*.jks\n*.keystore\n')
    (output/'README.md').write_text('''# Taim Android application
Requires JDK 21, Python 3, SDK platform 36, build-tools 35.0.0, NDK 28.2.13676358 and Rust Android targets aarch64-linux-android/x86_64-linux-android.
Set ANDROID_HOME or create local.properties. `./gradlew assembleDebug` builds a development APK; `./gradlew bundleRelease` builds an AAB.
Successful assembleRelease/bundleRelease tasks also require the full lintRelease check. Run lintDebug during development; do not disable compatibility/permission lint to obtain a passing build.
Release tasks require TAIM_KEYSTORE (absolute path), TAIM_KEY_ALIAS, TAIM_STORE_PASSWORD and TAIM_KEY_PASSWORD. They never fall back to a debug key. Keep secrets in environment/secret storage, not project files.
The pinned bootstrap verifies the Gradle distribution SHA-256. The Rust task verifies JNI exports, system dependencies and 16 KiB ELF alignment. AGP packages native libraries uncompressed with modern alignment.
Java host files are a generated snapshot; regenerate/copy reviewed framework updates when upgrading Taim. Register Java/Kotlin plugins after super.onCreate using registerTaimPlugin, then call typed Request::Plugin from Rust.
Native hierarchy slots use android_view(factory, JSON properties). Register UI-thread Java/Kotlin factories with registerTaimViewFactory after super.onCreate in your Activity subclass and select that subclass in the manifest. The built-in taim.texture factory provides a TextureView producer. Factories own native input/accessibility and cleanup; SurfaceView children and arbitrary GPU/native layer interleaving are unsupported. See the framework docs/android/native-views.md.
The built-in taim.web factory and android_web_view helper load bounded inline HTML or HTTPS content; JavaScript is off by default. Use project.py --internet for explicit network access. See docs/android/webview.md for navigation policy and limits.
Services::read_sensor returns a validated foreground motion/environment snapshot with bounded timeout and cancellation; SensorInventory lists available permission-free types. No background/body/high-rate sensing is enabled. See docs/android/sensors.md.
Use project.py --permission android.permission.CAMERA (repeatable) to declare app-selected permissions. Services::permission_status/request_permissions return typed observations; first request is distinct from completed denial without rationale. SpecialAccessStatus/SpecialAccessSettings handle OS-managed access separately. Opening settings never grants access; requery after resume. Applications need no accessibility-service permission to expose their own virtual accessibility nodes.
State persistence is opt-in through Scope::restorable with unique versioned keys; do not persist password signals. Deep-link input is untrusted and must be validated by the application.
App bundles/signing do not replace device testing, accessibility acceptance or Play Console policy review. Nothing in these tasks uploads or publishes an app.
Use project.py --notifications to opt into the POST_NOTIFICATIONS manifest declaration. Request permission explicitly on API 33+, create a channel, then post through Request::NotificationPost. No push provider or alarm scheduler is installed.
Use project.py --media to register TaimMediaProvider for camera capture. PickMedia uses the system picker/SAF without broad storage permission. CaptureMedia delegates to a camera app; PreviewMedia opens a native viewer. Delete successful owned captures explicitly. Interrupted/incomplete captures are removed automatically. Embedded camera preview/playback is separate native-view work.
Use project.py --location (approximate) or --location precise to declare only foreground location permissions. Request permission explicitly, then use Services::current_location or Services::watch_location. Watches expire and stop on activity pause/drop; no background location service is installed.
Use project.py --background-work for durable JobScheduler work. Implement the generated background_worker! dispatcher; it runs without an Activity. Jobs are deferrable and at-least-once, not exact alarms. Check WorkContext::is_stopped around bounded I/O, and keep external effects idempotent. No push transport or foreground service is installed. Add INTERNET explicitly if the handler uses network I/O.
''')
    print(output)
if __name__=='__main__':main()
