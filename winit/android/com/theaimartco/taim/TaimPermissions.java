package com.theaimartco.taim;
import android.content.*;
import android.content.pm.*;
import android.net.Uri;
import android.os.*;
import android.provider.Settings;
import org.json.*;
import java.util.*;

/** Public-API permission observations; no hidden APIs or privileged flag reads. */
// commit(): request history must survive an immediate process kill (process-restart tests lost apply() writes).
@android.annotation.SuppressLint("ApplySharedPref")
final class TaimPermissions {
    private final TaimActivity host;private final SharedPreferences history;
    private final LinkedHashMap<String,String> observed=new LinkedHashMap<>();
    private final Set<String> declared=new HashSet<>();private String[] pending;private int pendingCode;
    TaimPermissions(TaimActivity host){this.host=host;history=host.getSharedPreferences("taim.permissions.v1",Context.MODE_PRIVATE);
        try{String[] names=host.getPackageManager().getPackageInfo(host.getPackageName(),PackageManager.GET_PERMISSIONS).requestedPermissions;if(names!=null)declared.addAll(Arrays.asList(names));}catch(PackageManager.NameNotFoundException ignored){}
    }
    private JSONObject inspect(String name)throws JSONException {
        boolean manifest=declared.contains(name),available=true,runtime=false;
        try{PermissionInfo info=host.getPackageManager().getPermissionInfo(name,0);runtime=(info.protectionLevel&PermissionInfo.PROTECTION_MASK_BASE)==PermissionInfo.PROTECTION_DANGEROUS;}catch(PackageManager.NameNotFoundException absent){available=false;}
        boolean granted=available && manifest && host.checkSelfPermission(name)==PackageManager.PERMISSION_GRANTED;
        boolean rationale=!granted && manifest && runtime && host.shouldShowRequestPermissionRationale(name);
        boolean requested=history.getBoolean("requested:"+name,false),denied=history.getBoolean("denied:"+name,false),cancelled=history.getBoolean("cancelled:"+name,false);
        if(granted && denied){history.edit().remove("denied:"+name).commit();denied=false;}
        String state=granted?"granted":!available?"unavailable":!manifest?"notDeclared":!runtime?"restricted":rationale?"denied":denied?"settingsRequired":cancelled?"cancelled":requested?"denied":"notRequested";
        boolean canAsk=!granted && available && manifest && runtime && (!denied || rationale);
        return new JSONObject().put("name",name).put("granted",granted).put("declared",manifest).put("available",available).put("runtime",runtime).put("requestedBefore",requested).put("lastRequestCancelled",cancelled).put("shouldShowRationale",rationale).put("canAskAgain",canAsk).put("settingsRequired",state.equals("settingsRequired")).put("state",state);
    }
    JSONArray status(String[] names)throws JSONException {JSONArray values=new JSONArray();for(String name:names){JSONObject value=inspect(name);values.put(value);if(!observed.containsKey(name) && observed.size()<128)observed.put(name,value.toString());}return values;}
    boolean busy(){return pending!=null;}
    String[] requestable(String[] names)throws JSONException {JSONArray values=status(names);ArrayList<String> request=new ArrayList<>();for(int i=0;i<values.length();i++)if(values.getJSONObject(i).getBoolean("canAskAgain"))request.add(names[i]);return request.toArray(new String[0]);}
    void begin(int code,String[] names){if(pending!=null)throw new IllegalStateException();pending=names.clone();pendingCode=code;SharedPreferences.Editor edit=history.edit();for(String name:names)edit.putBoolean("requested:"+name,true).remove("cancelled:"+name);edit.commit();}
    void launchFailed(int code){if(pendingCode==code){pending=null;pendingCode=0;}}
    boolean finish(int code,String[] names,int[] grants){if(pending==null || pendingCode!=code)return false;
        // Empty/mismatched callbacks are cancellation, not evidence of permanent denial.
        SharedPreferences.Editor edit=history.edit();Set<String> answered=new HashSet<>();Set<String> expected=new HashSet<>(Arrays.asList(pending));
        if(names.length==grants.length){for(int i=0;i<names.length;i++)if(expected.contains(names[i])){answered.add(names[i]);edit.putBoolean("denied:"+names[i],grants[i]!=PackageManager.PERMISSION_GRANTED).remove("cancelled:"+names[i]);}}
        for(String name:pending)if(!answered.contains(name))edit.putBoolean("cancelled:"+name,true);edit.commit();
        pending=null;pendingCode=0;refresh();return true;
    }
    void refresh(){try{JSONArray changed=new JSONArray();for(String name:new ArrayList<>(observed.keySet())){JSONObject value=inspect(name);String encoded=value.toString();if(!encoded.equals(observed.get(name))){observed.put(name,encoded);changed.put(value);}}if(changed.length()>0)host.platformEvent(new JSONObject().put("type","permissionsChanged").put("permissions",changed));}catch(JSONException ignored){}}
    JSONObject special(String kind,boolean open)throws JSONException {
        boolean supported=true,granted;String action;
        switch(kind){
            case "overlay":granted=Settings.canDrawOverlays(host);action=Settings.ACTION_MANAGE_OVERLAY_PERMISSION;break;
            case "writeSettings":granted=Settings.System.canWrite(host);action=Settings.ACTION_MANAGE_WRITE_SETTINGS;break;
            case "installPackages":try{granted=host.getPackageManager().canRequestPackageInstalls();}catch(SecurityException denied){granted=false;}action=Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES;break;
            case "allFiles":supported=Build.VERSION.SDK_INT>=30;granted=Build.VERSION.SDK_INT>=30 && Environment.isExternalStorageManager();action="android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION";break;
            case "exactAlarms":supported=Build.VERSION.SDK_INT>=31;granted=Build.VERSION.SDK_INT<31 || host.getSystemService(android.app.AlarmManager.class).canScheduleExactAlarms();action="android.settings.REQUEST_SCHEDULE_EXACT_ALARM";break;
            case "batteryOptimization":granted=host.getSystemService(PowerManager.class).isIgnoringBatteryOptimizations(host.getPackageName());action=Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS;break;
            case "fullScreenIntent":supported=Build.VERSION.SDK_INT>=34;granted=Build.VERSION.SDK_INT>=34 && host.getSystemService(android.app.NotificationManager.class).canUseFullScreenIntent();action="android.settings.MANAGE_APP_USE_FULL_SCREEN_INTENT";break;
            default:throw new IllegalArgumentException();
        }
        if(open){if(!supported)throw new UnsupportedOperationException();Intent intent=new Intent(action);if(!kind.equals("batteryOptimization"))intent.setData(Uri.fromParts("package",host.getPackageName(),null));host.startActivity(intent);}
        return new JSONObject().put("kind",kind).put("supported",supported).put("granted",granted);
    }
}
