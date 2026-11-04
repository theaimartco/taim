package com.theaimartco.taim;
import android.graphics.*;
import android.view.*;
import org.json.JSONObject;
/** Built-in static SurfaceTexture producer. No idle animation or capture permission is involved. */
final class TaimTextureView implements TaimNativeViewFactory.Instance,TextureView.SurfaceTextureListener {
    private final TextureView view;private Surface surface;private boolean active,disposed;private int color=Color.BLUE;private final Paint paint=new Paint(Paint.ANTI_ALIAS_FLAG);
    TaimTextureView(TaimActivity activity,JSONObject props,TaimNativeViewFactory.Host host){view=new TextureView(activity);view.setSurfaceTextureListener(this);view.setContentDescription("Native texture");update(props);}
    public View view(){return view;}
    public void update(JSONObject props){color=props.optInt("color",Color.BLUE);draw();}
    public void resumed(){active=true;if(view.isAvailable() && surface==null)surface=new Surface(view.getSurfaceTexture());draw();}
    public void paused(){active=false;if(surface!=null){surface.release();surface=null;}}
    public void dispose(){if(disposed)return;disposed=true;paused();view.setSurfaceTextureListener(null);}
    private void draw(){if(!active || disposed || surface==null || !surface.isValid())return;Canvas canvas=null;try{canvas=surface.lockCanvas(null);canvas.drawColor(color);paint.setColor(Color.WHITE);paint.setTextSize(32);canvas.drawText("Native texture",16,48,paint);}catch(RuntimeException ignored){}finally{if(canvas!=null)try{surface.unlockCanvasAndPost(canvas);}catch(RuntimeException ignored){}}}
    public void onSurfaceTextureAvailable(SurfaceTexture texture,int width,int height){if(active){surface=new Surface(texture);draw();}}
    public void onSurfaceTextureSizeChanged(SurfaceTexture texture,int width,int height){draw();}
    public boolean onSurfaceTextureDestroyed(SurfaceTexture texture){if(surface!=null){surface.release();surface=null;}return true;}
    public void onSurfaceTextureUpdated(SurfaceTexture texture){}
}
