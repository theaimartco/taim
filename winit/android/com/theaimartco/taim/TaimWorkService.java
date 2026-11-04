package com.theaimartco.taim;

import android.app.job.*;
import android.content.*;
import android.os.*;
import org.json.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;

/** System-bound headless service. Never starts a NativeActivity or holds a foreground-service lease. */
public final class TaimWorkService extends JobService {
    private static final Handler MAIN=new Handler(Looper.getMainLooper());
    private static final AtomicLong NEXT=new AtomicLong(1);
    private static final ConcurrentHashMap<Long,Run> RUNS=new ConcurrentHashMap<>();
    private static ThreadFactory threads(String label){AtomicInteger next=new AtomicInteger();return task->{Thread thread=new Thread(task,label+next.incrementAndGet());thread.setDaemon(true);return thread;};}
    private static final ThreadPoolExecutor MANAGER=new ThreadPoolExecutor(1,1,0,TimeUnit.SECONDS,new ArrayBlockingQueue<>(64),threads("taim-work-ledger-"),new ThreadPoolExecutor.AbortPolicy());
    private static final ThreadPoolExecutor WORKERS=new ThreadPoolExecutor(2,2,0,TimeUnit.SECONDS,new ArrayBlockingQueue<>(2),threads("taim-work-rust-"),new ThreadPoolExecutor.AbortPolicy());
    private static final class Run {
        final TaimWorkService service;final Context context;final JobParameters params;final int id;final String generation;final long token;
        final AtomicBoolean stopped=new AtomicBoolean();final AtomicLong progressAt=new AtomicLong();volatile Future<?> future;volatile boolean osStopped;Runnable deadline;
        Run(TaimWorkService service,JobParameters params){this.service=service;context=service.getApplicationContext();this.params=params;id=params.getExtras().getInt("id",0);generation=params.getExtras().getString("generation","");token=NEXT.getAndIncrement();}
    }
    static boolean post(Runnable task){try{MANAGER.execute(task);return true;}catch(RejectedExecutionException full){return false;}}
    static boolean running(int id,String generation){for(Run run:RUNS.values())if(run.id==id && run.generation.equals(generation) && !run.stopped.get())return true;return false;}
    static void cancel(Context context,int id,String generation){for(Run run:RUNS.values())if(run.id==id && run.generation.equals(generation)){detach(run);}}
    private static void detach(Run run){run.stopped.set(true);RUNS.remove(run.token,run);if(run.deadline!=null)MAIN.removeCallbacks(run.deadline);Future<?> future=run.future;if(future!=null)future.cancel(true);WORKERS.purge();}
    private static void finished(Run run,boolean retry){MAIN.post(()->{if(!run.osStopped)run.service.jobFinished(run.params,retry);});}
    private static void cancelTerminalSchedule(Context context,int id,String generation){
        synchronized(TaimWork.LOCK){try{JobInfo job=TaimWork.scheduler(context).getPendingJob(TaimWork.BASE|id);if(TaimWork.ours(context,job) && generation.equals(job.getExtras().getString("generation")))TaimWork.scheduler(context).cancel(TaimWork.BASE|id);}catch(RuntimeException ignored){}}
    }
    @Override public void onCreate(){super.onCreate();TaimWork.initialize(getApplicationContext());}
    @Override public boolean onStartJob(JobParameters params){
        Run run=new Run(this,params);if(run.id<1 || run.id>4095 || params.getJobId()!=(TaimWork.BASE|run.id) || run.generation.length()!=36 || run.token<=0)return false;
        RUNS.put(run.token,run);
        if(!post(()->start(run))){detach(run);finished(run,true);}return true;
    }
    private static void start(Run run){
        if(run.stopped.get())return;
        try {
            TaimWork.configuration(run.context);JSONObject row=TaimWork.claim(run.context,run.id,run.generation);
            if(row==null){detach(run);finished(run,false);cancelTerminalSchedule(run.context,run.id,run.generation);return;}
            if(run.stopped.get()){TaimWork.stopped(run.context,run.id,run.generation,"stopped");return;}
            JSONObject spec=row.getJSONObject("spec");
            JSONObject input=new JSONObject().put("id",run.id).put("generation",run.generation).put("worker",spec.getString("worker"))
                .put("attempt",row.getInt("attempt")).put("input",spec.getJSONObject("input"))
                .put("files_dir",run.context.getFilesDir().getAbsolutePath()).put("cache_dir",run.context.getCacheDir().getAbsolutePath());
            input.put("network_handle",Build.VERSION.SDK_INT>=28?Api28.network(run.params):JSONObject.NULL);
            run.deadline=()->{
                if(!RUNS.containsKey(run.token))return;detach(run);
                if(!post(()->{try{boolean retry=TaimWork.finish(run.context,run.id,run.generation,"retry",new JSONObject(),"timeout");finished(run,retry);if(!retry)cancelTerminalSchedule(run.context,run.id,run.generation);}catch(Exception failed){finished(run,true);}}))finished(run,true);
            };
            MAIN.postDelayed(run.deadline,spec.getLong("timeout"));
            try{run.future=WORKERS.submit(()->execute(run,input));if(run.stopped.get()){run.future.cancel(true);WORKERS.purge();}}
            catch(RejectedExecutionException full){detach(run);boolean retry=TaimWork.finish(run.context,run.id,run.generation,"retry",new JSONObject(),"workerBusy");finished(run,retry);if(!retry)cancelTerminalSchedule(run.context,run.id,run.generation);}
        }catch(Exception failure){detach(run);try{TaimWork.finish(run.context,run.id,run.generation,"failure",new JSONObject().put("code","workerSetupFailed"),"workerSetupFailed");}catch(Exception ignored){}finished(run,false);cancelTerminalSchedule(run.context,run.id,run.generation);}
    }
    private static void execute(Run run,JSONObject input){
        if(run.stopped.get())return;String response;
        try {
            String library=TaimWork.configuration(run.context).metaData.getString(TaimWork.LIBRARY);System.loadLibrary(library);
            response=nativeRunWork(input.toString(),run.token);
            if(response==null || response.getBytes(StandardCharsets.UTF_8).length>20000)throw new IllegalArgumentException();
        }catch(Throwable failure){response="{\"outcome\":\"failure\",\"output\":{\"code\":\"nativeWorkerFailed\"}}";}
        final String result=response;
        if(!post(()->{
            if(run.stopped.get() || !RUNS.remove(run.token,run))return;
            if(run.deadline!=null)MAIN.removeCallbacks(run.deadline);
            try {
                JSONObject value=new JSONObject(result),output=value.getJSONObject("output");String outcome=value.getString("outcome");
                if(!Arrays.asList("success","failure","retry").contains(outcome) || output.toString().getBytes(StandardCharsets.UTF_8).length>16384)throw new IllegalArgumentException();
                boolean retry=TaimWork.finish(run.context,run.id,run.generation,outcome,output,outcome.equals("failure")?"workerFailure":null);
                run.stopped.set(true);finished(run,retry);
                if(!retry && !outcome.equals("success"))cancelTerminalSchedule(run.context,run.id,run.generation);
            }catch(Exception failure){run.stopped.set(true);try{TaimWork.finish(run.context,run.id,run.generation,"failure",new JSONObject().put("code","invalidOutput"),"invalidOutput");}catch(Exception ignored){}finished(run,false);cancelTerminalSchedule(run.context,run.id,run.generation);}
        })){detach(run);finished(run,true);}
    }
    @Override public boolean onStopJob(JobParameters params){
        for(Run run:RUNS.values())if(run.params==params || (run.params.getJobId()==params.getJobId() && run.generation.equals(params.getExtras().getString("generation")))) {
            run.osStopped=true;detach(run);post(()->{try{boolean retry=TaimWork.stopped(run.context,run.id,run.generation,"osStopped");if(!retry)cancelTerminalSchedule(run.context,run.id,run.generation);}catch(Exception ignored){}});return true;
        }
        return false;
    }
    @Override public void onDestroy(){for(Run run:RUNS.values())if(run.service==this){run.osStopped=true;detach(run);post(()->{try{boolean retry=TaimWork.stopped(run.context,run.id,run.generation,"serviceDestroyed");if(!retry)cancelTerminalSchedule(run.context,run.id,run.generation);}catch(Exception ignored){}});}super.onDestroy();}
    /** JNI host hooks run on worker threads, with immutable execution tokens. */
    public static boolean isCancelled(long token){Run run=RUNS.get(token);return run==null || run.stopped.get();}
    public static boolean setProgress(long token,String payload){
        Run run=RUNS.get(token);if(run==null || run.stopped.get() || payload==null || payload.getBytes(StandardCharsets.UTF_8).length>4096)return false;
        long now=SystemClock.elapsedRealtime(),previous=run.progressAt.get();if(previous!=0 && now-previous<250)return false;
        if(!run.progressAt.compareAndSet(previous,now))return false;
        try{JSONObject progress=new JSONObject(payload);return post(()->{if(!run.stopped.get())try{TaimWork.progress(run.context,run.id,run.generation,progress);}catch(Exception ignored){}});}catch(JSONException invalid){return false;}
    }
    private static native String nativeRunWork(String input,long token);
    @SuppressWarnings("UseRequiresApi") // framework-only host: no AndroidX @RequiresApi
    @android.annotation.TargetApi(28)
    private static final class Api28 {static Object network(JobParameters params){android.net.Network network=params.getNetwork();return network==null?JSONObject.NULL:network.getNetworkHandle();}}
}
