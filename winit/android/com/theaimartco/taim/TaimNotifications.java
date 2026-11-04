package com.theaimartco.taim;

import android.app.*;
import android.content.*;
import android.content.pm.PackageManager;
import android.graphics.drawable.Icon;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;
import android.service.notification.StatusBarNotification;
import org.json.*;
import java.nio.charset.StandardCharsets;

/** Local notifications only: no implicit prompts, alarm scheduling, receivers or push transport. */
// The app chooses its notification icon by name at runtime; shrinking is disabled and a missing
// resource fails the request, so reflection-based lookup is intentional.
@android.annotation.SuppressLint("DiscouragedApi")
final class TaimNotifications {
    static final String TAP_ID="com.theaimartco.taim.notification.ID";
    static final String TAP_PAYLOAD="com.theaimartco.taim.notification.PAYLOAD";
    private static String bounded(String text,int max) {
        if(text.getBytes(StandardCharsets.UTF_8).length>max)throw new IllegalArgumentException();
        return text;
    }
    static String channelId(String id) {
        if(!id.matches("[A-Za-z0-9._-]{1,128}"))throw new IllegalArgumentException();
        return id;
    }
    private static NotificationManager manager(Context context) {
        return context.getSystemService(NotificationManager.class);
    }
    private static boolean permission(Context context) {
        return Build.VERSION.SDK_INT<33 || context.checkSelfPermission("android.permission.POST_NOTIFICATIONS")==PackageManager.PERMISSION_GRANTED;
    }
    // This optional helper is present even without a POST_NOTIFICATIONS
    // declaration. Such apps must fail this local OS check, never gain a
    // permission merely by invoking a service method.
    @android.annotation.SuppressLint("NotificationPermission")
    private static void post(TaimActivity host,NotificationManager manager,int id,Notification notification){
        if(Build.VERSION.SDK_INT>=33 && host.checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED)throw new SecurityException();
        manager.notify(id,notification);
    }
    private static JSONObject channel(NotificationManager manager,String id)throws JSONException {
        NotificationChannel value=manager.getNotificationChannel(channelId(id));
        if(value==null)return null;
        return new JSONObject().put("id",value.getId()).put("name",value.getName())
            .put("importance",value.getImportance()).put("enabled",value.getImportance()!=NotificationManager.IMPORTANCE_NONE);
    }
    static Object request(TaimActivity host,JSONObject request)throws Exception {
        NotificationManager manager=manager(host);String op=request.getString("op");
        switch(op) {
            case "notificationChannel": {
                String id=channelId(request.getString("id"));String name=bounded(request.getString("name"),256);
                String description=bounded(request.getString("description"),1024);int importance=request.getInt("importance");
                if(name.trim().isEmpty() || importance<1 || importance>4)throw new IllegalArgumentException();
                NotificationChannel existing=manager.getNotificationChannel(id);
                // Never lower/raise the existing interruption policy on a metadata update.
                NotificationChannel value=existing==null?new NotificationChannel(id,name,importance):existing;
                value.setName(name);value.setDescription(description);manager.createNotificationChannel(value);
                return channel(manager,id);
            }
            case "notificationChannelDelete": manager.deleteNotificationChannel(channelId(request.getString("id")));return JSONObject.NULL;
            case "notificationCancel": {
                int id=request.getInt("id");if(id<0)throw new IllegalArgumentException();manager.cancel(id);return JSONObject.NULL;
            }
            case "notificationStatus": {
                JSONObject result=new JSONObject().put("enabled",manager.areNotificationsEnabled()).put("permissionGranted",permission(host));
                result.put("channel",request.isNull("channel")?JSONObject.NULL:channel(manager,request.getString("channel")));
                JSONArray active=new JSONArray();for(StatusBarNotification notification:manager.getActiveNotifications()) {
                    // Tagged notifications belong to app/plugin code outside this API.
                    if(notification.getTag()==null)active.put(new JSONObject().put("id",notification.getId()).put("channel",notification.getNotification().getChannelId()));
                }
                return result.put("active",active);
            }
            case "notificationSettings": {
                Intent intent;
                if(request.isNull("channel"))intent=new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE,host.getPackageName());
                else {String id=channelId(request.getString("channel"));if(manager.getNotificationChannel(id)==null)throw new IllegalArgumentException();
                    intent=new Intent(Settings.ACTION_CHANNEL_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE,host.getPackageName()).putExtra(Settings.EXTRA_CHANNEL_ID,id);}
                host.startActivity(intent);return JSONObject.NULL;
            }
            case "notificationPost": {
                int id=request.getInt("id");String channel=channelId(request.getString("channel"));
                if(id<0)throw new IllegalArgumentException();
                NotificationChannel existing=manager.getNotificationChannel(channel);if(existing==null)throw new IllegalArgumentException();
                if(!permission(host) || !manager.areNotificationsEnabled() || existing.getImportance()==NotificationManager.IMPORTANCE_NONE)throw new SecurityException();
                String title=bounded(request.getString("title"),1024),body=bounded(request.getString("body"),8192);
                String payload=request.isNull("payload")?null:bounded(request.getString("payload"),4096);
                int visibility=request.getInt("visibility");if(visibility< -1 || visibility>1)throw new IllegalArgumentException();
                int icon=android.R.drawable.stat_notify_more;
                if(!request.isNull("icon")) {
                    String name=request.getString("icon");if(!name.matches("[a-z0-9_]{1,128}"))throw new IllegalArgumentException();
                    icon=host.getResources().getIdentifier(name,"drawable",host.getPackageName());if(icon==0)throw new IllegalArgumentException();
                }
                Intent intent=new Intent(host,host.getClass()).setAction(Intent.ACTION_MAIN)
                    .setData(new Uri.Builder().scheme("taim-notification").authority(host.getPackageName()).appendPath(Integer.toString(id)).build())
                    .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP|Intent.FLAG_ACTIVITY_SINGLE_TOP).putExtra(TAP_ID,id);
                if(payload!=null)intent.putExtra(TAP_PAYLOAD,payload);
                PendingIntent tap=PendingIntent.getActivity(host,id,intent,PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
                Notification.Builder builder=new Notification.Builder(host,channel)
                    .setSmallIcon(Icon.createWithResource(request.isNull("icon")?"android":host.getPackageName(),icon))
                    .setContentTitle(title).setContentText(body).setStyle(new Notification.BigTextStyle().bigText(body))
                    .setContentIntent(tap).setAutoCancel(request.getBoolean("autoCancel"))
                    .setOnlyAlertOnce(request.getBoolean("onlyAlertOnce")).setVisibility(visibility);
                if(!request.isNull("timeoutMs")){long timeout=request.getLong("timeoutMs");if(timeout<1 || timeout>604800000L)throw new IllegalArgumentException();builder.setTimeoutAfter(timeout);}
                post(host,manager,id,builder.build());return JSONObject.NULL;
            }
            default:throw new IllegalArgumentException();
        }
    }
    static JSONObject tap(Intent intent)throws JSONException {
        try {
            if(intent==null || !intent.hasExtra(TAP_ID))return null;
            int id=intent.getIntExtra(TAP_ID,-1);if(id<0)return null;
            String payload=intent.getStringExtra(TAP_PAYLOAD);
            if(payload!=null && payload.getBytes(StandardCharsets.UTF_8).length>4096)return null;
            return new JSONObject().put("id",id).put("payload",payload==null?JSONObject.NULL:payload);
        }catch(RuntimeException malformed){return null;}
    }
}
