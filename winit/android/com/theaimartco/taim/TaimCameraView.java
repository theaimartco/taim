package com.theaimartco.taim;

import android.Manifest;
import android.content.pm.PackageManager;
import android.graphics.*;
import android.hardware.camera2.*;
import android.hardware.camera2.params.StreamConfigurationMap;
import android.os.*;
import android.util.Size;
import android.view.*;
import org.json.*;
import java.util.*;

/**
 * Inline camera preview (factory {@code taim.camera}) using Camera2 into a TextureView.
 * Properties: {@code lens} ("back"|"front"), {@code active} (default true).
 * The camera is opened only while the activity is resumed, the slot's surface exists and
 * CAMERA permission is granted, and is closed on pause so other apps can use it.
 * Events: opened {width,height,lens}, closed, error {reason: denied|unsupported|inUse|disabled|disconnected|failed}.
 * Preview only: still/video capture uses the delegated media capture service.
 */
final class TaimCameraView implements TaimNativeViewFactory.Instance, TextureView.SurfaceTextureListener {
    private final TaimActivity activity;
    private final TaimNativeViewFactory.Host host;
    private final TextureView view;
    private final CameraManager manager;
    private HandlerThread thread;
    private Handler background;
    private CameraDevice device;
    private CameraCaptureSession session;
    private Size preview;
    private int sensorOrientation;
    private String lens="back";
    private boolean wanted=true,resumed=true,disposed,opening;
    private int generation;

    TaimCameraView(TaimActivity activity,JSONObject properties,TaimNativeViewFactory.Host host)throws Exception {
        this.activity=activity;this.host=host;
        manager=activity.getSystemService(CameraManager.class);
        view=new TextureView(activity);view.setSurfaceTextureListener(this);view.setContentDescription("Camera preview");
        update(properties);
    }
    @Override public View view(){return view;}

    @Override public void update(JSONObject p)throws Exception {
        for(Iterator<String> keys=p.keys();keys.hasNext();){String k=keys.next();if(!k.equals("lens") && !k.equals("active"))throw new IllegalArgumentException();}
        String next=p.optString("lens","back");if(!next.equals("back") && !next.equals("front"))throw new IllegalArgumentException();
        boolean nextWanted=p.optBoolean("active",true);
        boolean changed=!next.equals(lens) || nextWanted!=wanted;
        lens=next;wanted=nextWanted;
        if(changed){close();maybeOpen();}
    }

    private void error(String reason){try{host.emit(new JSONObject().put("type","error").put("reason",reason));}catch(JSONException | IllegalStateException ignored){}}

    private String cameraId()throws CameraAccessException {
        int facing=lens.equals("front")?CameraCharacteristics.LENS_FACING_FRONT:CameraCharacteristics.LENS_FACING_BACK;
        for(String id:manager.getCameraIdList()){Integer f=manager.getCameraCharacteristics(id).get(CameraCharacteristics.LENS_FACING);if(f!=null && f==facing)return id;}
        return null;
    }

    private void maybeOpen(){
        if(disposed || !wanted || !resumed || opening || device!=null || !view.isAvailable())return;
        if(manager==null){error("unsupported");return;}
        if(activity.checkSelfPermission(Manifest.permission.CAMERA)!=PackageManager.PERMISSION_GRANTED){error("denied");return;}
        try{
            String id=cameraId();if(id==null){error("unsupported");return;}
            CameraCharacteristics c=manager.getCameraCharacteristics(id);
            StreamConfigurationMap map=c.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP);
            Integer orientation=c.get(CameraCharacteristics.SENSOR_ORIENTATION);sensorOrientation=orientation==null?90:orientation;
            if(map==null){error("unsupported");return;}
            preview=choose(map.getOutputSizes(SurfaceTexture.class));
            if(thread==null){thread=new HandlerThread("taim-camera");thread.start();background=new Handler(thread.getLooper());}
            opening=true;int token=++generation;
            manager.openCamera(id,new CameraDevice.StateCallback(){
                @Override public void onOpened(CameraDevice camera){activity.runOnUiThread(()->{
                    opening=false;
                    if(token!=generation || disposed || !resumed || !wanted){camera.close();return;}
                    device=camera;start(token);
                });}
                @Override public void onDisconnected(CameraDevice camera){activity.runOnUiThread(()->{opening=false;camera.close();if(device==camera){device=null;session=null;error("disconnected");}});}
                @Override public void onError(CameraDevice camera,int code){activity.runOnUiThread(()->{
                    opening=false;camera.close();if(device==camera){device=null;session=null;}
                    error(code==ERROR_CAMERA_IN_USE || code==ERROR_MAX_CAMERAS_IN_USE?"inUse":code==ERROR_CAMERA_DISABLED?"disabled":"failed");
                });}
            },background);
        }catch(CameraAccessException e){opening=false;error(e.getReason()==CameraAccessException.CAMERA_DISABLED?"disabled":e.getReason()==CameraAccessException.CAMERA_IN_USE?"inUse":"failed");}
        catch(SecurityException denied){opening=false;error("denied");}
        catch(RuntimeException failed){opening=false;error("failed");}
    }

    /** Largest 16:9/4:3 output at most 1920x1080, which every device must support for preview. */
    private static Size choose(Size[] sizes){
        Size best=null;
        for(Size s:sizes){
            long area=(long)s.getWidth()*s.getHeight();
            if(s.getWidth()>1920 || s.getHeight()>1080)continue;
            if(best==null || area>(long)best.getWidth()*best.getHeight())best=s;
        }
        return best!=null?best:sizes[0];
    }

    private void start(int token){
        SurfaceTexture texture=view.getSurfaceTexture();if(texture==null || device==null)return;
        texture.setDefaultBufferSize(preview.getWidth(),preview.getHeight());
        Surface target=new Surface(texture);
        try{
            CaptureRequest.Builder request=device.createCaptureRequest(CameraDevice.TEMPLATE_PREVIEW);
            request.addTarget(target);
            request.set(CaptureRequest.CONTROL_AF_MODE,CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE);
            device.createCaptureSession(Collections.singletonList(target),new CameraCaptureSession.StateCallback(){
                @Override public void onConfigured(CameraCaptureSession configured){activity.runOnUiThread(()->{
                    if(token!=generation || device==null){configured.close();return;}
                    session=configured;
                    try{configured.setRepeatingRequest(request.build(),null,background);transform();
                        host.emit(new JSONObject().put("type","opened").put("width",preview.getWidth()).put("height",preview.getHeight()).put("lens",lens));
                    }catch(CameraAccessException | IllegalStateException | JSONException failed){error("failed");close();}
                });}
                @Override public void onConfigureFailed(CameraCaptureSession failed){activity.runOnUiThread(()->{error("failed");close();});}
            },background);
        }catch(CameraAccessException | IllegalStateException failed){error("failed");close();}
    }

    /**
     * Centre-crop the preview into the slot, upright for the current display rotation.
     *
     * The camera HAL's SurfaceTexture transform already presents the buffer upright in the
     * device's natural orientation (including front-camera mirroring), and TextureView stretches
     * that to the view. So, in stretched view coordinates: undo the stretch to natural content
     * size, rotate by the display rotation about the centre, then scale uniformly to cover.
     */
    private void transform(){
        if(preview==null)return;
        float w=view.getWidth(),h=view.getHeight();if(w==0 || h==0)return;
        int display=activity.getWindowManager().getDefaultDisplay().getRotation()*90;
        boolean sideways=sensorOrientation%180!=0;
        float nw=sideways?preview.getHeight():preview.getWidth(),nh=sideways?preview.getWidth():preview.getHeight();
        float dw=display%180!=0?nh:nw,dh=display%180!=0?nw:nh;
        float scale=Math.max(w/dw,h/dh);
        Matrix m=new Matrix();
        m.setScale(nw/w,nh/h);
        m.postTranslate(-nw/2f,-nh/2f);
        m.postRotate(-display);
        m.postScale(scale,scale);
        m.postTranslate(w/2f,h/2f);
        view.setTransform(m);
    }

    private void close(){
        generation++;opening=false;
        if(session!=null){try{session.close();}catch(RuntimeException ignored){}session=null;}
        if(device!=null){device.close();device=null;try{host.emit(new JSONObject().put("type","closed"));}catch(JSONException | IllegalStateException ignored){}}
    }

    @Override public void resumed(){resumed=true;maybeOpen();}
    @Override public void paused(){resumed=false;close();}
    @Override public void dispose(){if(disposed)return;disposed=true;close();view.setSurfaceTextureListener(null);if(thread!=null){thread.quitSafely();thread=null;}}

    @Override public void onSurfaceTextureAvailable(SurfaceTexture texture,int width,int height){maybeOpen();}
    @Override public void onSurfaceTextureSizeChanged(SurfaceTexture texture,int width,int height){transform();}
    @Override public boolean onSurfaceTextureDestroyed(SurfaceTexture texture){close();return true;}
    @Override public void onSurfaceTextureUpdated(SurfaceTexture texture){}
}
