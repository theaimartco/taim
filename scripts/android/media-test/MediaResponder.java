package com.theaimartco.taim.mediadriver;
import android.app.*;
import android.content.*;
import android.net.Uri;
import android.os.*;
import android.provider.MediaStore;
import java.io.*;
/** External-UID protocol responder, not a substitute for hardware/codec acceptance. */
public final class MediaResponder extends Activity {
    static Uri item(String name){return Uri.parse("content://com.theaimartco.taim.mediadriver.media/"+name);}
    @Override public void onCreate(Bundle state){super.onCreate(state);Intent request=getIntent();String mode=request.getStringExtra("taim.fixture.mode");
        if(Intent.ACTION_VIEW.equals(request.getAction())){
            boolean passed=false;try {byte[] bytes;try(InputStream input=getContentResolver().openInputStream(request.getData())){bytes=new byte[32];if(input.read(bytes)<=0)throw new IOException();}
                boolean denied=false;try(OutputStream ignored=getContentResolver().openOutputStream(request.getData(),"wa")){}catch(SecurityException|FileNotFoundException expected){denied=true;}passed=denied;
            }catch(Exception error){}android.widget.TextView view=new android.widget.TextView(this);view.setText(passed?"Media viewer verified":"Media viewer failed");view.setTextSize(24);setContentView(view);return;
        }
        if(!"com.theaimartco.taim.mediaprobe".equals(getCallingPackage())){setResult(RESULT_CANCELED);finish();return;}
        Runnable respond=()->{try {
            if(MediaStore.ACTION_IMAGE_CAPTURE.equals(request.getAction()) || MediaStore.ACTION_VIDEO_CAPTURE.equals(request.getAction())){
                Uri output=request.getParcelableExtra(MediaStore.EXTRA_OUTPUT);if(output==null || (request.getFlags()&3)!=3)throw new IOException();
                boolean video=MediaStore.ACTION_VIDEO_CAPTURE.equals(request.getAction());
                if(video && (request.getIntExtra(MediaStore.EXTRA_DURATION_LIMIT,0)!=15 || request.getIntExtra(MediaStore.EXTRA_VIDEO_QUALITY,0)!=1))throw new IOException();
                try(OutputStream stream=getContentResolver().openOutputStream(output,"wt")){stream.write(MediaProvider.bytes(video?"movie.mp4":"capture.jpg"));}
                setResult("cancel".equals(mode)?RESULT_CANCELED:RESULT_OK,new Intent().setData(item("unexpected-returned-uri")));finish();return;
            }
            if("cancel".equals(mode)){setResult(RESULT_CANCELED);finish();return;}
            String type=request.getType();boolean video="video/*".equals(type),visual=type==null || "*/*".equals(type);
            Uri first=item("wrongMime".equals(mode)?"audio.bin":video?"movie.mp4":"photo.png");
            ClipData clip=ClipData.newRawUri("Taim fixture",first);
            if("overcount".equals(mode) || !video)clip.addItem(new ClipData.Item(item(visual?"movie.mp4":"second.png")));
            Intent result=new Intent().addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);result.setClipData(clip);setResult(RESULT_OK,result);finish();
        }catch(Exception error){setResult(RESULT_CANCELED);finish();}};
        if("slow".equals(mode))new Handler(Looper.getMainLooper()).postDelayed(respond,1000);else respond.run();
    }
}
