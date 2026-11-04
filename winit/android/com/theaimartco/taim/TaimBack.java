package com.theaimartco.taim;

import android.os.Build;
import org.json.JSONObject;

/** New predictive-back API types are isolated from the min-26 host. */
abstract class TaimBack {
    final TaimActivity host;
    TaimBack(TaimActivity host){this.host=host;}
    abstract void enabled(boolean enabled);
    abstract void close();
    static TaimBack create(TaimActivity host){if(Build.VERSION.SDK_INT>=34)return new Animated(host);if(Build.VERSION.SDK_INT>=33)return new Invoked(host);return new Legacy(host);}
    final void event(String phase,float progress,int edge){try{JSONObject value=new JSONObject();value.put("type","back");value.put("phase",phase);value.put("progress",Float.isFinite(progress)?Math.max(0,Math.min(1,progress)):0);value.put("edge",edge);host.platformEvent(value);}catch(org.json.JSONException ignored){}}
    private static final class Legacy extends TaimBack {Legacy(TaimActivity host){super(host);}void enabled(boolean value){}void close(){}}
    @SuppressWarnings("UseRequiresApi") // framework-only host: no AndroidX @RequiresApi
    @android.annotation.TargetApi(33)
    private static class Invoked extends TaimBack implements android.window.OnBackInvokedCallback {
        boolean registered;
        Invoked(TaimActivity host){super(host);}
        @Override public void onBackInvoked(){event("invoked",1,0);}
        void enabled(boolean value){if(value==registered)return;registered=value;if(value)host.getOnBackInvokedDispatcher().registerOnBackInvokedCallback(android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT,this);else host.getOnBackInvokedDispatcher().unregisterOnBackInvokedCallback(this);}
        void close(){enabled(false);}
    }
    @SuppressWarnings("UseRequiresApi") // framework-only host: no AndroidX @RequiresApi
    @android.annotation.TargetApi(34)
    private static final class Animated extends Invoked implements android.window.OnBackAnimationCallback {
        Animated(TaimActivity host){super(host);}
        @Override public void onBackStarted(android.window.BackEvent event){event("started",event.getProgress(),event.getSwipeEdge());}
        @Override public void onBackProgressed(android.window.BackEvent event){event("progressed",event.getProgress(),event.getSwipeEdge());}
        @Override public void onBackCancelled(){event("cancelled",0,0);}
    }
}
