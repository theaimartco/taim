package com.theaimartco.taim.mediadriver;
import android.app.*;
import android.content.*;
import android.os.*;
import android.view.accessibility.*;
import java.io.*;

/** Isolated media integration driver. No personal library, camera or microphone is used by responders. */
public final class MediaDriver extends Instrumentation {
    private Activity activity;private int checks;
    @Override public void onCreate(Bundle args){super.onCreate(args);start();}
    private AccessibilityNodeInfo find(String label){AccessibilityNodeInfo root=getUiAutomation().getRootInActiveWindow();if(root==null)return null;for(AccessibilityNodeInfo n:root.findAccessibilityNodeInfosByText(label))if(label.contentEquals(n.getText()==null?"":n.getText()) || label.contentEquals(n.getContentDescription()==null?"":n.getContentDescription()))return n;return null;}
    private void waitNode(String label)throws Exception{long end=SystemClock.uptimeMillis()+30000;while(SystemClock.uptimeMillis()<end){if(find(label)!=null)return;Thread.sleep(100);}throw new AssertionError("Missing node: "+label);}
    private void click(String label)throws Exception{waitNode(label);if(!find(label).performAction(AccessibilityNodeInfo.ACTION_CLICK))throw new AssertionError("Rejected click: "+label);}
    private void mode(String mode){runOnMainSync(()->{try{activity.getClass().getMethod("setMediaMode",String.class).invoke(activity,mode);}catch(Exception e){throw new RuntimeException(e);}});}
    private int captures(){File[] files=new File(activity.getFilesDir(),"taim-media").listFiles();return files==null?0:files.length;}
    private void back()throws Exception{try(InputStream input=new ParcelFileDescriptor.AutoCloseInputStream(getUiAutomation().executeShellCommand("input keyevent KEYCODE_BACK"))){byte[] data=new byte[64];while(input.read(data)!=-1){}}}
    @Override public void onStart(){Bundle result=new Bundle();try {
        activity=startActivitySync(new Intent(Intent.ACTION_MAIN).setComponent(new ComponentName("com.theaimartco.taim.mediaprobe","com.theaimartco.taim.TaimMediaProbeActivity")).putExtra("taim.media.fixture",true).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        waitNode("Media ready");if(captures()!=0)throw new AssertionError("Fixture capture directory not empty");checks++;
        mode("normal");click("Pick images");waitNode("Media selected: 2");click("Inspect media");waitNode("Media inspected");checks++;
        click("Retain media");waitNode("Media error: Denied");click("Release media");waitNode("Media released");checks++;
        click("Pick visual");waitNode("Media selected: 2");checks++;
        click("Pick videos");waitNode("Media selected: 1");click("Inspect media");waitNode("Media inspected");checks++;
        mode("wrongMime");click("Pick images");waitNode("Media error: Invalid");checks++;
        mode("overcount");click("Pick one");waitNode("Media error: Invalid");checks++;
        mode("cancel");click("Pick images");waitNode("Media error: Denied");checks++;
        mode("slow");click("Check busy");waitNode("Media busy handled");if(captures()!=0)throw new AssertionError("Busy request leaked capture output");checks++;
        mode("cancel");click("Capture photo");waitNode("Media error: Denied");if(captures()!=0)throw new AssertionError("Cancelled capture leaked output");checks++;
        mode("normal");click("Capture photo");waitNode("Media captured: 1");click("List captures");waitNode("Media owned: 1");click("Inspect media");waitNode("Media inspected");if(captures()!=1)throw new AssertionError("Capture not retained");checks++;
        click("Preview media");waitNode("Media viewer verified");back();waitNode("Media preview dispatched");checks++;
        click("Delete capture");waitNode("Media deleted");if(captures()!=0)throw new AssertionError("Delete failed");checks++;
        click("Capture video");waitNode("Media captured: 1");click("Inspect media");waitNode("Media inspected");click("Delete capture");waitNode("Media deleted");if(captures()!=0)throw new AssertionError("Video delete failed");checks++;
        try{activity.getContentResolver().openInputStream(android.net.Uri.parse("content://com.theaimartco.taim.mediaprobe.taim.media/capture/../../secret"));throw new AssertionError("Provider accepted traversal");}catch(FileNotFoundException expected){}checks++;
        mode("system");click("Pick images");Thread.sleep(1500);back();waitNode("Media error: Denied");checks++;
        click("List captures");waitNode("Media owned: 0");checks++;
        mode("slow");click("Capture photo");long end=SystemClock.uptimeMillis()+5000;
        while(captures()==0 && SystemClock.uptimeMillis()<end)Thread.sleep(20);
        if(captures()!=1)throw new AssertionError("Pending capture not allocated");
        runOnMainSync(()->activity.finish());end=SystemClock.uptimeMillis()+10000;
        while(captures()!=0 && SystemClock.uptimeMillis()<end)Thread.sleep(100);
        if(captures()!=0)throw new AssertionError("Activity teardown leaked incomplete capture");checks++;
        result.putInt("checks",checks);result.putString("stream","Taim media integration passed\n");finish(Activity.RESULT_OK,result);
    }catch(Throwable error){result.putInt("checks",checks);result.putString("stream","Media fixture failed: "+error+"\n");finish(Activity.RESULT_CANCELED,result);}}
}
