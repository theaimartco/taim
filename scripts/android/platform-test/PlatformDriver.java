package com.theaimartco.taim.platformdriver;

import android.app.*;
import android.content.*;
import android.os.*;
import android.view.*;
import android.view.accessibility.*;
import android.view.autofill.AutofillValue;
import android.util.SparseArray;
import org.json.*;
import java.util.*;

/** Separate signed test APK; never packaged in production hosts. */
public final class PlatformDriver extends Instrumentation {
    private Activity activity;
    private int checks;
    private boolean notifications,cold;
    @Override public void onCreate(Bundle args){super.onCreate(args);notifications=args!=null && "true".equals(args.getString("notifications"));cold=args!=null && "true".equals(args.getString("cold"));start();}
    private AccessibilityNodeInfo find(String label){AccessibilityNodeInfo root=getUiAutomation().getRootInActiveWindow();if(root==null)return null;for(AccessibilityNodeInfo node:root.findAccessibilityNodeInfosByText(label))if(label.contentEquals(node.getText()==null?"":node.getText()) || label.contentEquals(node.getContentDescription()==null?"":node.getContentDescription()) || label.contentEquals(node.getHintText()==null?"":node.getHintText()))return node;return null;}
    private AccessibilityNodeInfo waitNode(String label)throws Exception{long end=SystemClock.uptimeMillis()+30000;while(SystemClock.uptimeMillis()<end){AccessibilityNodeInfo node=find(label);if(node!=null)return node;Thread.sleep(100);}throw new AssertionError("Missing node: "+label);}
    private void click(String label)throws Exception{
        long end=SystemClock.uptimeMillis()+5000;AccessibilityNodeInfo node=null;
        while(SystemClock.uptimeMillis()<end){node=waitNode(label);if(node.performAction(AccessibilityNodeInfo.ACTION_CLICK))return;Thread.sleep(100);}
        throw new AssertionError("Action rejected: "+label+" class="+node.getClassName()+" actions="+node.getActions()+" enabled="+node.isEnabled()+" visible="+node.isVisibleToUser());
    }
    private View virtual(View view){if(view.getClass().getSimpleName().equals("TaimAccessibility"))return view;if(view instanceof ViewGroup){ViewGroup group=(ViewGroup)view;for(int i=0;i<group.getChildCount();i++){View result=virtual(group.getChildAt(i));if(result!=null)return result;}}return null;}
    @Override public void onStart(){Bundle result=new Bundle();try{
        Intent intent=new Intent(Intent.ACTION_MAIN).setComponent(new ComponentName("com.theaimartco.taim.platformprobe","com.theaimartco.taim.TaimProbeActivity")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        if(cold)intent.putExtra("com.theaimartco.taim.notification.ID",4103).putExtra("com.theaimartco.taim.notification.PAYLOAD","cold");
        activity=startActivitySync(intent);
        if(cold){waitNode("Notification route: 4103:cold");result.putString("stream","Taim notification cold-intent replay passed\n");result.putInt("checks",1);finish(Activity.RESULT_OK,result);return;}
        waitNode("Count: 0");checks++;
        click("Increment");waitNode("Count: 1");checks++;
        click("Run services");waitNode("Services passed");checks++;
        click("Open page");waitNode("Page open");getUiAutomation().executeShellCommand("input keyevent KEYCODE_BACK").close();waitNode("Page closed");checks++;
        Bundle text=new Bundle();text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,"Hello नमस्ते🌍");
        if(!waitNode("Probe input").performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,text))throw new AssertionError("SET_TEXT rejected");waitNode("Input: Hello नमस्ते🌍");checks++;
        AccessibilityNodeInfo password=waitNode("Probe password");if(!password.isPassword() || password.getText()!=null)throw new AssertionError("Secret disclosure");checks++;
        final Throwable[] failure={null};runOnMainSync(()->{try{
            View view=virtual(activity.getWindow().getDecorView());if(view==null)throw new AssertionError("Virtual host missing");
            java.lang.reflect.Field field=view.getClass().getDeclaredField("nodes");field.setAccessible(true);
            Map<?,?> nodes=(Map<?,?>)field.get(view);Integer target=null;
            for(Object node:nodes.values()){JSONObject value=(JSONObject)node;if(value.optString("name").equals("Probe password"))target=value.getInt("id");}
            if(target==null)throw new AssertionError("Autofill field missing");SparseArray<AutofillValue> values=new SparseArray<>();values.put(target,AutofillValue.forText("test-only-autofill"));view.autofill(values);
        }catch(Throwable error){failure[0]=error;}});if(failure[0]!=null)throw new AssertionError("Autofill dispatch failed",failure[0]);
        waitNode("Secure autofill passed");password=waitNode("Probe password");if(password.getText()!=null)throw new AssertionError("Autofill disclosed secret");checks++;
        runOnMainSync(()->activity.onTrimMemory(80));Thread.sleep(1000);waitNode("Count: 1");waitNode("Input: Hello नमस्ते🌍");checks++;
        if(notifications){
            click("Run notifications");waitNode("Notifications denied passed");checks++;
            getUiAutomation().grantRuntimePermission("com.theaimartco.taim.platformprobe","android.permission.POST_NOTIFICATIONS");
            android.app.NotificationManager manager=activity.getSystemService(android.app.NotificationManager.class);
            long permissionUntil=SystemClock.uptimeMillis()+5000;
            while(!manager.areNotificationsEnabled() && SystemClock.uptimeMillis()<permissionUntil)Thread.sleep(100);
            if(!manager.areNotificationsEnabled())throw new AssertionError("Notification permission not effective");
            click("Run notifications");waitNode("Notifications posted");checks++;
            android.service.notification.StatusBarNotification[] active=manager.getActiveNotifications();
            long postedUntil=SystemClock.uptimeMillis()+5000;
            while(active.length!=2 && SystemClock.uptimeMillis()<postedUntil){Thread.sleep(100);active=manager.getActiveNotifications();}
            if(active.length!=2 || manager.getNotificationChannel("probe.notifications.v1").getImportance()!=2)throw new AssertionError("Notification/channel mismatch");
            android.app.PendingIntent tap=null;
            for(android.service.notification.StatusBarNotification entry:active){
                if(entry.getNotification().visibility!=android.app.Notification.VISIBILITY_PRIVATE || !entry.getNotification().extras.getString(android.app.Notification.EXTRA_TITLE).equals("Taim fixture "+entry.getId()))throw new AssertionError("Notification content/policy mismatch");
                if(entry.getId()==4102)tap=entry.getNotification().contentIntent;
            }
            if(tap==null)throw new AssertionError("Tap PendingIntent missing");checks++;
            tap.send();waitNode("Notification route: 4102:route:4102");checks++;
            // A fresh service subscription must replay the last launch intent too.
            click("Replay notification");waitNode("Notification replay: 4102:route:4102");checks++;
            click("Cancel notifications");waitNode("Notifications cancelled");long until=SystemClock.uptimeMillis()+5000;
            while(manager.getActiveNotifications().length!=0 && SystemClock.uptimeMillis()<until)Thread.sleep(100);
            if(manager.getActiveNotifications().length!=0)throw new AssertionError("Notifications not cancelled");checks++;
        }
        result.putString("stream","Taim Android platform checks passed\n");result.putInt("checks",checks);finish(Activity.RESULT_OK,result);
    }catch(Throwable error){result.putString("stream","Platform check failed: "+error+"\n");result.putInt("checks",checks);finish(Activity.RESULT_CANCELED,result);}}
}
