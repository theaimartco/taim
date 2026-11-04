package com.theaimartco.taim;

import android.app.Activity;
import android.content.*;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.*;
import android.security.keystore.*;
import android.util.Base64;
import android.view.HapticFeedbackConstants;
import org.json.*;
import java.io.*;
import java.lang.ref.WeakReference;
import java.security.KeyStore;
import java.util.*;
import java.util.concurrent.*;
import javax.crypto.*;
import javax.crypto.spec.GCMParameterSpec;

/** Lifecycle-owned RPC dispatcher. No arbitrary Java method invocation or file-path RPC. */
final class TaimServices {
    static final int PERMISSIONS=7311, OPEN=7312, CREATE=7313, MEDIA=7314;
    private static final int LIMIT=4*1024*1024;
    private static final Set<String> PLUGIN_ERRORS=new HashSet<>(Arrays.asList("denied","cancelled","cancelledForeground","unsupported","unavailable","disabled","busy","invalid","timeout","notFound"));
    private final WeakReference<TaimActivity> activity;
    private final Context context;
    private final TaimLocation locations;
    private final TaimSensors sensors;
    private final TaimPermissions permissionManager;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final ThreadPoolExecutor worker=new ThreadPoolExecutor(1,1,0,TimeUnit.SECONDS,new ArrayBlockingQueue<>(8),new ThreadPoolExecutor.AbortPolicy());
    private final Map<Long,Job> jobs=new HashMap<>();
    private final Map<String,TaimPlugin> plugins=new HashMap<>();
    private final Map<Integer,Long> results=new HashMap<>();
    private int nextCode=0x4000;
    private long interactive;
    private String[] permissions;
    private boolean closed;
    private static final class Job { final long token; final CancellationSignal cancellation=new CancellationSignal(); Future<?> future; Runnable timeout; int code; int kind; TaimMedia.Session media; TaimLocation.Handle location; TaimSensors.Handle sensor; boolean biometric; Job(long token){this.token=token;} }
    TaimServices(TaimActivity host){activity=new WeakReference<>(host);context=host.getApplicationContext();locations=new TaimLocation(host);sensors=new TaimSensors(host);permissionManager=new TaimPermissions(host);TaimWork.initialize(context);
        // Framework-only built-ins. Each owns its OS resources and is detached in close().
        plugins.put("taim.bluetooth",new TaimBluetooth(host));
        plugins.put("taim.nfc",new TaimNfc(host));
        plugins.put("taim.geofence",new TaimGeofence(host));
        plugins.put("taim.foreground",new TaimForeground(host));
    }
    private TaimActivity host(){TaimActivity host=activity.get();if(host==null || closed || host.isFinishing())throw new IllegalStateException();return host;}
    private void complete(long id,Object value,String error){
        Job job=jobs.remove(id);if(job==null)return;
        if(job.location!=null)job.location.close();
        if(job.sensor!=null)job.sensor.close();
        if(job.media!=null)job.media.close(error==null);
        if(job.code!=0)results.remove(job.code);if(job.timeout!=null)main.removeCallbacks(job.timeout);
        if(interactive==id){interactive=0;permissions=null;}
        try {JSONObject result=new JSONObject();result.put("ok",error==null);if(error==null)result.put("value",value==null?JSONObject.NULL:value);else result.put("error",error);
            TaimActivity.serviceResponse(job.token,id,result.toString());
        }catch(JSONException ignored){TaimActivity.serviceResponse(job.token,id,"{\"ok\":false,\"error\":\"failed\"}");}
    }
    private void finishAsync(long id,Object value,String error){if(!closed)main.post(()->complete(id,value,error));}
    private int busyPicker(long id,int kind){if(interactive!=0 || nextCode>=0x8000)throw new RejectedExecutionException();interactive=id;Job job=jobs.get(id);job.code=nextCode++;job.kind=kind;results.put(job.code,id);return job.code;}
    void request(long token,long id,String payload){
        if(closed || jobs.size()>=8){TaimActivity.serviceResponse(token,id,"{\"ok\":false,\"error\":\"busy\"}");return;}
        Job job=new Job(token);jobs.put(id,job);
        job.timeout=()->{Job live=jobs.get(id);if(live!=null){live.cancellation.cancel();if(live.future!=null)live.future.cancel(true);complete(id,null,"timeout");}};main.postDelayed(job.timeout,120000);
        try {
            JSONObject request=new JSONObject(payload);String op=request.getString("op");TaimActivity host=host();
            switch(op){
                case "biometricStatus":complete(id,TaimBiometric.status(host,request),null);return;
                case "biometricKeyCreate":case "biometricKeyDelete":case "biometricPublicKey":
                    job.future=worker.submit(()->{try{finishAsync(id,op.equals("biometricKeyCreate")?TaimBiometric.createKey(request):op.equals("biometricKeyDelete")?TaimBiometric.deleteKey(request):TaimBiometric.publicKey(request),null);}
                        catch(UnsupportedOperationException absent){finishAsync(id,null,"unsupported");}
                        catch(IllegalArgumentException | JSONException bad){finishAsync(id,null,"invalid");}
                        // No enrolled strong biometric: Keystore refuses to create an auth-bound key.
                        catch(java.security.InvalidAlgorithmParameterException notEnrolled){finishAsync(id,null,"notEnrolled");}
                        catch(Exception failed){finishAsync(id,null,"failed");}});return;
                case "biometricSign":case "biometricEncrypt":case "biometricDecrypt": {
                    if(interactive!=0)throw new RejectedExecutionException();interactive=id;job.biometric=true;
                    try{TaimBiometric.cryptoOperation(host,op,request,job.cancellation,(value,error)->complete(id,value,error));}
                    catch(TaimBiometric.MissingKeyException missing){complete(id,null,"notFound");}
                    catch(android.security.keystore.KeyPermanentlyInvalidatedException invalidated){complete(id,null,"keyInvalidated");}
                    catch(java.security.InvalidKeyException | java.security.UnrecoverableKeyException unusable){complete(id,null,"keyInvalidated");}
                    return;
                }
                case "biometricAuthenticate": {
                    if(interactive!=0)throw new RejectedExecutionException();interactive=id;job.biometric=true;
                    TaimBiometric.authenticate(host,request,job.cancellation,(value,error)->complete(id,value,error));return;
                }
                case "sensorInventory":complete(id,sensors.inventory(),null);return;
                case "sensorRead":job.sensor=sensors.read(request,(value,error)->complete(id,value,error));return;
                case "workEnqueue": case "workInfo": case "workList": case "workCancel": case "workPrune": case "workReconcile":
                    job.future=worker.submit(()->{try{finishAsync(id,TaimWork.request(context,request),null);}
                        catch(UnsupportedOperationException absent){finishAsync(id,null,"unsupported");}catch(RejectedExecutionException busy){finishAsync(id,null,"busy");}
                        catch(SecurityException denied){finishAsync(id,null,"denied");}catch(IllegalArgumentException | JSONException bad){finishAsync(id,null,"invalid");}catch(Exception failed){finishAsync(id,null,"failed");}});return;
                case "locationStatus":complete(id,locations.status(),null);return;
                case "locationSettings":host.startActivity(new Intent(android.provider.Settings.ACTION_LOCATION_SOURCE_SETTINGS));complete(id,null,null);return;
                case "locationLast":complete(id,locations.last(request),null);return;
                case "locationCurrent":job.location=locations.current(request,(value,error)->complete(id,value,error));return;
                case "locationWatch":complete(id,locations.watch(request),null);return;
                case "locationStop":{long watch=request.getLong("watch");if(watch<=0)throw new IllegalArgumentException();locations.stop(watch);complete(id,null,null);return;}
                case "mediaPick": case "mediaCapture": {
                    int code=busyPicker(id,MEDIA);job.media=new TaimMedia.Session(host,request);job.media.launch(host,request,code);return;
                }
                case "mediaInfo": case "mediaRetain": case "mediaRelease": case "mediaDelete": case "mediaPreview": case "mediaList": {
                    job.future=worker.submit(()->{try {
                        Object value=TaimMedia.io(context,request,job.cancellation);
                        if(op.equals("mediaPreview"))main.post(()->{
                            if(!jobs.containsKey(id))return;
                            try {JSONObject info=(JSONObject)value;TaimMedia.preview(host(),TaimMedia.uri(info.getString("uri")),info.optString("mime",""));complete(id,null,null);}
                            catch(ActivityNotFoundException absent){complete(id,null,"unsupported");}catch(SecurityException denied){complete(id,null,"denied");}catch(IllegalArgumentException bad){complete(id,null,"invalid");}catch(Exception failed){complete(id,null,"failed");}
                        });
                        else finishAsync(id,value,null);
                    }catch(SecurityException denied){finishAsync(id,null,"denied");}catch(IllegalArgumentException bad){finishAsync(id,null,"invalid");}catch(Exception failed){finishAsync(id,null,"failed");}});return;
                }
                case "clipboardRead": {
                    ClipboardManager clipboard=(ClipboardManager)context.getSystemService(Context.CLIPBOARD_SERVICE);
                    ClipData data=clipboard.getPrimaryClip();Object value=null;
                    if(data!=null && data.getItemCount()>0){ClipData.Item item=data.getItemAt(0);if(request.optBoolean("html"))value=item.getHtmlText();else{CharSequence text=item.getText();value=text==null?null:text.toString();}}
                    if(value instanceof String && ((String)value).length()>LIMIT)throw new IllegalArgumentException();complete(id,value,null);return;
                }
                case "clipboardWrite": {
                    String text=request.getString("text");String html=request.isNull("html")?null:request.getString("html");
                    if(text.length()>LIMIT || (html!=null && html.length()>LIMIT))throw new IllegalArgumentException();
                    ClipData data=html==null?ClipData.newPlainText("Taim",text):ClipData.newHtmlText("Taim",text,html);
                    if(request.optBoolean("sensitive")){PersistableBundle extra=new PersistableBundle();extra.putBoolean("android.content.extra.IS_SENSITIVE",true);data.getDescription().setExtras(extra);}
                    ((ClipboardManager)context.getSystemService(Context.CLIPBOARD_SERVICE)).setPrimaryClip(data);complete(id,null,null);return;
                }
                case "permissions": case "permissionStatus": {
                    JSONArray names=request.getJSONArray("names");if(names.length()==0 || names.length()>32)throw new IllegalArgumentException();
                    String[] values=new String[names.length()];for(int i=0;i<values.length;i++){values[i]=names.getString(i);if(!values[i].startsWith("android.permission.") || values[i].length()>256)throw new IllegalArgumentException();}
                    if(new HashSet<>(Arrays.asList(values)).size()!=values.length)throw new IllegalArgumentException();
                    if(op.equals("permissionStatus")){complete(id,permissionManager.status(values),null);return;}
                    if(permissionManager.busy())throw new RejectedExecutionException();
                    String[] ask=permissionManager.requestable(values);if(ask.length==0){complete(id,permissionManager.status(values),null);return;}
                    if(!host.isTaimResumed()){complete(id,null,"cancelledForeground");return;}
                    int code=busyPicker(id,PERMISSIONS);permissions=values;permissionManager.begin(code,ask);
                    try{host.requestPermissions(ask,code);}catch(RuntimeException failure){permissionManager.launchFailed(code);throw failure;}return;
                }
                case "specialAccessStatus": case "specialAccessSettings":complete(id,permissionManager.special(request.getString("kind"),op.equals("specialAccessSettings")),null);return;
                case "appSettings": host.startActivity(new Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS,Uri.fromParts("package",host.getPackageName(),null)));complete(id,null,null);return;
                case "notificationChannel": case "notificationChannelDelete": case "notificationPost":
                case "notificationCancel": case "notificationStatus": case "notificationSettings":
                    complete(id,TaimNotifications.request(host,request),null);return;
                case "openUrl": {
                    Uri uri=Uri.parse(request.getString("uri"));String scheme=uri.getScheme();
                    if(!Arrays.asList("https","http","mailto","tel","geo").contains(scheme))throw new IllegalArgumentException();
                    Intent intent=new Intent(Intent.ACTION_VIEW,uri);intent.addCategory(Intent.CATEGORY_BROWSABLE);host.startActivity(intent);complete(id,null,null);return;
                }
                case "share": {
                    String text=request.getString("text");if(text.length()>LIMIT)throw new IllegalArgumentException();
                    Intent intent=new Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT,text);
                    host.startActivity(Intent.createChooser(intent,request.optString("title","")));complete(id,null,null);return;
                }
                case "shareDocument": {
                    Uri uri=content(request.getString("uri"));Intent intent=new Intent(Intent.ACTION_SEND).setType(mime(request.getString("type"))).putExtra(Intent.EXTRA_STREAM,uri);
                    intent.setClipData(ClipData.newRawUri("Taim",uri));intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                    host.startActivity(Intent.createChooser(intent,request.optString("title","")));complete(id,null,null);return;
                }
                case "openDocument": {
                    JSONArray types=request.getJSONArray("types");if(types.length()==0 || types.length()>32)throw new IllegalArgumentException();
                    String[] values=new String[types.length()];for(int i=0;i<values.length;i++)values[i]=mime(types.getString(i));
                    Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(values.length==1?values[0]:"*/*");
                    if(values.length>1)intent.putExtra(Intent.EXTRA_MIME_TYPES,values);
                    intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION|Intent.FLAG_GRANT_WRITE_URI_PERMISSION|Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
                    host.startActivityForResult(intent,busyPicker(id,OPEN));return;
                }
                case "createDocument": {
                    String name=request.getString("name");if(name.isEmpty() || name.length()>255 || name.contains("/") || name.contains("\\"))throw new IllegalArgumentException();
                    Intent intent=new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(mime(request.getString("type"))).putExtra(Intent.EXTRA_TITLE,name);
                    intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION|Intent.FLAG_GRANT_WRITE_URI_PERMISSION|Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
                    host.startActivityForResult(intent,busyPicker(id,CREATE));return;
                }
                case "haptic": {
                    int kind=request.getInt("kind");int feedback=kind==0?HapticFeedbackConstants.LONG_PRESS:kind==1?HapticFeedbackConstants.KEYBOARD_TAP:HapticFeedbackConstants.CLOCK_TICK;
                    complete(id,host.getWindow().getDecorView().performHapticFeedback(feedback),null);return;
                }
                case "device": {
                    JSONObject info=new JSONObject();info.put("sdk",Build.VERSION.SDK_INT);info.put("manufacturer",Build.MANUFACTURER);info.put("model",Build.MODEL);
                    info.put("locale",context.getResources().getConfiguration().getLocales().get(0).toLanguageTag());info.put("density",context.getResources().getDisplayMetrics().density);info.put("fontScale",context.getResources().getConfiguration().fontScale);
                    complete(id,info,null);return;
                }
                case "directories": {
                    JSONObject directories=new JSONObject();directories.put("files",context.getFilesDir().getAbsolutePath());directories.put("cache",context.getCacheDir().getAbsolutePath());directories.put("noBackup",context.getNoBackupFilesDir().getAbsolutePath());complete(id,directories,null);return;
                }
                case "plugin": {
                    TaimPlugin plugin=plugins.get(request.getString("channel"));if(plugin==null)throw new IllegalArgumentException();
                    String method=request.getString("method");if(method.isEmpty() || method.length()>128)throw new IllegalArgumentException();
                    plugin.call(method,request.getJSONObject("arguments"),new TaimPlugin.Result(){public void success(Object value){if(value!=null && value.toString().length()>6*1024*1024){finishAsync(id,null,"invalid");return;}finishAsync(id,value,null);}public void error(){finishAsync(id,null,"failed");}public void error(String code){finishAsync(id,null,PLUGIN_ERRORS.contains(code)?code:"failed");}});return;
                }
                case "intent": complete(id,intent(host.getIntent()),null);return;
                case "readDocument": case "writeDocument": case "asset": case "securePut": case "secureGet": case "secureDelete":
                    job.future=worker.submit(()->{try {finishAsync(id,io(request,job.cancellation),null);}catch(SecurityException denied){finishAsync(id,null,"denied");}catch(IllegalArgumentException bad){finishAsync(id,null,"invalid");}catch(Exception failed){finishAsync(id,null,"failed");}});return;
                default: throw new IllegalArgumentException();
            }
        } catch(TaimLocation.DisabledException disabled){complete(id,null,"locationDisabled");}
        catch(TaimLocation.ForegroundException background){complete(id,null,"cancelledForeground");}
        catch(ActivityNotFoundException | UnsupportedOperationException absent){complete(id,null,"unsupported");}
        catch(RejectedExecutionException busy){complete(id,null,"busy");}
        catch(SecurityException denied){complete(id,null,"denied");}
        catch(IllegalArgumentException | JSONException bad){complete(id,null,"invalid");}
        catch(Exception failed){complete(id,null,"failed");}
    }
    void register(String channel,TaimPlugin plugin){if(closed || channel.isEmpty() || channel.length()>128 || plugins.size()>=32 || plugins.containsKey(channel))throw new IllegalArgumentException();plugins.put(channel,plugin);}
    boolean registered(String channel){return plugins.containsKey(channel);}
    void pauseLocation(){locations.pause();sensors.pause();for(TaimPlugin plugin:plugins.values()){try{plugin.paused();}catch(RuntimeException ignored){}}}
    void resumePlugins(){for(TaimPlugin plugin:plugins.values()){try{plugin.resumed();}catch(RuntimeException ignored){}}}
    void stopLocation(long watch){locations.stop(watch);}
    void cancelLocationRequest(long id){Job job=jobs.get(id);if(job!=null && (job.location!=null || job.sensor!=null || job.biometric)){if(job.biometric)job.cancellation.cancel();complete(id,null,"cancelledForeground");}}
    private static String mime(String value){if(value.length()>256 || !value.contains("/") || value.contains("\n") || value.contains("\r"))throw new IllegalArgumentException();return value;}
    private static Uri content(String value){Uri uri=Uri.parse(value);if(!"content".equals(uri.getScheme()))throw new IllegalArgumentException();return uri;}
    private static byte[] decode(String value,int limit){if(value.length()>limit*2)throw new IllegalArgumentException();byte[] bytes=Base64.decode(value,Base64.DEFAULT);if(bytes.length>limit)throw new IllegalArgumentException();return bytes;}
    private static String read(InputStream stream,int limit,CancellationSignal cancellation)throws IOException {
        try(InputStream input=stream;ByteArrayOutputStream output=new ByteArrayOutputStream()){
            byte[] buffer=new byte[8192];int count;
            while((count=input.read(buffer))!=-1){cancellation.throwIfCanceled();if(output.size()+count>limit)throw new IllegalArgumentException();output.write(buffer,0,count);}
            return Base64.encodeToString(output.toByteArray(),Base64.NO_WRAP);
        }
    }
    private Object io(JSONObject request,CancellationSignal cancellation)throws Exception {
        String op=request.getString("op");ContentResolver resolver=context.getContentResolver();
        if(op.equals("readDocument")){
            int limit=request.getInt("limit");if(limit<0 || limit>LIMIT)throw new IllegalArgumentException();
            android.content.res.AssetFileDescriptor file=resolver.openAssetFileDescriptor(content(request.getString("uri")),"r",cancellation);
            if(file==null)throw new FileNotFoundException();try(android.content.res.AssetFileDescriptor owned=file){return read(owned.createInputStream(),limit,cancellation);}
        }
        if(op.equals("writeDocument")){
            byte[] bytes=decode(request.getString("data"),LIMIT);
            ParcelFileDescriptor file=resolver.openFileDescriptor(content(request.getString("uri")),"wt",cancellation);
            if(file==null)throw new FileNotFoundException();try(OutputStream output=new ParcelFileDescriptor.AutoCloseOutputStream(file)){cancellation.throwIfCanceled();output.write(bytes);}return JSONObject.NULL;
        }
        if(op.equals("asset")){
            String name=request.getString("name");if(name.isEmpty() || name.startsWith("/") || Arrays.asList(name.split("/")).contains(".."))throw new IllegalArgumentException();
            int limit=request.getInt("limit");if(limit<0 || limit>LIMIT)throw new IllegalArgumentException();return read(context.getAssets().open(name),limit,cancellation);
        }
        String name=request.getString("key");if(name.isEmpty() || name.length()>128)throw new IllegalArgumentException();
        SharedPreferences preferences=context.getSharedPreferences("taim.secure.v1",Context.MODE_PRIVATE);
        if(op.equals("secureDelete")){if(!preferences.edit().remove(name).commit())throw new IOException();return JSONObject.NULL;}
        KeyStore store=KeyStore.getInstance("AndroidKeyStore");store.load(null);String alias="taim.secure.aes.v1";
        if(!store.containsAlias(alias)){
            KeyGenerator generator=KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES,"AndroidKeyStore");
            generator.init(new KeyGenParameterSpec.Builder(alias,KeyProperties.PURPOSE_ENCRYPT|KeyProperties.PURPOSE_DECRYPT).setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).setRandomizedEncryptionRequired(true).build());generator.generateKey();
        }
        javax.crypto.SecretKey key=(javax.crypto.SecretKey)store.getKey(alias,null);Cipher cipher=Cipher.getInstance("AES/GCM/NoPadding");
        if(op.equals("securePut")){
            byte[] bytes=decode(request.getString("data"),65536);cipher.init(Cipher.ENCRYPT_MODE,key);cipher.updateAAD(name.getBytes(java.nio.charset.StandardCharsets.UTF_8));byte[] encrypted=cipher.doFinal(bytes);
            String value=Base64.encodeToString(cipher.getIV(),Base64.NO_WRAP)+":"+Base64.encodeToString(encrypted,Base64.NO_WRAP);
            if(!preferences.edit().putString(name,value).commit())throw new IOException();return JSONObject.NULL;
        }
        String value=preferences.getString(name,null);if(value==null)return JSONObject.NULL;
        String[] parts=value.split(":",2);if(parts.length!=2)throw new IOException();byte[] iv=decode(parts[0],32);
        cipher.init(Cipher.DECRYPT_MODE,key,new GCMParameterSpec(128,iv));cipher.updateAAD(name.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        return Base64.encodeToString(cipher.doFinal(decode(parts[1],65536+32)),Base64.NO_WRAP);
    }
    void resumePermissions(){permissionManager.refresh();}
    void permissionResult(int requestCode,String[] names,int[] grants){
        if(!permissionManager.finish(requestCode,names,grants))return;
        Long id=results.get(requestCode);Job job=id==null?null:jobs.get(id);
        if(job==null || job.kind!=PERMISSIONS || interactive!=id || permissions==null)return;
        try {complete(interactive,permissionManager.status(permissions),null);
        }catch(Exception failed){complete(interactive,null,"failed");}
    }
    void activityResult(int requestCode,int resultCode,Intent data){
        Long id=results.get(requestCode);Job job=id==null?null:jobs.get(id);
        if(job==null || interactive!=id)return;
        if(job.kind==MEDIA){
            if(resultCode!=Activity.RESULT_OK){complete(id,null,"cancelled");return;}
            if(job.media==null){complete(id,null,"failed");return;}
            try {job.future=worker.submit(()->{try{finishAsync(id,job.media.result(data,job.cancellation),null);}
                catch(SecurityException denied){finishAsync(id,null,"denied");}catch(IllegalArgumentException bad){finishAsync(id,null,"invalid");}catch(Exception failed){finishAsync(id,null,"failed");}});}
            catch(RejectedExecutionException busy){complete(id,null,"busy");}return;
        }
        if(job.kind!=OPEN && job.kind!=CREATE)return;
        if(resultCode!=Activity.RESULT_OK || data==null || data.getData()==null){complete(interactive,null,"cancelled");return;}
        try {Uri uri=data.getData();int flags=data.getFlags()&(Intent.FLAG_GRANT_READ_URI_PERMISSION|Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
            if((data.getFlags()&Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)!=0){try{if((flags&Intent.FLAG_GRANT_READ_URI_PERMISSION)!=0 && (flags&Intent.FLAG_GRANT_WRITE_URI_PERMISSION)!=0)context.getContentResolver().takePersistableUriPermission(uri,Intent.FLAG_GRANT_READ_URI_PERMISSION|Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
                else if((flags&Intent.FLAG_GRANT_READ_URI_PERMISSION)!=0)context.getContentResolver().takePersistableUriPermission(uri,Intent.FLAG_GRANT_READ_URI_PERMISSION);
                else if((flags&Intent.FLAG_GRANT_WRITE_URI_PERMISSION)!=0)context.getContentResolver().takePersistableUriPermission(uri,Intent.FLAG_GRANT_WRITE_URI_PERMISSION);}catch(SecurityException ignored){}}
            JSONObject result=new JSONObject();result.put("uri",uri.toString());result.put("mime",context.getContentResolver().getType(uri));result.put("persisted",context.getContentResolver().getPersistedUriPermissions().stream().anyMatch(permission->permission.getUri().equals(uri)));complete(interactive,result,null);
        }catch(Exception failed){complete(interactive,null,"failed");}
    }
    static JSONObject intent(Intent intent)throws JSONException {JSONObject value=new JSONObject();value.put("type","intent");value.put("action",intent==null?"":intent.getAction());value.put("uri",intent==null?null:intent.getDataString());value.put("mime",intent==null?null:intent.getType());value.put("notification",TaimNotifications.tap(intent));return value;}
    void close(){closed=true;locations.close();sensors.close();main.removeCallbacksAndMessages(null);for(TaimPlugin plugin:plugins.values()){try{plugin.detach();}catch(RuntimeException ignored){}}plugins.clear();for(Job job:jobs.values()){job.cancellation.cancel();if(job.future!=null)job.future.cancel(true);}for(Long id:new ArrayList<>(jobs.keySet()))complete(id,null,"failed");worker.shutdownNow();}
}
