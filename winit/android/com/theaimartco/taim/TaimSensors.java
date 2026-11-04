package com.theaimartco.taim;

import android.hardware.*;
import android.os.*;
import org.json.*;
import java.lang.ref.WeakReference;
import java.util.*;

/** Bounded foreground snapshots of permission-free motion/environment sensors. */
final class TaimSensors {
    interface Reply {void complete(Object value,String error);}
    private final WeakReference<TaimActivity> activity;
    private final SensorManager manager;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final Set<Handle> active=new HashSet<>();
    private static final int[] TYPES={1,2,4,5,6,8,9,10,11,12,13,15,20};
    TaimSensors(TaimActivity activity){this.activity=new WeakReference<>(activity);manager=activity.getSystemService(SensorManager.class);}
    private static boolean supported(int type){for(int allowed:TYPES)if(type==allowed)return true;return false;}
    JSONArray inventory()throws JSONException {
        JSONArray result=new JSONArray();if(manager!=null)for(int type:TYPES){Sensor sensor=manager.getDefaultSensor(type);if(sensor!=null)result.put(new JSONObject().put("type",type).put("name",sensor.getName()).put("vendor",sensor.getVendor()).put("minimum_delay_us",sensor.getMinDelay()).put("maximum_range",sensor.getMaximumRange()).put("resolution",sensor.getResolution()).put("power_ma",sensor.getPower()));}return result;
    }
    Handle read(JSONObject request,Reply reply)throws Exception {
        Object sensorValue=request.get("sensor"),timeoutValue=request.get("timeout");
        if(!(sensorValue instanceof Integer || sensorValue instanceof Long) || !(timeoutValue instanceof Integer || timeoutValue instanceof Long))throw new IllegalArgumentException();
        long rawType=((Number)sensorValue).longValue(),rawTimeout=((Number)timeoutValue).longValue();
        if(rawType<Integer.MIN_VALUE || rawType>Integer.MAX_VALUE || rawTimeout<Integer.MIN_VALUE || rawTimeout>Integer.MAX_VALUE)throw new IllegalArgumentException();
        int type=(int)rawType,timeout=(int)rawTimeout;
        if(!supported(type) || timeout<100 || timeout>10000)throw new IllegalArgumentException();
        TaimActivity host=activity.get();if(host==null || !host.isTaimResumed())throw new TaimLocation.ForegroundException();
        if(manager==null)throw new UnsupportedOperationException();Sensor sensor=manager.getDefaultSensor(type);if(sensor==null)throw new UnsupportedOperationException();
        if(active.size()>=8)throw new java.util.concurrent.RejectedExecutionException();
        Handle handle=new Handle(sensor,reply);active.add(handle);
        try {
            // 50 Hz maximum requested rate: no high-rate or body-sensor permission.
            if(!manager.registerListener(handle,sensor,20000,main))throw new UnsupportedOperationException();
            main.postDelayed(handle.deadline,timeout);return handle;
        }catch(RuntimeException failure){handle.close();throw failure;}
    }
    void pause(){for(Handle handle:new ArrayList<>(active))handle.end(null,"cancelledForeground");}
    void close(){pause();}
    final class Handle implements SensorEventListener {
        private final Sensor sensor;private final Reply reply;private final long started=SystemClock.elapsedRealtimeNanos();private boolean closed;
        final Runnable deadline=()->end(null,"timeout");
        Handle(Sensor sensor,Reply reply){this.sensor=sensor;this.reply=reply;}
        @Override public void onAccuracyChanged(Sensor sensor,int accuracy){}
        @Override public void onSensorChanged(SensorEvent event){
            if(closed || event.sensor!=sensor || event.timestamp<started || event.values.length==0 || event.values.length>16)return;
            TaimActivity host=activity.get();if(host==null || !host.isTaimResumed()){end(null,"cancelledForeground");return;}
            try {
                JSONArray values=new JSONArray();for(float value:event.values){if(!Float.isFinite(value))return;values.put((double)value);}
                end(new JSONObject().put("sensor",sensor.getType()).put("timestamp_ns",event.timestamp).put("accuracy",event.accuracy).put("values",values),null);
            }catch(JSONException failed){end(null,"failed");}
        }
        void end(Object value,String error){if(closed)return;close();reply.complete(value,error);}
        void close(){if(closed)return;closed=true;active.remove(this);main.removeCallbacks(deadline);try{manager.unregisterListener(this);}catch(RuntimeException ignored){}}
    }
}
