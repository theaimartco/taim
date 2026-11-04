package com.theaimartco.taim;

import android.content.res.AssetFileDescriptor;
import android.graphics.Matrix;
import android.graphics.SurfaceTexture;
import android.media.AudioAttributes;
import android.media.MediaPlayer;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.view.*;
import org.json.*;
import java.util.Iterator;

/**
 * Inline video playback (factory {@code taim.video}): framework MediaPlayer into a TextureView.
 * Properties: {@code uri} (https://, content:// or asset:///path), {@code playing}, {@code looping},
 * {@code muted}, {@code fit} ("contain"|"cover"), and {@code seekMs}+{@code seekToken} (a seek runs
 * when the token changes). Events: prepared, position (every 500 ms while playing), buffering,
 * completed, error. https playback needs the INTERNET permission. Pausing the activity pauses
 * playback; resuming continues only if {@code playing} is still true.
 */
final class TaimVideoView implements TaimNativeViewFactory.Instance, TextureView.SurfaceTextureListener {
    private final TaimActivity activity;
    private final TaimNativeViewFactory.Host host;
    private final TextureView view;
    private final Handler main=new Handler(Looper.getMainLooper());
    private MediaPlayer player;
    private Surface surface;
    private String uri;
    private boolean playing,looping,muted,cover,prepared,active=true,disposed;
    private long seekToken=Long.MIN_VALUE;
    private final Runnable ticker=new Runnable(){public void run(){
        if(player!=null && prepared && player.isPlaying()){emit("position",new JSONObject());main.postDelayed(this,500);}
    }};

    TaimVideoView(TaimActivity activity,JSONObject properties,TaimNativeViewFactory.Host host)throws Exception {
        this.activity=activity;this.host=host;
        view=new TextureView(activity);view.setSurfaceTextureListener(this);
        view.setContentDescription("Video");
        update(properties);
    }
    @Override public View view(){return view;}

    private static boolean allowed(String value){
        if(value.length()>4096)return false;
        Uri u=Uri.parse(value);String scheme=u.getScheme();
        if("https".equalsIgnoreCase(scheme))return u.getHost()!=null && !u.getHost().isEmpty();
        if("content".equals(scheme))return true;
        if("asset".equals(scheme)){String path=u.getPath();return path!=null && path.length()>1 && !path.contains("..");}
        return false;
    }

    @Override public void update(JSONObject p)throws Exception {
        for(Iterator<String> keys=p.keys();keys.hasNext();){
            String k=keys.next();
            if(!k.matches("uri|playing|looping|muted|fit|seekMs|seekToken"))throw new IllegalArgumentException();
        }
        String next=p.getString("uri");if(!allowed(next))throw new IllegalArgumentException();
        String fit=p.optString("fit","contain");if(!fit.equals("contain") && !fit.equals("cover"))throw new IllegalArgumentException();
        playing=p.optBoolean("playing",false);looping=p.optBoolean("looping",false);muted=p.optBoolean("muted",false);
        boolean fitChanged=cover!=fit.equals("cover");cover=fit.equals("cover");
        if(!next.equals(uri)){uri=next;open();}
        else if(player!=null && prepared){
            player.setLooping(looping);player.setVolume(muted?0f:1f,muted?0f:1f);
            if(p.has("seekToken")){long token=p.getLong("seekToken");if(token!=seekToken){seekToken=token;long ms=Math.max(0,p.optLong("seekMs",0));player.seekTo((int)Math.min(ms,Integer.MAX_VALUE));}}
            apply();
            if(fitChanged)transform();
        }
        if(p.has("seekToken") && player==null)seekToken=p.getLong("seekToken");
    }

    private void open(){
        release();
        if(disposed || surface==null)return; // reopened when the surface becomes available
        prepared=false;
        MediaPlayer mp=new MediaPlayer();player=mp;
        try{
            mp.setAudioAttributes(new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_MOVIE).build());
            Uri u=Uri.parse(uri);
            if("asset".equals(u.getScheme())){
                try(AssetFileDescriptor fd=activity.getAssets().openFd(u.getPath().substring(1))){mp.setDataSource(fd.getFileDescriptor(),fd.getStartOffset(),fd.getLength());}
            }else mp.setDataSource(activity,u);
            mp.setSurface(surface);
            mp.setOnPreparedListener(ready->{
                if(player!=ready)return;prepared=true;
                ready.setLooping(looping);ready.setVolume(muted?0f:1f,muted?0f:1f);transform();
                try{emit("prepared",new JSONObject().put("durationMs",ready.getDuration()).put("width",ready.getVideoWidth()).put("height",ready.getVideoHeight()));}catch(JSONException ignored){}
                apply();
            });
            mp.setOnVideoSizeChangedListener((m,w,h)->transform());
            mp.setOnCompletionListener(m->{if(player==m && !looping){main.removeCallbacks(ticker);emit("completed",new JSONObject());}});
            mp.setOnInfoListener((m,what,extra)->{
                if(what==MediaPlayer.MEDIA_INFO_BUFFERING_START || what==MediaPlayer.MEDIA_INFO_BUFFERING_END){
                    try{emit("buffering",new JSONObject().put("active",what==MediaPlayer.MEDIA_INFO_BUFFERING_START));}catch(JSONException ignored){}
                }
                return false;
            });
            mp.setOnErrorListener((m,what,extra)->{
                if(player==m){try{emit("error",new JSONObject().put("what",what).put("extra",extra));}catch(JSONException ignored){}release();}
                return true;
            });
            mp.prepareAsync();
        }catch(Exception failed){
            try{emit("error",new JSONObject().put("what","open"));}catch(JSONException ignored){}
            release();
        }
    }

    private void apply(){
        if(player==null || !prepared)return;
        if(playing && active){if(!player.isPlaying()){player.start();main.removeCallbacks(ticker);main.post(ticker);}}
        else if(player.isPlaying()){player.pause();main.removeCallbacks(ticker);emit("position",new JSONObject());}
    }

    /** Letterbox (contain) or crop (cover) the decoded frame inside the slot. */
    private void transform(){
        if(player==null || !prepared)return;
        float vw=player.getVideoWidth(),vh=player.getVideoHeight(),w=view.getWidth(),h=view.getHeight();
        if(vw<=0 || vh<=0 || w<=0 || h<=0)return;
        float scale=cover?Math.max(w/vw,h/vh):Math.min(w/vw,h/vh);
        Matrix m=new Matrix();
        // TextureView stretches the frame to the view; undo that, then apply the uniform scale.
        m.setScale(vw*scale/w,vh*scale/h,w/2f,h/2f);
        view.setTransform(m);
    }

    private void emit(String type,JSONObject event){
        try{
            event.put("type",type);
            if(player!=null && prepared){event.put("positionMs",player.getCurrentPosition());event.put("playing",player.isPlaying());}
            host.emit(event);
        }catch(JSONException | IllegalStateException ignored){}
    }

    private void release(){
        main.removeCallbacks(ticker);
        if(player!=null){MediaPlayer old=player;player=null;prepared=false;try{old.reset();}catch(RuntimeException ignored){}old.release();}
    }

    @Override public void resumed(){active=true;apply();}
    @Override public void paused(){active=false;if(player!=null && prepared && player.isPlaying()){player.pause();main.removeCallbacks(ticker);}}
    @Override public void dispose(){if(disposed)return;disposed=true;release();view.setSurfaceTextureListener(null);if(surface!=null){surface.release();surface=null;}}

    @Override public void onSurfaceTextureAvailable(SurfaceTexture texture,int width,int height){
        surface=new Surface(texture);
        if(player==null)open();else player.setSurface(surface);
    }
    @Override public void onSurfaceTextureSizeChanged(SurfaceTexture texture,int width,int height){transform();}
    @Override public boolean onSurfaceTextureDestroyed(SurfaceTexture texture){
        if(player!=null){try{player.setSurface(null);}catch(RuntimeException ignored){}}
        if(surface!=null){surface.release();surface=null;}
        return true;
    }
    @Override public void onSurfaceTextureUpdated(SurfaceTexture texture){}
}
