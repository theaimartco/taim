package com.theaimartco.taim;

import android.Manifest;
import android.app.PendingIntent;
import android.content.*;
import android.content.pm.PackageManager;
import android.location.LocationManager;
import android.os.Build;
import org.json.*;
import java.lang.ref.WeakReference;
import java.util.Iterator;

/**
 * Circular geofences using the framework LocationManager proximity alerts (no Play services).
 * Channel {@code taim.geofence}. Requires the opt-in {@link TaimGeofenceReceiver} manifest entry.
 *
 * Transitions are persisted (bounded ring of 64) so they survive the app process; a running
 * activity also receives them live. Android clears proximity alerts on reboot and app update;
 * call {@code restore} at startup to re-register the persisted regions.
 * Delivery while the app is not visible requires ACCESS_BACKGROUND_LOCATION.
 */
// commit(): a broadcast receiver's process may be killed right after onReceive, and regions must be
// durable before add() reports success.
@android.annotation.SuppressLint("ApplySharedPref")
final class TaimGeofence implements TaimPlugin {
    static final String CHANNEL="taim.geofence", PREFS="taim.geofence", EXTRA_ID="taim.geofence.id";
    private static final int MAX_REGIONS=100, MAX_EVENTS=64;
    static WeakReference<TaimActivity> live=new WeakReference<>(null);
    private final Context context;

    TaimGeofence(TaimActivity host){context=host.getApplicationContext();live=new WeakReference<>(host);}

    private SharedPreferences prefs(){return context.getSharedPreferences(PREFS,Context.MODE_PRIVATE);}
    private boolean fine(){return context.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION)==PackageManager.PERMISSION_GRANTED;}
    private boolean background(){return Build.VERSION.SDK_INT<29 || context.checkSelfPermission(Manifest.permission.ACCESS_BACKGROUND_LOCATION)==PackageManager.PERMISSION_GRANTED;}
    private static boolean receiverDeclared(Context context){
        try{context.getPackageManager().getReceiverInfo(new ComponentName(context,TaimGeofenceReceiver.class),0);return true;}
        catch(PackageManager.NameNotFoundException absent){return false;}
    }

    private PendingIntent intent(String id,int flags){
        Intent intent=new Intent(context,TaimGeofenceReceiver.class).setAction("com.theaimartco.taim.GEOFENCE."+id).putExtra(EXTRA_ID,id);
        // Mutable: the platform adds KEY_PROXIMITY_ENTERING. The intent is explicit to our own receiver.
        return PendingIntent.getBroadcast(context,id.hashCode(),intent,flags|(Build.VERSION.SDK_INT>=31?PendingIntent.FLAG_MUTABLE:0));
    }

    @Override public void call(String method,JSONObject args,Result result){
        try{
            LocationManager manager=context.getSystemService(LocationManager.class);
            if(manager==null || !receiverDeclared(context)){result.error("unsupported");return;}
            switch(method){
                case "status":result.success(new JSONObject().put("finePermission",fine()).put("backgroundPermission",background())
                    .put("enabled",Build.VERSION.SDK_INT<28 || manager.isLocationEnabled()).put("regions",regions().length()));return;
                case "add":{
                    if(!fine()){result.error("denied");return;}
                    String id=id(args.getString("id"));
                    double lat=args.getDouble("latitude"),lng=args.getDouble("longitude");float radius=(float)args.getDouble("radiusM");
                    long expiration=args.getLong("expirationMs");
                    if(lat<-90 || lat>90 || lng<-180 || lng>180 || !(radius>=50 && radius<=100000) || (expiration!=-1 && (expiration<1000 || expiration>365L*86400000L)))throw new IllegalArgumentException();
                    JSONObject regions=regions();
                    if(!regions.has(id) && regions.length()>=MAX_REGIONS){result.error("busy");return;}
                    long expiresAt=expiration==-1?-1:System.currentTimeMillis()+expiration;
                    manager.addProximityAlert(lat,lng,radius,expiration,intent(id,PendingIntent.FLAG_UPDATE_CURRENT));
                    regions.put(id,new JSONObject().put("latitude",lat).put("longitude",lng).put("radiusM",radius).put("expiresAt",expiresAt));
                    prefs().edit().putString("regions",regions.toString()).commit();
                    result.success(null);return;
                }
                case "remove":{
                    String id=id(args.getString("id"));
                    PendingIntent existing=intent(id,PendingIntent.FLAG_NO_CREATE);
                    if(existing!=null){manager.removeProximityAlert(existing);existing.cancel();}
                    JSONObject regions=regions();regions.remove(id);prefs().edit().putString("regions",regions.toString()).commit();
                    result.success(null);return;
                }
                case "list":result.success(regions());return;
                case "restore":{
                    if(!fine()){result.error("denied");return;}
                    JSONObject regions=regions(),kept=new JSONObject();long now=System.currentTimeMillis();int restored=0;
                    for(Iterator<String> keys=regions.keys();keys.hasNext();){
                        String id=keys.next();JSONObject r=regions.getJSONObject(id);long expiresAt=r.getLong("expiresAt");
                        if(expiresAt!=-1 && expiresAt<=now)continue;
                        manager.addProximityAlert(r.getDouble("latitude"),r.getDouble("longitude"),(float)r.getDouble("radiusM"),expiresAt==-1?-1:expiresAt-now,intent(id,PendingIntent.FLAG_UPDATE_CURRENT));
                        kept.put(id,r);restored++;
                    }
                    prefs().edit().putString("regions",kept.toString()).commit();
                    result.success(restored);return;
                }
                case "pending":{ // drain persisted transitions
                    JSONArray events=new JSONArray(prefs().getString("events","[]"));
                    prefs().edit().putString("events","[]").commit();
                    result.success(events);return;
                }
                default:result.error("invalid");
            }
        }catch(SecurityException denied){result.error("denied");}
        catch(JSONException | IllegalArgumentException bad){result.error("invalid");}
    }

    private JSONObject regions()throws JSONException{return new JSONObject(prefs().getString("regions","{}"));}
    private static String id(String id){if(!id.matches("[A-Za-z0-9._-]{1,64}"))throw new IllegalArgumentException();return id;}

    /** Called by the receiver on the main thread. Persist first, then deliver live if possible. */
    static void transition(Context context,String id,boolean entering){
        try{
            SharedPreferences prefs=context.getSharedPreferences(PREFS,Context.MODE_PRIVATE);
            JSONObject event=new JSONObject().put("type","transition").put("id",id).put("entering",entering).put("timeMs",System.currentTimeMillis());
            JSONArray events=new JSONArray(prefs.getString("events","[]"));events.put(event);
            JSONArray bounded=new JSONArray();for(int i=Math.max(0,events.length()-MAX_EVENTS);i<events.length();i++)bounded.put(events.get(i));
            prefs.edit().putString("events",bounded.toString()).commit();
            TaimActivity host=live.get();
            if(host!=null && !host.isFinishing())host.emitTaimPluginEvent(CHANNEL,event);
        }catch(JSONException ignored){}
    }
}
