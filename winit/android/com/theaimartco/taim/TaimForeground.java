package com.theaimartco.taim;

import android.app.*;
import android.content.*;
import android.content.pm.PackageManager;
import android.content.pm.ServiceInfo;
import android.os.Build;
import org.json.*;
import java.lang.ref.WeakReference;

/**
 * Foreground service control. Channel {@code taim.foreground}. Requires the opt-in
 * {@link TaimForegroundService} manifest entry with a matching foregroundServiceType and
 * FOREGROUND_SERVICE(_TYPE) permissions; project.py --foreground-service declares them.
 *
 * The service keeps the app process at foreground priority while the activity is in the
 * background, so Rust tasks already running in the UI process continue. It does not run
 * Rust after the activity is destroyed: use durable background work for that.
 */
// ServiceInfo type constants are inlined ints, passed to startForeground(type) only on API 29+.
@android.annotation.SuppressLint("InlinedApi")
final class TaimForeground implements TaimPlugin {
    static final String CHANNEL="taim.foreground";
    private final WeakReference<TaimActivity> activity;
    TaimForeground(TaimActivity host){activity=new WeakReference<>(host);}

    static int type(String name){
        switch(name){
            case "dataSync":return ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC;
            case "mediaPlayback":return ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK;
            case "location":return ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION;
            case "connectedDevice":return ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE;
            case "microphone":return 128; // FOREGROUND_SERVICE_TYPE_MICROPHONE (API 30)
            case "camera":return 64;      // FOREGROUND_SERVICE_TYPE_CAMERA (API 30)
            case "health":return 256;     // API 34
            case "remoteMessaging":return 512; // API 34
            case "shortService":return 2048;   // API 34
            case "specialUse":return 1073741824; // API 34
            default:throw new IllegalArgumentException();
        }
    }

    private int declaredTypes(Context context){
        try{
            ServiceInfo info=context.getPackageManager().getServiceInfo(new ComponentName(context,TaimForegroundService.class),0);
            return Build.VERSION.SDK_INT>=29?info.getForegroundServiceType():-1;
        }catch(PackageManager.NameNotFoundException absent){return 0;}
    }

    @Override public void call(String method,JSONObject args,Result result){
        TaimActivity host=activity.get();if(host==null){result.error("unavailable");return;}
        try{
            int declared=declaredTypes(host);
            if(declared==0){result.error("unsupported");return;}
            switch(method){
                case "status":result.success(new JSONObject().put("running",TaimForegroundService.running));return;
                case "start":case "update":{
                    String title=text(args,"title",256),body=text(args,"text",1024),channel=args.getString("channel");
                    if(!channel.matches("[A-Za-z0-9._-]{1,128}"))throw new IllegalArgumentException();
                    int type=type(args.getString("type"));
                    if(declared!=-1 && (declared&type)!=type){result.error("invalid");return;} // type not declared in the manifest
                    NotificationManager notifications=host.getSystemService(NotificationManager.class);
                    if(notifications==null || notifications.getNotificationChannel(channel)==null){result.error("notFound");return;}
                    if(method.equals("update") && !TaimForegroundService.running){result.error("notFound");return;}
                    Intent intent=new Intent(host,TaimForegroundService.class).putExtra("title",title).putExtra("text",body)
                        .putExtra("channel",channel).putExtra("type",type).putExtra("activity",host.getClass().getName());
                    try{host.startForegroundService(intent);}
                    catch(IllegalStateException notAllowed){result.error("denied");return;} // background start restriction (API 31+)
                    result.success(null);return;
                }
                case "stop":host.stopService(new Intent(host,TaimForegroundService.class));result.success(null);return;
                default:result.error("invalid");
            }
        }catch(SecurityException denied){result.error("denied");}
        catch(JSONException | IllegalArgumentException bad){result.error("invalid");}
    }
    private static String text(JSONObject args,String key,int max)throws JSONException{String v=args.getString(key);if(v.isEmpty() || v.length()>max)throw new IllegalArgumentException();return v;}
}
