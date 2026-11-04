package com.theaimartco.taim;

import android.app.job.*;
import android.content.*;
import android.content.pm.*;
import android.database.Cursor;
import android.database.sqlite.*;
import android.os.*;
import org.json.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.atomic.AtomicBoolean;

/** App-private durable ledger plus OS scheduler. Every method doing disk I/O runs off the main thread. */
final class TaimWork {
    static final Object LOCK=new Object();static final int BASE=0x54410000;
    static final String LIBRARY="com.theaimartco.taim.work.lib_name";
    private static Store store;private static final AtomicBoolean initialized=new AtomicBoolean();
    private static final class Store extends SQLiteOpenHelper {
        Store(Context context){super(context,"taim-work-v1.db",null,1);setWriteAheadLoggingEnabled(true);}
        @Override public void onCreate(SQLiteDatabase db){db.execSQL("CREATE TABLE work(id INTEGER PRIMARY KEY,generation TEXT NOT NULL,spec TEXT NOT NULL,state TEXT NOT NULL,attempt INTEGER NOT NULL,runs INTEGER NOT NULL,output TEXT,progress TEXT,error TEXT,updated INTEGER NOT NULL,earliest INTEGER NOT NULL)");}
        @Override public void onUpgrade(SQLiteDatabase db,int from,int to){throw new IllegalStateException("Unsupported work schema");}
    }
    static SQLiteDatabase db(Context context){if(store==null)store=new Store(context.getApplicationContext());return store.getWritableDatabase();}
    static ComponentName component(Context context){return new ComponentName(context,TaimWorkService.class);}
    static ServiceInfo configuration(Context context)throws Exception {
        ServiceInfo info;
        try{info=context.getPackageManager().getServiceInfo(component(context),PackageManager.GET_META_DATA);}catch(PackageManager.NameNotFoundException missing){throw new UnsupportedOperationException();}
        if(!info.exported || !JobService.PERMISSION_BIND.equals(info.permission) || !info.applicationInfo.processName.equals(info.processName)
            || info.metaData==null || !info.metaData.getString(LIBRARY,"").matches("[A-Za-z0-9_]{1,128}"))throw new UnsupportedOperationException();
        return info;
    }
    static void initialize(Context context){
        if(initialized.get())return;
        try{configuration(context);}catch(Exception absent){return;}
        if(initialized.compareAndSet(false,true) && !TaimWorkService.post(()->{try{reconcile(context);}catch(Exception ignored){}}))initialized.set(false);
    }
    static int id(JSONObject request)throws JSONException {int id=request.getInt("id");if(id<1 || id>4095)throw new IllegalArgumentException();return id;}
    static boolean active(String state){return state.equals("queued") || state.equals("retry") || state.equals("running");}
    static JobScheduler scheduler(Context context){return context.getSystemService(JobScheduler.class);}
    static boolean ours(Context context,JobInfo info){return info!=null && component(context).equals(info.getService());}
    static JSONObject row(Context context,int id)throws Exception {
        try(Cursor cursor=db(context).rawQuery("SELECT generation,spec,state,attempt,runs,output,progress,error,updated,earliest FROM work WHERE id=?",new String[]{Integer.toString(id)})){
            if(!cursor.moveToFirst())return null;
            JSONObject value=new JSONObject().put("id",id).put("generation",cursor.getString(0)).put("spec",new JSONObject(cursor.getString(1)))
                .put("state",cursor.getString(2)).put("attempt",cursor.getInt(3)).put("runs",cursor.getInt(4)).put("updated_ms",cursor.getLong(8)).put("earliest",cursor.getLong(9));
            value.put("output",cursor.isNull(5)?JSONObject.NULL:new JSONObject(cursor.getString(5)));
            value.put("progress",cursor.isNull(6)?new JSONObject():new JSONObject(cursor.getString(6)));
            value.put("last_error",cursor.isNull(7)?JSONObject.NULL:cursor.getString(7));return value;
        }
    }
    static JSONObject describe(Context context,JSONObject row,boolean full)throws Exception {
        JSONObject spec=row.getJSONObject("spec");JobInfo pending=scheduler(context).getPendingJob(BASE|row.getInt("id"));
        JSONObject value=new JSONObject().put("id",row.getInt("id")).put("generation",row.getString("generation"))
            .put("worker",spec.getString("worker")).put("state",row.getString("state")).put("attempt",row.getInt("attempt"))
            .put("runs",row.getInt("runs")).put("maximum_attempts",spec.getInt("maxAttempts")).put("updated_ms",row.getLong("updated_ms"))
            .put("last_error",row.get("last_error")).put("scheduled",ours(context,pending) && row.getString("generation").equals(pending.getExtras().getString("generation")));
        if(full)value.put("input",spec.getJSONObject("input")).put("output",row.get("output")).put("progress",row.get("progress"));return value;
    }
    private static void validate(JSONObject spec)throws Exception {
        id(spec);if(!spec.getString("worker").matches("[A-Za-z0-9._-]{1,64}"))throw new IllegalArgumentException();
        if(spec.getJSONObject("input").toString().getBytes(StandardCharsets.UTF_8).length>16384)throw new IllegalArgumentException();
        long delay=spec.getLong("delay"),backoff=spec.getLong("backoff");int attempts=spec.getInt("maxAttempts"),timeout=spec.getInt("timeout");
        if(delay<0 || delay>604800000L || backoff<10000 || backoff>18000000 || attempts<1 || attempts>16 || timeout<1000 || timeout>120000)throw new IllegalArgumentException();
        if(!Arrays.asList("none","connected","unmetered","notRoaming","metered").contains(spec.getString("network")) || !Arrays.asList("keep","replace").contains(spec.getString("policy"))
            || !Arrays.asList("linear","exponential").contains(spec.getString("backoffPolicy")))throw new IllegalArgumentException();
        if(!spec.isNull("interval")){long interval=spec.getLong("interval"),flex=spec.getLong("flex");if(interval<900000 || interval>604800000L || flex<Math.max(300000,interval/20) || flex>interval)throw new IllegalArgumentException();}
    }
    // Optional library capability: default apps deliberately do not declare this normal permission.
    // Lint cannot model the app-selected declaration; the OS permission is checked at the call site.
    @android.annotation.SuppressLint("MissingPermission")
    private static void persist(Context context,JobInfo.Builder builder){
        if(context.checkSelfPermission(android.Manifest.permission.RECEIVE_BOOT_COMPLETED)!=PackageManager.PERMISSION_GRANTED)throw new SecurityException("Persisted work requires RECEIVE_BOOT_COMPLETED");
        builder.setPersisted(true);
    }
    static JobInfo build(Context context,JSONObject row)throws Exception {
        JSONObject spec=row.getJSONObject("spec");PersistableBundle extras=new PersistableBundle();extras.putInt("id",row.getInt("id"));extras.putString("generation",row.getString("generation"));
        JobInfo.Builder builder=new JobInfo.Builder(BASE|row.getInt("id"),component(context)).setExtras(extras)
            .setRequiresCharging(spec.getBoolean("charging")).setRequiresBatteryNotLow(spec.getBoolean("batteryNotLow")).setRequiresStorageNotLow(spec.getBoolean("storageNotLow"))
            .setBackoffCriteria(spec.getLong("backoff"),spec.getString("backoffPolicy").equals("linear")?JobInfo.BACKOFF_POLICY_LINEAR:JobInfo.BACKOFF_POLICY_EXPONENTIAL);
        if(spec.getBoolean("persisted"))persist(context,builder);
        int network;switch(spec.getString("network")){case "connected":network=JobInfo.NETWORK_TYPE_ANY;break;case "unmetered":network=JobInfo.NETWORK_TYPE_UNMETERED;break;case "notRoaming":network=JobInfo.NETWORK_TYPE_NOT_ROAMING;break;case "metered":network=JobInfo.NETWORK_TYPE_METERED;break;default:network=JobInfo.NETWORK_TYPE_NONE;}
        builder.setRequiredNetworkType(network);
        if(spec.isNull("interval"))builder.setMinimumLatency(Math.max(0,row.getLong("earliest")-System.currentTimeMillis()));else builder.setPeriodic(spec.getLong("interval"),spec.getLong("flex"));
        return builder.build();
    }
    static void prune(Context context){db(context).execSQL("DELETE FROM work WHERE id IN (SELECT id FROM work WHERE state IN ('succeeded','failed','cancelled') ORDER BY updated DESC LIMIT -1 OFFSET 128)");}
    static void update(Context context,JSONObject row,String state,String error,Object output,boolean resetAttempts)throws Exception {
        ContentValues values=new ContentValues();values.put("state",state);values.put("error",error);values.put("updated",System.currentTimeMillis());
        if(output instanceof JSONObject)values.put("output",output.toString());if(resetAttempts)values.put("attempt",0);
        db(context).update("work",values,"id=? AND generation=?",new String[]{row.getString("id"),row.getString("generation")});
    }
    static Object request(Context context,JSONObject request)throws Exception {
        configuration(context);synchronized(LOCK){String op=request.getString("op");
            if(op.equals("workEnqueue"))return enqueue(context,request);
            if(op.equals("workReconcile"))return new JSONObject().put("rescheduled",reconcile(context));
            if(op.equals("workPrune")){int before=(int)android.database.DatabaseUtils.longForQuery(db(context),"SELECT COUNT(*) FROM work WHERE state IN ('succeeded','failed','cancelled')",null);db(context).delete("work","state IN ('succeeded','failed','cancelled')",null);return new JSONObject().put("removed",before);}
            if(op.equals("workList")){JSONArray values=new JSONArray();try(Cursor cursor=db(context).rawQuery(request.getBoolean("finished")?"SELECT id FROM work ORDER BY updated DESC LIMIT 192":"SELECT id FROM work WHERE state IN ('queued','running','retry') ORDER BY updated DESC LIMIT 64",null)){while(cursor.moveToNext()){JSONObject row=row(context,cursor.getInt(0));if(row!=null)values.put(describe(context,row,false));}}return values;}
            int id=id(request);JSONObject row=row(context,id);
            if(op.equals("workCancel")){if(row!=null && active(row.getString("state"))){update(context,row,"cancelled","cancelled",null,false);TaimWorkService.cancel(context,id,row.getString("generation"));}
                JobInfo job=scheduler(context).getPendingJob(BASE|id);if(ours(context,job))scheduler(context).cancel(BASE|id);return JSONObject.NULL;}
            return row==null?JSONObject.NULL:describe(context,row,true);
        }
    }
    private static Object enqueue(Context context,JSONObject spec)throws Exception {
        validate(spec);int id=id(spec);JSONObject old=row(context,id);JobScheduler scheduler=scheduler(context);JobInfo pending=scheduler.getPendingJob(BASE|id);
        if(pending!=null && !ours(context,pending))throw new java.util.concurrent.RejectedExecutionException();
        if(old!=null && active(old.getString("state")) && spec.getString("policy").equals("keep"))return describe(context,old,true);
        long count=android.database.DatabaseUtils.longForQuery(db(context),"SELECT COUNT(*) FROM work WHERE state IN ('queued','running','retry')",null);
        if(count>=64 && (old==null || !active(old.getString("state"))))throw new java.util.concurrent.RejectedExecutionException();
        if(old!=null)TaimWorkService.cancel(context,id,old.getString("generation"));if(ours(context,pending))scheduler.cancel(BASE|id);
        ContentValues values=new ContentValues();values.put("id",id);values.put("generation",UUID.randomUUID().toString());values.put("spec",spec.toString());values.put("state","queued");values.put("attempt",0);values.put("runs",0);values.putNull("output");values.put("progress","{}");values.putNull("error");values.put("updated",System.currentTimeMillis());values.put("earliest",System.currentTimeMillis()+spec.getLong("delay"));
        db(context).insertWithOnConflict("work",null,values,SQLiteDatabase.CONFLICT_REPLACE);JSONObject row=row(context,id);
        if(scheduler.schedule(build(context,row))!=JobScheduler.RESULT_SUCCESS){update(context,row,"failed","scheduleRejected",null,false);throw new java.util.concurrent.RejectedExecutionException();}
        prune(context);return describe(context,row,true);
    }
    static int reconcile(Context context)throws Exception {
        configuration(context);synchronized(LOCK){int rescheduled=0;ArrayList<Integer> ids=new ArrayList<>();try(Cursor cursor=db(context).rawQuery("SELECT id FROM work WHERE state IN ('queued','running','retry')",null)){while(cursor.moveToNext())ids.add(cursor.getInt(0));}
            for(int id:ids){JSONObject row=row(context,id);String generation=row.getString("generation");JobInfo pending=scheduler(context).getPendingJob(BASE|id);
                if(TaimWorkService.running(id,generation) || (ours(context,pending) && generation.equals(pending.getExtras().getString("generation"))))continue;
                if(pending!=null && !ours(context,pending))continue;
                if(row.getInt("attempt")>=row.getJSONObject("spec").getInt("maxAttempts")){update(context,row,"failed","attemptsExhausted",null,false);continue;}
                if(scheduler(context).schedule(build(context,row))==JobScheduler.RESULT_SUCCESS){update(context,row,"queued","recovered",null,false);rescheduled++;}
            }
            prune(context);return rescheduled;
        }
    }
    static JSONObject claim(Context context,int id,String generation)throws Exception {synchronized(LOCK){JSONObject row=row(context,id);
        if(row==null || !generation.equals(row.getString("generation")) || !active(row.getString("state")))return null;
        JSONObject spec=row.getJSONObject("spec");if(row.getInt("attempt")>=spec.getInt("maxAttempts")){update(context,row,"failed","attemptsExhausted",null,false);return null;}
        ContentValues values=new ContentValues();values.put("state","running");values.put("attempt",row.getInt("attempt")+1);values.put("runs",row.getInt("runs")+1);values.put("progress","{}");values.putNull("error");values.put("updated",System.currentTimeMillis());db(context).update("work",values,"id=? AND generation=?",new String[]{Integer.toString(id),generation});return row(context,id);
    }}
    static void progress(Context context,int id,String generation,JSONObject progress)throws Exception {synchronized(LOCK){ContentValues values=new ContentValues();values.put("progress",progress.toString());values.put("updated",System.currentTimeMillis());db(context).update("work",values,"id=? AND generation=? AND state='running'",new String[]{Integer.toString(id),generation});}}
    static boolean stopped(Context context,int id,String generation,String reason)throws Exception {synchronized(LOCK){JSONObject row=row(context,id);
        if(row==null || !generation.equals(row.getString("generation")) || !active(row.getString("state")))return false;
        if(row.getString("state").equals("running"))return finish(context,id,generation,"retry",new JSONObject(),reason);
        update(context,row,"retry",reason,null,false);return true;
    }}
    static boolean finish(Context context,int id,String generation,String outcome,JSONObject output,String error)throws Exception {synchronized(LOCK){JSONObject row=row(context,id);
        if(row==null || !generation.equals(row.getString("generation")) || !row.getString("state").equals("running"))return false;
        JSONObject spec=row.getJSONObject("spec");boolean retry=outcome.equals("retry") && row.getInt("attempt")<spec.getInt("maxAttempts");
        boolean periodic=outcome.equals("success") && !spec.isNull("interval");String state=periodic?"queued":outcome.equals("success")?"succeeded":retry?"retry":"failed";
        update(context,row,state,retry?error:outcome.equals("retry")?"attemptsExhausted":error,output,periodic);
        if(retry || periodic){long delay=periodic?spec.getLong("interval"):spec.getLong("backoff");if(retry){int attempt=row.getInt("attempt");delay=Math.min(18000000L,delay*(spec.getString("backoffPolicy").equals("linear")?attempt:1L<<Math.min(15,attempt-1)));}
            ContentValues values=new ContentValues();values.put("earliest",System.currentTimeMillis()+delay);db(context).update("work",values,"id=? AND generation=?",new String[]{Integer.toString(id),generation});}
        prune(context);return retry;
    }}
}
