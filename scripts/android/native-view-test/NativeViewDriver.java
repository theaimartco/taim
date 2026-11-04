package com.theaimartco.taim.nativeviewdriver;
import android.app.*;
import android.content.*;
import android.graphics.*;
import android.os.*;
import android.view.*;
import android.view.accessibility.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import org.json.*;

/** Real native hierarchy input, pixels and lifecycle; reflection is fixture-only. */
public final class NativeViewDriver extends Instrumentation {
    private final String pkg="com.theaimartco.taim.nativeviewprobe";
    private Activity activity;private int checks;private boolean webTests,sensorTests;
    @Override public void onCreate(Bundle args){super.onCreate(args);webTests=args!=null && "true".equals(args.getString("web"));sensorTests=args!=null && "true".equals(args.getString("sensors"));start();}
    private AccessibilityNodeInfo findIn(AccessibilityNodeInfo node,String label){if(node==null)return null;if(label.contentEquals(node.getText()==null?"":node.getText()) || label.contentEquals(node.getHintText()==null?"":node.getHintText()) || label.contentEquals(node.getContentDescription()==null?"":node.getContentDescription()))return node;for(int i=0;i<node.getChildCount();i++){AccessibilityNodeInfo found=findIn(node.getChild(i),label);if(found!=null)return found;}return null;}
    private AccessibilityNodeInfo find(String label){return findIn(getUiAutomation().getRootInActiveWindow(),label);}
    private AccessibilityNodeInfo waitNode(String label)throws Exception{long end=SystemClock.uptimeMillis()+15000;do{AccessibilityNodeInfo node=find(label);if(node!=null)return node;Thread.sleep(100);}while(SystemClock.uptimeMillis()<end);throw new AssertionError("Missing node: "+label);}
    private void click(String label)throws Exception{if(!waitNode(label).performAction(AccessibilityNodeInfo.ACTION_CLICK))throw new AssertionError("Rejected click: "+label);Thread.sleep(200);}
    private void shell(String command)throws Exception{try(InputStream input=new ParcelFileDescriptor.AutoCloseInputStream(getUiAutomation().executeShellCommand(command))){byte[] data=new byte[256];while(input.read(data)!=-1){}}}
    private int count(String name)throws Exception{return activity.getClass().getField(name).getInt(activity);}
    private Object field(Object owner,String name)throws Exception{Class<?> c=owner.getClass();while(c!=null){try{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(owner);}catch(NoSuchFieldException e){c=c.getSuperclass();}}throw new NoSuchFieldException(name);}
    private View nativeChild(Class<?> type,View root){if(type.isInstance(root))return root;if(root instanceof ViewGroup){ViewGroup group=(ViewGroup)root;for(int i=0;i<group.getChildCount();i++){View found=nativeChild(type,group.getChildAt(i));if(found!=null)return found;}}return null;}
    private int pixel(AccessibilityNodeInfo node)throws Exception{Rect rect=new Rect();node.getBoundsInScreen(rect);Bitmap image=getUiAutomation().takeScreenshot();if(image==null)throw new AssertionError("No screenshot");int color=image.getPixel(rect.centerX(),rect.bottom-12);image.recycle();return color;}
    private String nodes(AccessibilityNodeInfo node){if(node==null)return "null";String s="["+node.getClassName()+":"+node.getText()+":"+node.getContentDescription()+":"+node.getHintText()+"]";for(int i=0;i<node.getChildCount();i++)s+=nodes(node.getChild(i));return s;}
    private void invoke(Object object,String name,Class<?> type,Object argument){runOnMainSync(()->{try{Method method=object.getClass().getDeclaredMethod(name,type);method.setAccessible(true);method.invoke(object,argument);}catch(Exception error){throw new RuntimeException(error);}});}
    private void nativeKey(Object manager,long id,long generation,int code,boolean pressed,boolean repeat,String text){runOnMainSync(()->{try{Method key=manager.getClass().getDeclaredMethod("key",long.class,long.class,int.class,boolean.class,boolean.class,String.class);key.setAccessible(true);key.invoke(manager,id,generation,code,pressed,repeat,text);}catch(Exception error){throw new RuntimeException(error);}});}
    private void require(boolean condition,String error){if(!condition)throw new AssertionError(error);checks++;android.util.Log.i("TaimNativeKeys","check "+checks+": "+error);}
    private void sensors()throws Exception {
        waitNode("Sensor sample ready");require(true,"Public typed Rust sensor API receives a validated real sample through JNI services");
        Object sensors=field(field(activity,"services"),"sensors");Class<?> type=sensors.getClass();Method inventory=type.getDeclaredMethod("inventory");inventory.setAccessible(true);
        final JSONArray[] available=new JSONArray[1];runOnMainSync(()->{try{available[0]=(JSONArray)inventory.invoke(sensors);}catch(Exception e){throw new RuntimeException(e);}});
        boolean accelerometer=false;for(int i=0;i<available[0].length();i++)if(available[0].getJSONObject(i).getInt("type")==1)accelerometer=true;
        require(accelerometer && available[0].length()<=13,"Sensor inventory exposes bounded default sensors on the dedicated emulator");
        Class<?> replyType=Class.forName("com.theaimartco.taim.TaimSensors$Reply",true,activity.getClassLoader());Method read=type.getDeclaredMethod("read",JSONObject.class,replyType);read.setAccessible(true);
        java.util.concurrent.CountDownLatch delivered=new java.util.concurrent.CountDownLatch(1);java.util.concurrent.atomic.AtomicReference<Object[]> result=new java.util.concurrent.atomic.AtomicReference<>();
        Object reply=Proxy.newProxyInstance(replyType.getClassLoader(),new Class<?>[]{replyType},(proxy,method,args)->{if(method.getName().equals("complete")){result.set(args);delivered.countDown();}return null;});
        final Object[] handle=new Object[1];runOnMainSync(()->{try{handle[0]=read.invoke(sensors,new JSONObject().put("sensor",1).put("timeout",5000),reply);}catch(Exception e){throw new RuntimeException(e);}});
        if(!delivered.await(6,java.util.concurrent.TimeUnit.SECONDS))throw new AssertionError("Real accelerometer sample missing");
        Object[] value=result.get();JSONObject sample=value[0] instanceof JSONObject?(JSONObject)value[0]:null;
        require(value[1]==null && sample!=null && sample.getInt("sensor")==1 && sample.getLong("timestamp_ns")>0 && sample.getJSONArray("values").length()==3,"Real Android accelerometer snapshot delivered with monotonic timestamp");
        require((Boolean)field(handle[0],"closed") && ((Set<?>)field(sensors,"active")).isEmpty(),"Completed sensor snapshot unregisters its listener");
        final int[] cancelled=new int[1];Object cancelReply=Proxy.newProxyInstance(replyType.getClassLoader(),new Class<?>[]{replyType},(proxy,method,args)->{if(method.getName().equals("complete") && "cancelledForeground".equals(args[1]))cancelled[0]++;return null;});
        final boolean[] busy=new boolean[1];runOnMainSync(()->{try{for(int i=0;i<8;i++)read.invoke(sensors,new JSONObject().put("sensor",1).put("timeout",5000),cancelReply);try{read.invoke(sensors,new JSONObject().put("sensor",1).put("timeout",5000),cancelReply);}catch(InvocationTargetException full){busy[0]=full.getCause() instanceof java.util.concurrent.RejectedExecutionException;}Method pause=type.getDeclaredMethod("pause");pause.setAccessible(true);pause.invoke(sensors);}catch(Exception e){throw new RuntimeException(e);}});
        require(busy[0],"Sensor adapter rejects a ninth concurrent listener");
        require(cancelled[0]==8 && ((Set<?>)field(sensors,"active")).isEmpty(),"Foreground sensor cancellation completes and unregisters all active listeners");
        final int[] rejected=new int[1];runOnMainSync(()->{try{for(JSONObject request:new JSONObject[]{new JSONObject().put("sensor",21).put("timeout",1000),new JSONObject().put("sensor",1).put("timeout",99),new JSONObject().put("sensor",1).put("timeout",10001)})try{read.invoke(sensors,request,reply);}catch(InvocationTargetException invalid){if(invalid.getCause() instanceof IllegalArgumentException)rejected[0]++;}}catch(Exception e){throw new RuntimeException(e);}});
        require(rejected[0]==3 && ((Set<?>)field(sensors,"active")).isEmpty(),"Protected sensor types and unbounded deadlines rejected before registration");
        final Object[] expired=new Object[1];final boolean[] timedOut=new boolean[1];Object timeoutReply=Proxy.newProxyInstance(replyType.getClassLoader(),new Class<?>[]{replyType},(proxy,method,args)->{if(method.getName().equals("complete"))timedOut[0]=args[0]==null && "timeout".equals(args[1]);return null;});
        runOnMainSync(()->{try{expired[0]=read.invoke(sensors,new JSONObject().put("sensor",1).put("timeout",100),timeoutReply);((Runnable)field(expired[0],"deadline")).run();}catch(Exception e){throw new RuntimeException(e);}});
        require(timedOut[0] && (Boolean)field(expired[0],"closed") && ((Set<?>)field(sensors,"active")).isEmpty(),"Sensor deadline completes with timeout and unregisters the listener");
        shell("input keyevent KEYCODE_HOME");long until=SystemClock.uptimeMillis()+5000;while((Boolean)field(activity,"resumed") && SystemClock.uptimeMillis()<until)Thread.sleep(50);
        final boolean[] background=new boolean[1];runOnMainSync(()->{try{read.invoke(sensors,new JSONObject().put("sensor",1).put("timeout",1000),reply);}catch(InvocationTargetException paused){background[0]=paused.getCause().getClass().getSimpleName().equals("ForegroundException");}catch(Exception e){throw new RuntimeException(e);}});
        require(background[0] && ((Set<?>)field(sensors,"active")).isEmpty(),"Actual paused activity cannot acquire sensor samples");
        shell("am start -W -n "+pkg+"/com.theaimartco.taim.TaimNativeProbeActivity");waitNode("Native button");
    }
    private void web(Object manager,Map<Long,Object> entries)throws Exception {
        JSONObject properties=new JSONObject().put("html","<html><body><h1>Web adapter ready</h1><script>document.querySelector('h1').textContent='Script executed'</script></body></html>");
        JSONObject descriptor=new JSONObject().put("id",888888).put("factory","taim.web").put("properties",properties).put("enabled",true).put("bounds",new JSONArray(new int[]{40,300,900,400})).put("clip",new JSONArray(new int[]{40,300,900,400}));
        invoke(manager,"publish",String.class,new JSONObject().put("views",new JSONArray().put(descriptor)).toString());waitNode("Web adapter ready");
        Object instance=field(entries.get(888888L),"instance");android.webkit.WebView view=(android.webkit.WebView)field(instance,"web");
        final boolean[] settings=new boolean[1];runOnMainSync(()->{android.webkit.WebSettings s=view.getSettings();settings[0]=!s.getJavaScriptEnabled() && !s.getAllowFileAccess() && !s.getAllowContentAccess() && s.getBlockNetworkLoads() && s.getMixedContentMode()==android.webkit.WebSettings.MIXED_CONTENT_NEVER_ALLOW && !s.getDomStorageEnabled();});
        require(settings[0] && find("Script executed")==null,"Inline WebView rendered accessible HTML with scripting/network/file access disabled");
        Object policy=field(instance,"policy");Method allows=policy.getClass().getDeclaredMethod("allows",String.class);allows.setAccessible(true);
        require(!(Boolean)allows.invoke(policy,"https://example.com/") && !(Boolean)allows.invoke(policy,"file:///etc/hosts"),"Inline WebView policy rejects network and files");
        Class<?> policyType=policy.getClass();Constructor<?> constructor=policyType.getDeclaredConstructor(JSONObject.class);constructor.setAccessible(true);Object remote=constructor.newInstance(new JSONObject().put("url","https://example.com/page"));
        require((Boolean)allows.invoke(remote,"https://example.com:443/next") && !(Boolean)allows.invoke(remote,"https://example.com.evil.test/") && !(Boolean)allows.invoke(remote,"http://example.com/") && !(Boolean)allows.invoke(remote,"https://example.com:444/") && !(Boolean)allows.invoke(remote,"javascript:alert(1)"),"WebView main-navigation policy enforces HTTPS and exact initial origin");
        int rejected=0;for(String url:new String[]{"file:///secret","https://user:pass@example.com/","https:///missing","intent://example.com/"})try{constructor.newInstance(new JSONObject().put("url",url));}catch(InvocationTargetException expected){if(expected.getCause() instanceof IllegalArgumentException)rejected++;}
        require(rejected==4,"Invalid and credential-bearing WebView source URIs rejected");
        invoke(manager,"publish",String.class,new JSONObject().put("views",new JSONArray().put(descriptor)).toString());require(field(entries.get(888888L),"instance")==instance && field(instance,"web")==view,"Idempotent WebView update retains native instance");
        properties.put("javascript",true);descriptor.put("properties",properties);invoke(manager,"publish",String.class,new JSONObject().put("views",new JSONArray().put(descriptor)).toString());waitNode("Script executed");require(true,"Explicit JavaScript opt-in executes inline document without native bridge");
        invoke(manager,"publish",String.class,new JSONObject().put("views",new JSONArray()).toString());require((Boolean)field(instance,"closed") && field(instance,"web")==null,"Unmount destroys WebView and drops its retained reference");
    }
    @Override public void onStart(){Bundle result=new Bundle();int code=Activity.RESULT_CANCELED;String message;
        try{
            android.accessibilityservice.AccessibilityServiceInfo info=getUiAutomation().getServiceInfo();info.flags &= ~android.accessibilityservice.AccessibilityServiceInfo.FLAG_INCLUDE_NOT_IMPORTANT_VIEWS;getUiAutomation().setServiceInfo(info);
            activity=startActivitySync(new Intent(Intent.ACTION_MAIN).setComponent(new ComponentName(pkg,"com.theaimartco.taim.TaimNativeProbeActivity")).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            waitNode("Native button");waitNode("Native texture");require(count("created")==1,"Native factory not created exactly once");
            click("Native button");require(count("clicks")==1,"Native accessibility activation failed");waitNode("Native events: 1");
            Rect bounds=new Rect();waitNode("Native button").getBoundsInScreen(bounds);shell("input tap "+bounds.centerX()+" "+bounds.centerY());Thread.sleep(250);require(count("clicks")==2,"Native touch not routed");waitNode("Native events: 2");
            AccessibilityNodeInfo editor=waitNode("Native editor");Bundle text=new Bundle();text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,"नमस्ते 日本 🌍");require(editor.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,text),"Native Unicode text rejected");waitNode("नमस्ते 日本 🌍");
            Object manager=field(activity,"nativeViews");View tree=(View)manager;
            final View[] focused=new View[1];runOnMainSync(()->{focused[0]=nativeChild(android.widget.EditText.class,tree);focused[0].requestFocus();});Thread.sleep(300);
            require(focused[0].hasFocus(),"Native editor lost Android focus");
            final boolean[] committed=new boolean[1];runOnMainSync(()->{android.view.inputmethod.InputConnection connection=((android.widget.EditText)focused[0]).onCreateInputConnection(new android.view.inputmethod.EditorInfo());committed[0]=connection!=null && connection.commitText("!",1);});require(committed[0],"Native input connection rejected commit");waitNode("नमस्ते 日本 🌍!");
            shell("input keyevent KEYCODE_A");waitNode("नमस्ते 日本 🌍!a");require(true,"Hardware typing");
            shell("input keyevent KEYCODE_DEL");waitNode("नमस्ते 日本 🌍!");require(true,"Hardware backspace");
            shell("input keyevent KEYCODE_DPAD_LEFT KEYCODE_FORWARD_DEL");waitNode("नमस्ते 日本 🌍");require(true,"Arrow and forward delete");
            shell("input keycombination KEYCODE_CTRL_LEFT KEYCODE_A");shell("input text xyz");waitNode("xyz");require(true,"Ctrl+A replacement");
            shell("input keycombination KEYCODE_SHIFT_LEFT KEYCODE_B");waitNode("xyzB");require(true,"Shift modifier");
            runOnMainSync(()->{android.view.inputmethod.InputConnection connection=((android.widget.EditText)focused[0]).onCreateInputConnection(new android.view.inputmethod.EditorInfo());long time=SystemClock.uptimeMillis();connection.sendKeyEvent(new KeyEvent(time,time,KeyEvent.ACTION_DOWN,KeyEvent.KEYCODE_C,0));connection.sendKeyEvent(new KeyEvent(time,time+1,KeyEvent.ACTION_DOWN,KeyEvent.KEYCODE_C,1));connection.sendKeyEvent(new KeyEvent(time,time+2,KeyEvent.ACTION_UP,KeyEvent.KEYCODE_C,0));});waitNode("xyzBcc");require(true,"IME KeyEvent repeat");
            runOnMainSync(()->{android.view.inputmethod.InputConnection connection=((android.widget.EditText)focused[0]).onCreateInputConnection(new android.view.inputmethod.EditorInfo());connection.sendKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN,KeyEvent.KEYCODE_DEL));connection.sendKeyEvent(new KeyEvent(KeyEvent.ACTION_UP,KeyEvent.KEYCODE_DEL));});waitNode("xyzBc");require(true,"IME KeyEvent delete");
            final View button=nativeChild(android.widget.Button.class,tree);runOnMainSync(()->{button.setFocusableInTouchMode(true);button.requestFocus();});Thread.sleep(200);shell("input keyevent KEYCODE_ENTER");Thread.sleep(200);require(count("clicks")==3,"Native button Enter did not activate exactly once");waitNode("Native events: 3");
            runOnMainSync(()->focused[0].requestFocus());Thread.sleep(200);
            shell("input keyevent KEYCODE_SHIFT_LEFT");
            runOnMainSync(()->{try{Method key=manager.getClass().getDeclaredMethod("key",long.class,long.class,int.class,boolean.class,boolean.class,String.class);key.setAccessible(true);Object entry=field(manager,"focused");key.invoke(manager,((Number)field(entry,"id")).longValue(),((Number)field(entry,"generation")).longValue(),KeyEvent.KEYCODE_SHIFT_LEFT,true,false,"");}catch(Exception error){throw new RuntimeException(error);}});
            invoke(manager,"clearFocus",boolean.class,false);require(((Map<?,?>)field(manager,"keys")).isEmpty(),"Focus loss retained held modifiers");
            runOnMainSync(()->focused[0].requestFocus());Thread.sleep(200);Bundle reset=new Bundle();reset.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,"नमस्ते 日本 🌍!");waitNode("xyzBc").performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,reset);waitNode("नमस्ते 日本 🌍!");
            runOnMainSync(()->((android.view.inputmethod.InputMethodManager)activity.getSystemService(Context.INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(focused[0].getWindowToken(),0));Thread.sleep(250);
            int blue=pixel(waitNode("Native texture"));require(Color.blue(blue)>200 && Color.red(blue)<40,"Texture producer did not render blue pixels");
            click("Change texture color");Thread.sleep(400);int red=pixel(waitNode("Native texture"));require(Color.red(red)>200 && Color.blue(red)<40,"Texture update did not render red pixels");
            require(count("created")==1,"Property update recreated native controls");
            click("Toggle native input");Thread.sleep(200);require(find("Native button")==null,"Disabled native subtree remains accessible");
            int before=count("clicks");shell("input tap "+bounds.centerX()+" "+bounds.centerY());Thread.sleep(200);require(count("clicks")==before,"Disabled native control accepted touch");
            click("Toggle native input");waitNode("Native button");waitNode("नमस्ते 日本 🌍!");require(count("created")==1,"Re-enable lost native state");
            click("Show modal");waitNode("Close modal");Thread.sleep(200);require(find("Native button")==null && find("Native texture")==null,"Modal exposes native subtree");
            click("Close modal");waitNode("Native button");waitNode("नमस्ते 日本 🌍!");require(count("created")==1,"Modal recreated native instance");
            int resumes=count("resumes");shell("input keyevent KEYCODE_HOME");Thread.sleep(400);require(count("pauses")>0,"Background did not pause native factory");
            shell("am start -W -n "+pkg+"/com.theaimartco.taim.TaimNativeProbeActivity");waitNode("Native button");Thread.sleep(250);require(count("resumes")>resumes && count("created")==1,"Resume recreated native control or missed lifecycle");waitNode("नमस्ते 日本 🌍!");
            @SuppressWarnings("unchecked") Map<Long,Object> entries=(Map<Long,Object>)field(manager,"entries");
            JSONArray original=new JSONArray();Object old=null;JSONObject controls=null;for(Object entry:entries.values()){JSONObject descriptor=new JSONObject(field(entry,"descriptor").toString());original.put(descriptor);if("fixture.controls".equals(descriptor.getString("factory"))){old=entry;controls=descriptor;}}
            if(old==null)throw new AssertionError("Missing controls entry");long generation=((Number)field(old,"generation")).longValue();
            JSONArray clipped=new JSONArray(original.toString());for(int i=0;i<clipped.length();i++){JSONObject value=clipped.getJSONObject(i);if("fixture.controls".equals(value.getString("factory"))){JSONArray frame=value.getJSONArray("bounds");value.put("clip",new JSONArray(new double[]{frame.getDouble(0),frame.getDouble(1),frame.getDouble(2),30}));}}
            invoke(manager,"publish",String.class,new JSONObject().put("views",clipped).toString());Thread.sleep(200);int clipClicks=count("clicks");shell("input tap "+bounds.centerX()+" "+bounds.centerY());Thread.sleep(200);require(count("clicks")==clipClicks,"Touch outside native clip activated control");
            invoke(manager,"publish",String.class,new JSONObject().put("views",new JSONArray()).toString());require(count("disposed")==1,"Unmount did not dispose native controls");
            invoke(manager,"publish",String.class,new JSONObject().put("views",original).toString());waitNode("Native button");Object replacementEntry=entries.get(controls.getLong("id"));require(count("created")==2 && ((Number)field(replacementEntry,"generation")).longValue()>generation,"Remount reused native generation");
            final android.widget.EditText replacementEditor=(android.widget.EditText)nativeChild(android.widget.EditText.class,tree);runOnMainSync(()->replacementEditor.requestFocus());Thread.sleep(200);
            long slotId=controls.getLong("id"),newGeneration=((Number)field(replacementEntry,"generation")).longValue();
            nativeKey(manager,slotId,generation,KeyEvent.KEYCODE_A,true,false,"a");nativeKey(manager,slotId,generation,KeyEvent.KEYCODE_A,false,false,"");require(replacementEditor.getText().length()==0,"Old generation key reached replacement editor");
            nativeKey(manager,slotId,newGeneration,KeyEvent.KEYCODE_UNKNOWN,true,false,"日本🌍");nativeKey(manager,slotId,newGeneration,KeyEvent.KEYCODE_UNKNOWN,false,false,"");require("日本🌍".contentEquals(replacementEditor.getText()),"Translated Unicode key text was lost");
            nativeKey(manager,slotId,newGeneration,KeyEvent.KEYCODE_A,true,true,"a");require("日本🌍".contentEquals(replacementEditor.getText()) && ((Map<?,?>)field(manager,"keys")).isEmpty(),"Orphan repeat created a fresh key stream");
            invoke(old,"emit",JSONObject.class,new JSONObject().put("type","click"));Thread.sleep(300);require(find("Native events: 3")!=null && find("Native events: 4")==null,"Disposed host delivered stale callback");
            JSONObject rejected=new JSONObject(controls.toString()).put("id",999999).put("factory","fixture.surface");JSONArray withInvalid=new JSONArray(original.toString()).put(rejected);invoke(manager,"publish",String.class,new JSONObject().put("views",withInvalid).toString());require(count("badDisposed")==1 && field(entries.get(999999L),"instance")==null,"SurfaceView factory was not rejected and disposed");
            invoke(manager,"publish",String.class,new JSONObject().put("views",original).toString());
            if(webTests){web(manager,entries);invoke(manager,"publish",String.class,new JSONObject().put("views",original).toString());waitNode("Native button");}
            if(sensorTests)sensors();
            int expectedDisposed=count("disposed")+1;
            runOnMainSync(()->activity.finish());long end=SystemClock.uptimeMillis()+5000;while(count("disposed")<expectedDisposed && SystemClock.uptimeMillis()<end)Thread.sleep(100);require(count("disposed")==expectedDisposed,"Native factory was not disposed exactly once");
            code=Activity.RESULT_OK;message="Native hierarchy, Unicode input, TextureView pixels and lifecycle passed\n";
        }catch(Throwable error){message="Native embedding failed: "+error+"\n"+nodes(getUiAutomation().getRootInActiveWindow())+"\n";}
        result.putInt("checks",checks);result.putString("stream",message);finish(code,result);
    }
}
