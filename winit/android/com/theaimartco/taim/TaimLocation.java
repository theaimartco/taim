package com.theaimartco.taim;

import android.Manifest;
import android.content.*;
import android.content.pm.PackageManager;
import android.location.*;
import android.os.*;
import org.json.*;
import java.lang.ref.WeakReference;
import java.util.*;
import java.util.concurrent.RejectedExecutionException;

/** Foreground-only, bounded location listeners. No background service, implicit prompt or polling. */
final class TaimLocation {
    interface Reply {void complete(Object value,String error);}
    private final WeakReference<TaimActivity> activity;
    private final LocationManager manager;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final Map<Long,Handle> handles=new HashMap<>();
    private long next=1;
    TaimLocation(TaimActivity host){activity=new WeakReference<>(host);manager=host.getSystemService(LocationManager.class);}
    private TaimActivity host(){TaimActivity host=activity.get();if(host==null || host.isFinishing())throw new IllegalStateException();return host;}
    private boolean granted(String permission){TaimActivity host=activity.get();return host!=null && host.checkSelfPermission(permission)==PackageManager.PERMISSION_GRANTED;}
    private boolean fine(){return granted(Manifest.permission.ACCESS_FINE_LOCATION);}
    private boolean coarse(){return granted(Manifest.permission.ACCESS_COARSE_LOCATION);}
    private boolean enabled(){
        if(manager==null)return false;
        if(Build.VERSION.SDK_INT>=28)return Api28.enabled(manager);
        for(String provider:new String[]{LocationManager.GPS_PROVIDER,LocationManager.NETWORK_PROVIDER})if(manager.getAllProviders().contains(provider) && manager.isProviderEnabled(provider))return true;
        return false;
    }
    JSONObject status()throws JSONException {
        JSONArray providers=new JSONArray();if(manager!=null)for(String name:new String[]{LocationManager.GPS_PROVIDER,LocationManager.NETWORK_PROVIDER,LocationManager.PASSIVE_PROVIDER}) {
            if(manager.getAllProviders().contains(name))providers.put(new JSONObject().put("name",name).put("enabled",manager.isProviderEnabled(name)));
        }
        TaimActivity host=activity.get();return new JSONObject().put("enabled",enabled()).put("coarseGranted",coarse()).put("fineGranted",fine())
            .put("foreground",host!=null && host.isTaimResumed()).put("providers",providers).put("active",handles.size());
    }
    private String provider(JSONObject request)throws Exception {
        if(manager==null)throw new UnsupportedOperationException();
        if(!host().isTaimResumed())throw new ForegroundException();
        if(!coarse() && !fine())throw new SecurityException();
        if(!enabled())throw new DisabledException();
        String accuracy=request.getString("accuracy");if(!accuracy.equals("high") && !accuracy.equals("balanced"))throw new IllegalArgumentException();
        String[] preferred=accuracy.equals("high")?new String[]{LocationManager.GPS_PROVIDER,LocationManager.NETWORK_PROVIDER}:new String[]{LocationManager.NETWORK_PROVIDER,LocationManager.GPS_PROVIDER};
        List<String> providers=manager.getAllProviders();for(String name:preferred)if((!name.equals(LocationManager.GPS_PROVIDER) || fine()) && providers.contains(name) && manager.isProviderEnabled(name))return name;
        throw new UnsupportedOperationException();
    }
    private static long maximumAge(JSONObject request)throws JSONException {long age=request.getLong("maximumAge");if(age<0 || age>86400000L)throw new IllegalArgumentException();return age;}
    private static long age(Location location){long elapsed=location.getElapsedRealtimeNanos(),now=SystemClock.elapsedRealtimeNanos();return elapsed<=0 || elapsed>now?Long.MAX_VALUE:(now-elapsed)/1000000L;}
    // The host also ships in apps that did not opt into location permissions.
    // Lint cannot prove that such a manifest makes these runtime guards deny.
    // Keep exemptions only on protected leaves, after a fresh explicit check.
    @android.annotation.SuppressLint("MissingPermission")
    private Location lastKnown(String provider){
        TaimActivity current=host();
        if(current.checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION)!=PackageManager.PERMISSION_GRANTED
            && current.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION)!=PackageManager.PERMISSION_GRANTED)throw new SecurityException("Location permission denied");
        return manager.getLastKnownLocation(provider);
    }
    @android.annotation.SuppressLint("MissingPermission")
    private void subscribe(String provider,long interval,float distance,LocationListener listener){
        TaimActivity current=host();
        if(current.checkSelfPermission(Manifest.permission.ACCESS_COARSE_LOCATION)!=PackageManager.PERMISSION_GRANTED
            && current.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION)!=PackageManager.PERMISSION_GRANTED)throw new SecurityException("Location permission denied");
        if(provider.equals(LocationManager.GPS_PROVIDER) && !fine())throw new SecurityException("Precise location permission denied");
        manager.requestLocationUpdates(provider,interval,distance,listener,Looper.getMainLooper());
    }
    JSONObject last(JSONObject request)throws Exception {
        String provider=provider(request);long maximumAge=maximumAge(request);if(maximumAge==0)return null;
        try{Location location=lastKnown(provider);return location!=null && age(location)<=maximumAge?fix(location):null;}catch(SecurityException revoked){throw new SecurityException("Location permission revoked",revoked);}
    }
    private JSONObject fix(Location location)throws JSONException {
        double latitude=location.getLatitude(),longitude=location.getLongitude();String provider=location.getProvider();
        if(!Double.isFinite(latitude) || latitude< -90 || latitude>90 || !Double.isFinite(longitude) || longitude< -180 || longitude>180
            || provider==null || provider.isEmpty() || provider.length()>64 || age(location)==Long.MAX_VALUE)return null;
        JSONObject value=new JSONObject().put("latitude",latitude).put("longitude",longitude)
            .put("accuracy_m",location.hasAccuracy() && Float.isFinite(location.getAccuracy()) && location.getAccuracy()>=0?location.getAccuracy():JSONObject.NULL)
            .put("altitude_m",location.hasAltitude() && Double.isFinite(location.getAltitude())?location.getAltitude():JSONObject.NULL)
            .put("speed_mps",location.hasSpeed() && Float.isFinite(location.getSpeed()) && location.getSpeed()>=0?location.getSpeed():JSONObject.NULL)
            .put("bearing_deg",location.hasBearing() && Float.isFinite(location.getBearing())?location.getBearing():JSONObject.NULL)
            .put("time_ms",Math.max(0,location.getTime())).put("elapsed_realtime_ns",location.getElapsedRealtimeNanos()).put("age_ms",age(location))
            .put("provider",provider).put("mocked",location.isFromMockProvider()).put("precise_permission",fine());
        return value;
    }
    Handle current(JSONObject request,Reply reply)throws Exception {
        String provider=provider(request);long maximumAge=maximumAge(request);int timeout=request.getInt("timeout");if(timeout<1 || timeout>60000)throw new IllegalArgumentException();
        return create(provider,false,maximumAge,timeout,0,0,reply);
    }
    JSONObject watch(JSONObject request)throws Exception {
        String provider=provider(request);int interval=request.getInt("interval"),duration=request.getInt("duration");double distance=request.getDouble("distance");
        if(interval<1000 || interval>60000 || duration<1 || duration>60000 || !Double.isFinite(distance) || distance<0 || distance>100000)throw new IllegalArgumentException();
        Handle handle=create(provider,true,0,duration,interval,(float)distance,null);
        return new JSONObject().put("watch",handle.id).put("provider",provider);
    }
    private Handle create(String provider,boolean watch,long maximumAge,int timeout,int interval,float distance,Reply reply){
        if(handles.size()>=4 || next==Long.MAX_VALUE)throw new RejectedExecutionException();
        Handle handle=new Handle(next++,provider,watch,maximumAge,timeout,interval,distance,reply);handles.put(handle.id,handle);main.post(handle::start);return handle;
    }
    void stop(long id){Handle handle=handles.get(id);if(handle!=null && handle.watch)handle.end("stopped",null);}
    void pause(){for(Handle handle:new ArrayList<>(handles.values()))handle.end("background",null);}
    void close(){for(Handle handle:new ArrayList<>(handles.values()))handle.end("unavailable",null);}
    private void event(JSONObject event){TaimActivity host=activity.get();if(host!=null)host.platformEvent(event);}

    final class Handle implements LocationListener {
        final long id,maximumAge,started=SystemClock.elapsedRealtimeNanos();final String provider;final boolean watch;
        final int timeout,interval;final float distance;final Reply reply;boolean closed;
        final Runnable deadline=()->end("expired",null);
        Handle(long id,String provider,boolean watch,long maximumAge,int timeout,int interval,float distance,Reply reply){this.id=id;this.provider=provider;this.watch=watch;this.maximumAge=maximumAge;this.timeout=timeout;this.interval=interval;this.distance=distance;this.reply=reply;}
        void start(){if(closed)return;try {
            TaimActivity current=host();
            if(!current.isTaimResumed()){end("background",null);return;}
            if(!enabled() || !manager.isProviderEnabled(provider)){end("disabled",null);return;}
            if((!fine() && !coarse()) || (provider.equals(LocationManager.GPS_PROVIDER) && !fine())){end("denied",null);return;}
            if(!watch && maximumAge>0){Location cached=lastKnown(provider);if(cached!=null && age(cached)<=maximumAge){JSONObject value=fix(cached);if(value!=null){end(null,value);return;}}}
            subscribe(provider,interval,distance,this);main.postDelayed(deadline,timeout);
        }catch(SecurityException denied){end("denied",null);}catch(Exception failed){end("failed",null);}}
        @Override public void onLocationChanged(Location location){if(closed)return;
            if((!fine() && !coarse()) || (provider.equals(LocationManager.GPS_PROVIDER) && !fine())){end("denied",null);return;}
            TaimActivity host=activity.get();if(host==null || !host.isTaimResumed()){end("background",null);return;}
            if(!enabled() || !manager.isProviderEnabled(provider)){end("disabled",null);return;}
            if((watch || maximumAge==0) && location.getElapsedRealtimeNanos()<started)return;
            if(!watch && maximumAge>0 && age(location)>maximumAge)return;
            try {JSONObject value=fix(location);if(value==null)return;
                if(watch)event(new JSONObject().put("type","location").put("watch",id).put("fix",value));else end(null,value);
            }catch(JSONException failed){end("failed",null);}
        }
        @Override public void onProviderDisabled(String name){end("disabled",null);}
        @Override public void onProviderEnabled(String name){}
        @Override public void onStatusChanged(String name,int status,Bundle extras){}
        void end(String reason,Object value){if(closed)return;close();
            if(watch){try{event(new JSONObject().put("type","locationEnded").put("watch",id).put("reason",reason==null?"stopped":reason));}catch(JSONException ignored){}}
            else reply.complete(value,reason==null?null:reason.equals("background")?"cancelledForeground":reason.equals("disabled")?"locationDisabled":reason.equals("expired")?"timeout":reason);
        }
        void close(){if(closed)return;closed=true;handles.remove(id);main.removeCallbacks(deadline);try{manager.removeUpdates(this);}catch(RuntimeException ignored){}}
    }
    static final class DisabledException extends Exception {}
    static final class ForegroundException extends Exception {}
    @SuppressWarnings("UseRequiresApi") // framework-only host: no AndroidX @RequiresApi
    @android.annotation.TargetApi(28)
    private static final class Api28 {static boolean enabled(LocationManager manager){return manager.isLocationEnabled();}}
}
