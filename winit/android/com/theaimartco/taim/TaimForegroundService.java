package com.theaimartco.taim;

import android.app.*;
import android.content.Intent;
import android.os.Build;
import android.os.IBinder;

/** Opt-in, non-exported foreground service controlled by {@link TaimForeground}. */
public final class TaimForegroundService extends Service {
    static volatile boolean running;
    private static final int NOTIFICATION_ID=0x7A1F;

    @Override public int onStartCommand(Intent intent,int flags,int startId){
        if(intent==null){stopSelf();return START_NOT_STICKY;} // never restart without the app's intent
        Intent open=new Intent().setClassName(this,intent.getStringExtra("activity")).setAction(Intent.ACTION_MAIN)
            .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP|Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent tap=PendingIntent.getActivity(this,NOTIFICATION_ID,open,PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
        Notification notification=new Notification.Builder(this,intent.getStringExtra("channel"))
            .setSmallIcon(android.R.drawable.stat_notify_sync).setContentTitle(intent.getStringExtra("title"))
            .setContentText(intent.getStringExtra("text")).setContentIntent(tap).setOngoing(true).build();
        try{
            if(Build.VERSION.SDK_INT>=29)startForeground(NOTIFICATION_ID,notification,intent.getIntExtra("type",0));
            else startForeground(NOTIFICATION_ID,notification);
            running=true;
        }catch(RuntimeException refused){ // missing type permission or background start restriction
            running=false;stopSelf();
        }
        return START_NOT_STICKY;
    }
    @Override public void onTimeout(int startId){stopSelf();} // shortService/dataSync limits (API 34/35)
    @Override public void onDestroy(){running=false;super.onDestroy();}
    @Override public IBinder onBind(Intent intent){return null;}
}
