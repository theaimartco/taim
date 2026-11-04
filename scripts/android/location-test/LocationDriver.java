package com.theaimartco.taim.locationdriver;
import android.app.*;
import android.content.*;
import android.location.*;
import android.os.*;
import android.view.accessibility.*;
import java.io.*;
import java.util.Map;

/** Emulator-only synthetic LocationManager-provider tests. No coordinates are logged. */
public final class LocationDriver extends Instrumentation {
    private static final String PACKAGE="com.theaimartco.taim.locationprobe";
    private Activity activity;private LocationManager manager;private int checks;private boolean gps,network;
    @Override public void onCreate(Bundle args){super.onCreate(args);start();}
    private AccessibilityNodeInfo find(String label){AccessibilityNodeInfo root=getUiAutomation().getRootInActiveWindow();if(root==null)return null;for(AccessibilityNodeInfo n:root.findAccessibilityNodeInfosByText(label))if(label.contentEquals(n.getText()==null?"":n.getText()) || label.contentEquals(n.getContentDescription()==null?"":n.getContentDescription()))return n;return null;}
    private void waitNode(String label)throws Exception{long end=SystemClock.uptimeMillis()+15000;while(SystemClock.uptimeMillis()<end){if(find(label)!=null)return;Thread.sleep(100);}throw new AssertionError("Missing node: "+label);}
    private void click(String label)throws Exception{waitNode(label);if(!find(label).performAction(AccessibilityNodeInfo.ACTION_CLICK))throw new AssertionError("Rejected click: "+label);}
    private void shell(String cmd)throws Exception{try(InputStream input=new ParcelFileDescriptor.AutoCloseInputStream(getUiAutomation().executeShellCommand(cmd))){byte[] data=new byte[256];while(input.read(data)!=-1){}}}
    private void active(int count)throws Exception{long end=SystemClock.uptimeMillis()+5000;do{click("Refresh location status");Thread.sleep(100);if(find("Location active: "+count)!=null)return;}while(SystemClock.uptimeMillis()<end);throw new AssertionError("Native location session count mismatch: "+count);}
    private void feed(String provider,long ageMs){Location location=new Location(provider);location.setLatitude(1.25);location.setLongitude(2.5);location.setAccuracy(3f);location.setAltitude(12);location.setSpeed(2);location.setBearing(60);location.setTime(System.currentTimeMillis()-ageMs);location.setElapsedRealtimeNanos(SystemClock.elapsedRealtimeNanos()-ageMs*1000000L);manager.setTestProviderLocation(provider,location);}
    private void add(String provider,boolean coarse){manager.addTestProvider(provider,false,false,false,false,true,true,true,Criteria.POWER_LOW,coarse?Criteria.ACCURACY_COARSE:Criteria.ACCURACY_FINE);manager.setTestProviderEnabled(provider,true);}
    private void reopen()throws Exception{shell("am start -W -n "+PACKAGE+"/com.theaimartco.taim.TaimActivity");}
    private int nativeActive()throws Exception {
        java.lang.reflect.Field services=activity.getClass().getDeclaredField("services");services.setAccessible(true);Object client=services.get(activity);
        java.lang.reflect.Field locations=client.getClass().getDeclaredField("locations");locations.setAccessible(true);Object location=locations.get(client);
        java.lang.reflect.Field handles=location.getClass().getDeclaredField("handles");handles.setAccessible(true);return ((Map<?,?>)handles.get(location)).size();
    }
    @Override public void onStart(){Bundle result=new Bundle();int code=Activity.RESULT_CANCELED;String message="Location fixture did not run";
        try {
            activity=startActivitySync(new Intent(Intent.ACTION_MAIN).setComponent(new ComponentName(PACKAGE,"com.theaimartco.taim.TaimActivity")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            waitNode("Location ready");active(0);waitNode("Location permission: denied");click("Current location");waitNode("Location error: Denied");checks++;
            getUiAutomation().grantRuntimePermission(PACKAGE,"android.permission.ACCESS_COARSE_LOCATION");
            active(0);waitNode("Location permission: approximate");manager=activity.getSystemService(LocationManager.class);
            add(LocationManager.GPS_PROVIDER,false);gps=true;add(LocationManager.NETWORK_PROVIDER,true);network=true;
            click("Current location");active(1);feed(LocationManager.NETWORK_PROVIDER,0);waitNode("Location current approximate");waitNode("Location mock: true");active(0);checks++;
            getUiAutomation().grantRuntimePermission(PACKAGE,"android.permission.ACCESS_FINE_LOCATION");active(0);waitNode("Location permission: precise");
            click("Current location");active(1);feed(LocationManager.GPS_PROVIDER,0);waitNode("Location current precise");active(0);checks++;
            feed(LocationManager.GPS_PROVIDER,0);Thread.sleep(100);click("Cached location");waitNode("Location cached");active(0);checks++;
            // Android refuses to replace a newer cached sample with an older injected timestamp.
            Thread.sleep(1200);click("Cached location");waitNode("Location cache empty");checks++;
            click("Location timeout");waitNode("Location error: Timeout");active(0);checks++;
            click("Drop current location");waitNode("Location current dropped");active(0);checks++;
            click("Watch location");waitNode("Location watching");active(1);feed(LocationManager.GPS_PROVIDER,0);waitNode("Location samples: 1");click("Stop location");waitNode("Location ended: Stopped");active(0);checks++;
            click("Drop location watch");waitNode("Location watch dropped");active(0);checks++;
            click("Watch short");waitNode("Location ended: Expired");active(0);checks++;
            click("Location capacity");waitNode("Location capacity handled");active(0);checks++;
            click("Watch location");active(1);manager.setTestProviderEnabled(LocationManager.GPS_PROVIDER,false);waitNode("Location ended: Disabled");active(0);manager.setTestProviderEnabled(LocationManager.GPS_PROVIDER,true);checks++;
            shell("cmd location set-location-enabled false --user 0");click("Current location");waitNode("Location error: LocationDisabled");active(0);shell("cmd location set-location-enabled true --user 0");manager.setTestProviderEnabled(LocationManager.GPS_PROVIDER,true);checks++;
            click("Watch location");active(1);shell("input keyevent KEYCODE_HOME");Thread.sleep(300);feed(LocationManager.GPS_PROVIDER,0);reopen();waitNode("Location ended: Background");active(0);waitNode("Location samples: 1");checks++;
            click("Current location");active(1);shell("input keyevent KEYCODE_HOME");Thread.sleep(300);reopen();waitNode("Location error: Cancelled");active(0);checks++;
            click("Watch location");active(1);runOnMainSync(()->activity.finish());long end=SystemClock.uptimeMillis()+5000;
            while(nativeActive()!=0 && SystemClock.uptimeMillis()<end)Thread.sleep(100);if(nativeActive()!=0)throw new AssertionError("Activity teardown retained a location listener");checks++;
            code=Activity.RESULT_OK;message="Taim foreground location integration passed\n";
        }catch(Throwable error){message="Location fixture failed: "+error+"\n";}
        finally {
            try {if(gps)manager.removeTestProvider(LocationManager.GPS_PROVIDER);if(network)manager.removeTestProvider(LocationManager.NETWORK_PROVIDER);}
            catch(RuntimeException cleanup){code=Activity.RESULT_CANCELED;message+="Mock provider cleanup failed\n";}
            result.putInt("checks",checks);result.putString("stream",message);finish(code,result);
        }
    }
}
