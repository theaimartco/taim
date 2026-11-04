package com.theaimartco.taim.workdriver;
import android.app.*;
import android.app.job.*;
import android.content.*;
import android.os.*;
import android.view.accessibility.*;
import org.json.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.concurrent.*;

/** Only fixture jobs/files are mutated. The verify phase never launches an Activity. */
public final class WorkDriver extends Instrumentation {
    static final String PACKAGE="com.theaimartco.taim.workprobe";static final int BASE=0x54410000;
    Activity activity;String phase,generation;int oldPid,checks;
    @Override public void onCreate(Bundle args){super.onCreate(args);phase=args==null?"suite":args.getString("phase","suite");generation=args==null?"":args.getString("generation","");oldPid=args==null?0:Integer.parseInt(args.getString("oldPid","0"));start();}
    Object request(JSONObject value)throws Exception{Class<?> type=Class.forName("com.theaimartco.taim.TaimWork",true,getTargetContext().getClassLoader());Method method=type.getDeclaredMethod("request",Context.class,JSONObject.class);method.setAccessible(true);return method.invoke(null,getTargetContext(),value);}
    JSONObject info(int id)throws Exception{Object value=request(new JSONObject().put("op","workInfo").put("id",id));return value instanceof JSONObject?(JSONObject)value:null;}
    JSONObject state(int id,String expected)throws Exception{long end=SystemClock.uptimeMillis()+15000;JSONObject value=null;while(SystemClock.uptimeMillis()<end){value=info(id);if(value!=null && expected.equals(value.optString("state")))return value;Thread.sleep(100);}throw new AssertionError("Job "+id+" did not reach "+expected+"; last="+(value==null?"missing":value.optString("state"))+" error="+(value==null?"":value.optString("last_error")));}
    AccessibilityNodeInfo find(String label){AccessibilityNodeInfo root=getUiAutomation().getRootInActiveWindow();if(root==null)return null;for(AccessibilityNodeInfo n:root.findAccessibilityNodeInfosByText(label))if(label.contentEquals(n.getText()==null?"":n.getText()) || label.contentEquals(n.getContentDescription()==null?"":n.getContentDescription()))return n;return null;}
    void waitNode(String label)throws Exception{long end=SystemClock.uptimeMillis()+15000;while(SystemClock.uptimeMillis()<end){if(find(label)!=null)return;Thread.sleep(100);}throw new AssertionError("Missing node: "+label);}
    void click(String label)throws Exception{waitNode(label);if(!find(label).performAction(AccessibilityNodeInfo.ACTION_CLICK))throw new AssertionError("Action rejected: "+label);}
    void enqueue(String button,String label)throws Exception{click(button);waitNode("Work enqueued: "+label);}
    String shell(String cmd)throws Exception{try(InputStream input=new ParcelFileDescriptor.AutoCloseInputStream(getUiAutomation().executeShellCommand(cmd));ByteArrayOutputStream output=new ByteArrayOutputStream()){byte[] buffer=new byte[512];int count;while((count=input.read(buffer))!=-1)output.write(buffer,0,count);return output.toString("UTF-8");}}
    void force(int id)throws Exception{long end=SystemClock.uptimeMillis()+5000;String result="";while(SystemClock.uptimeMillis()<end){result=shell("cmd jobscheduler run -f "+PACKAGE+" "+(BASE|id));if(result.contains("Running job") && !result.toLowerCase().contains("already"))return;Thread.sleep(100);}throw new AssertionError("Could not force fixture job: "+result);}
    void marker(String prefix,int id,String generation)throws Exception{File file=new File(getTargetContext().getFilesDir(),prefix+id+"-"+generation);long end=SystemClock.uptimeMillis()+5000;while(!file.isFile() && SystemClock.uptimeMillis()<end)Thread.sleep(50);if(!file.isFile())throw new AssertionError("Worker marker missing");}
    boolean running(int id,String generation)throws Exception{Class<?> type=Class.forName("com.theaimartco.taim.TaimWorkService",true,getTargetContext().getClassLoader());Method method=type.getDeclaredMethod("running",int.class,String.class);method.setAccessible(true);return (Boolean)method.invoke(null,id,generation);}
    @Override public void onStart(){Bundle result=new Bundle();try{
        if(phase.equals("verify")){
            JSONObject row=state(17,"succeeded"),output=row.getJSONObject("output");
            if(!generation.equals(row.getString("generation")) || !output.getBoolean("headless") || output.getInt("pid")==oldPid || !output.getString("label").equals("headless"))throw new AssertionError("Headless generation/process mismatch");checks++;
            File file=new File(getTargetContext().getFilesDir(),"work-fixture-result-17-"+generation+".json");try(FileInputStream input=new FileInputStream(file);ByteArrayOutputStream bytes=new ByteArrayOutputStream()){byte[] buffer=new byte[1024];int n;while((n=input.read(buffer))!=-1)bytes.write(buffer,0,n);JSONObject saved=new JSONObject(bytes.toString("UTF-8"));if(!saved.getBoolean("headless") || saved.getInt("pid")!=output.getInt("pid"))throw new AssertionError("Headless file mismatch");}checks++;
            if(row.getBoolean("scheduled"))throw new AssertionError("Completed one-off job still scheduled");checks++;
            result.putInt("worker_pid",output.getInt("pid"));result.putString("stream","Taim headless process-restart work passed\n");
        }else{
            for(int id=1;id<=20;id++)request(new JSONObject().put("op","workCancel").put("id",id));request(new JSONObject().put("op","workPrune"));
            activity=startActivitySync(new Intent(Intent.ACTION_MAIN).setComponent(new ComponentName(PACKAGE,"com.theaimartco.taim.TaimActivity")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));waitNode("Work ready");
            enqueue("Enqueue success","first");JSONObject first=state(1,"queued");String original=first.getString("generation");enqueue("Keep existing","kept");JSONObject kept=info(1);if(!original.equals(kept.getString("generation")) || !kept.getJSONObject("input").getString("label").equals("first"))throw new AssertionError("Keep changed the active generation");checks++;
            force(1);JSONObject done=state(1,"succeeded");if(done.getInt("attempt")!=1 || !done.getJSONObject("output").getString("label").equals("first") || !done.getJSONObject("progress").getString("phase").equals("running"))throw new AssertionError("Success/progress mismatch");checks++;
            enqueue("Enqueue retry","retry");force(2);if(state(2,"retry").getInt("attempt")!=1)throw new AssertionError("Retry attempt mismatch");force(2);if(state(2,"succeeded").getInt("attempt")!=2)throw new AssertionError("Retry did not recover");checks++;
            enqueue("Exhaust attempts","exhausted");force(3);state(3,"retry");force(3);if(state(3,"failed").getInt("attempt")!=2)throw new AssertionError("Attempt bound ignored");checks++;
            enqueue("Permanent failure","failure");force(4);if(state(4,"failed").getInt("attempt")!=1)throw new AssertionError("Permanent failure retried");checks++;
            enqueue("Start old worker","old");force(1);String old=state(1,"running").getString("generation");enqueue("Replace worker","new");force(1);JSONObject replacement=state(1,"succeeded");if(old.equals(replacement.getString("generation")))throw new AssertionError("Replacement reused generation");Thread.sleep(2300);if(!info(1).getJSONObject("output").getString("label").equals("new"))throw new AssertionError("Stale result replaced new output");checks++;
            enqueue("Enqueue slow","slow");force(5);String cancelled=state(5,"running").getString("generation");click("Cancel current work");waitNode("Work cancelled");state(5,"cancelled");marker("work-fixture-stopped-",5,cancelled);checks++;
            enqueue("Enqueue timeout","timeout");force(6);JSONObject timed=state(6,"failed");marker("work-fixture-stopped-",6,timed.getString("generation"));checks++;
            enqueue("Enqueue periodic","periodic");JobScheduler scheduler=getTargetContext().getSystemService(JobScheduler.class);JobInfo periodic=scheduler.getPendingJob(BASE|7);
            if(periodic==null || !periodic.isPeriodic() || !periodic.isPersisted() || periodic.getIntervalMillis()!=900000 || periodic.getFlexMillis()!=300000 || !periodic.isRequireCharging() || !periodic.isRequireBatteryNotLow() || !periodic.isRequireStorageNotLow() || periodic.getNetworkType()!=JobInfo.NETWORK_TYPE_ANY || periodic.getInitialBackoffMillis()!=10000)throw new AssertionError("OS constraints/timing mismatch");
            force(7);long until=SystemClock.uptimeMillis()+15000;JSONObject cycle;
            do{cycle=info(7);if(cycle.getString("state").equals("queued") && cycle.getInt("runs")==1)break;Thread.sleep(100);}while(SystemClock.uptimeMillis()<until);
            if(cycle.getInt("attempt")!=0 || cycle.getInt("runs")!=1 || cycle.getJSONObject("output").opt("socket_bound")==Boolean.FALSE)throw new AssertionError("Periodic/network delivery mismatch");
            click("Cancel current work");waitNode("Work cancelled");state(7,"cancelled");checks++;
            enqueue("Enqueue recovery","recovery");String recovery=state(8,"queued").getString("generation");scheduler.cancel(BASE|8);click("Reconcile work");waitNode("Work reconciled");JobInfo recovered=scheduler.getPendingJob(BASE|8);if(recovered==null || !recovery.equals(recovered.getExtras().getString("generation")))throw new AssertionError("Missing scheduler entry not recovered");click("Cancel current work");waitNode("Work cancelled");state(8,"cancelled");checks++;
            enqueue("Enqueue prestart stop","prestart");String prestart=state(9,"queued").getString("generation");CountDownLatch entered=new CountDownLatch(1),release=new CountDownLatch(1);
            Class<?> service=Class.forName("com.theaimartco.taim.TaimWorkService",true,getTargetContext().getClassLoader());Method post=service.getDeclaredMethod("post",Runnable.class);post.setAccessible(true);
            try{post.invoke(null,(Runnable)()->{entered.countDown();try{release.await(10,TimeUnit.SECONDS);}catch(InterruptedException ignored){}});if(!entered.await(5,TimeUnit.SECONDS))throw new AssertionError("Manager fixture gate failed");force(9);long wait=SystemClock.uptimeMillis()+5000;while(!running(9,prestart) && SystemClock.uptimeMillis()<wait)Thread.sleep(20);if(!running(9,prestart))throw new AssertionError("Preclaim job not active");shell("cmd jobscheduler timeout "+PACKAGE+" "+(BASE|9));wait=SystemClock.uptimeMillis()+5000;while(running(9,prestart) && SystemClock.uptimeMillis()<wait)Thread.sleep(20);if(running(9,prestart))throw new AssertionError("OS stop did not detach preclaim job");}finally{release.countDown();}
            if(state(9,"retry").getInt("attempt")!=0)throw new AssertionError("Pre-start stop consumed an attempt");force(9);state(9,"succeeded");checks++;
            enqueue("Enqueue OS stop","osstop");force(10);String stopped=state(10,"running").getString("generation");shell("cmd jobscheduler timeout "+PACKAGE+" "+(BASE|10));state(10,"retry");marker("work-fixture-stopped-",10,stopped);click("Cancel current work");waitNode("Work cancelled");state(10,"cancelled");checks++;
            click("Prune history");waitNode("Work pruned");if(((JSONArray)request(new JSONObject().put("op","workList").put("finished",true))).length()!=0)throw new AssertionError("Terminal pruning left history");checks++;
            enqueue("Prepare headless","headless");JSONObject headless=state(17,"queued");result.putString("generation",headless.getString("generation"));result.putInt("suite_pid",android.os.Process.myPid());checks++;
            runOnMainSync(()->activity.finish());result.putString("stream","Taim background-work suite passed\n");
        }
        result.putInt("checks",checks);finish(Activity.RESULT_OK,result);
    }catch(Throwable error){result.putInt("checks",checks);result.putString("stream","Background work fixture failed: "+error+"\n");finish(Activity.RESULT_CANCELED,result);}}
}
