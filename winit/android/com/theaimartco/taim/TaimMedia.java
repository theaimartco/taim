package com.theaimartco.taim;

import android.app.Activity;
import android.content.*;
import android.content.pm.*;
import android.database.Cursor;
import android.net.Uri;
import android.os.*;
import android.provider.*;
import org.json.*;
import java.io.*;
import java.util.*;

/** Media lifecycle adapter. Never requests broad storage/camera permissions implicitly. */
final class TaimMedia {
    static Uri uri(String value){
        Uri uri=Uri.parse(value);if(value.length()>8192 || !"content".equals(uri.getScheme()) || uri.getAuthority()==null
            || uri.getAuthority().isEmpty() || uri.getFragment()!=null)throw new IllegalArgumentException();return uri;
    }
    static boolean persisted(Context context,Uri uri){for(UriPermission permission:context.getContentResolver().getPersistedUriPermissions())if(permission.getUri().equals(uri) && permission.isReadPermission())return true;return false;}
    static JSONObject info(Context context,Uri uri,CancellationSignal cancel)throws Exception {
        cancel.throwIfCanceled();ContentResolver resolver=context.getContentResolver();String mime=resolver.getType(uri);
        JSONObject value=new JSONObject().put("uri",uri.toString()).put("mime",mime==null?JSONObject.NULL:mime)
            .put("name",JSONObject.NULL).put("size",JSONObject.NULL).put("persisted",persisted(context,uri))
            .put("owned",TaimMediaProvider.authority(context).equals(uri.getAuthority()));
        try(Cursor cursor=resolver.query(uri,new String[]{OpenableColumns.DISPLAY_NAME,OpenableColumns.SIZE},null,null,null,cancel)) {
            if(cursor!=null && cursor.moveToFirst()) {
                int name=cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME),size=cursor.getColumnIndex(OpenableColumns.SIZE);
                if(name>=0 && !cursor.isNull(name)){String text=cursor.getString(name);if(text.length()>1024)text=text.substring(0,1024);value.put("name",text);}
                if(size>=0 && !cursor.isNull(size)){long bytes=cursor.getLong(size);if(bytes>=0)value.put("size",bytes);}
            }
        }
        cancel.throwIfCanceled();return value;
    }
    static Object io(Context context,JSONObject request,CancellationSignal cancel)throws Exception {
        String op=request.getString("op");cancel.throwIfCanceled();
        if(op.equals("mediaList")){
            JSONArray items=new JSONArray();String[] names=TaimMediaProvider.directory(context).list();
            if(names!=null)for(String name:names){if(items.length()>=64)break;
                Uri candidate=new Uri.Builder().scheme("content").authority(TaimMediaProvider.authority(context)).appendPath("capture").appendPath(name).build();
                try{if(TaimMediaProvider.file(context,candidate).isFile())items.put(info(context,candidate,cancel));}catch(FileNotFoundException ignored){}
            }
            return new JSONObject().put("items",items).put("source","owned");
        }
        Uri uri=uri(request.getString("uri"));
        if(op.equals("mediaRetain")){context.getContentResolver().takePersistableUriPermission(uri,Intent.FLAG_GRANT_READ_URI_PERMISSION);return info(context,uri,cancel);}
        if(op.equals("mediaRelease")){
            for(UriPermission permission:context.getContentResolver().getPersistedUriPermissions())if(permission.getUri().equals(uri)) {
                int flags=(permission.isReadPermission()?Intent.FLAG_GRANT_READ_URI_PERMISSION:0)|(permission.isWritePermission()?Intent.FLAG_GRANT_WRITE_URI_PERMISSION:0);
                context.getContentResolver().releasePersistableUriPermission(uri,flags);
            }
            return JSONObject.NULL;
        }
        if(op.equals("mediaDelete")){
            File file;try{file=TaimMediaProvider.file(context,uri);}catch(FileNotFoundException invalid){throw new IllegalArgumentException();}
            context.revokeUriPermission(uri,Intent.FLAG_GRANT_READ_URI_PERMISSION|Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
            if(file.exists() && !file.delete())throw new IOException();return JSONObject.NULL;
        }
        return info(context,uri,cancel);
    }
    static void preview(Activity host,Uri uri,String mime)throws Exception {
        if(mime==null || !(mime.startsWith("image/") || mime.startsWith("video/")))throw new IllegalArgumentException();
        Intent intent=new Intent(Intent.ACTION_VIEW).setDataAndType(uri,mime).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        intent.setClipData(ClipData.newRawUri("Taim media",uri));
        host.startActivity(intent);
    }
    static final class Session {
        final Context context;final boolean capture;final String kind;final int maximum;Uri output;String source;
        Session(Activity host,JSONObject request)throws Exception {
            context=host.getApplicationContext();capture=request.getString("op").equals("mediaCapture");kind=request.getString("kind");
            if(!Arrays.asList("image","video","visual").contains(kind) || (capture && kind.equals("visual")))throw new IllegalArgumentException();
            maximum=capture?1:request.getInt("maximum");if(maximum<1 || maximum>32)throw new IllegalArgumentException();
            if(capture){
                ProviderInfo provider;
                try{provider=host.getPackageManager().getProviderInfo(new ComponentName(host,TaimMediaProvider.class),0);}catch(PackageManager.NameNotFoundException absent){throw new UnsupportedOperationException();}
                if(provider.exported || !provider.grantUriPermissions || !TaimMediaProvider.authority(host).equals(provider.authority))throw new IllegalArgumentException();
                output=TaimMediaProvider.create(context,kind.equals("video"));
            }
        }
        void launch(Activity host,JSONObject request,int code)throws Exception {
            if(capture){
                Intent intent=new Intent(kind.equals("image")?MediaStore.ACTION_IMAGE_CAPTURE:MediaStore.ACTION_VIDEO_CAPTURE)
                    .putExtra(MediaStore.EXTRA_OUTPUT,output)
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION|Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
                intent.setClipData(ClipData.newRawUri("Taim capture",output));
                if(kind.equals("video")){int duration=request.getInt("duration");if(duration<1 || duration>60)throw new IllegalArgumentException();intent.putExtra(MediaStore.EXTRA_DURATION_LIMIT,duration).putExtra(MediaStore.EXTRA_VIDEO_QUALITY,request.getBoolean("highQuality")?1:0);}
                source="camera";host.startActivityForResult(intent,code);return;
            }
            if(Build.VERSION.SDK_INT>=33){try{source="photoPicker";host.startActivityForResult(Api33.picker(kind,maximum),code);return;}catch(ActivityNotFoundException absent){}}
            source="documents";Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE)
                .setType(kind.equals("visual")?"*/*":kind+"/*").putExtra(Intent.EXTRA_ALLOW_MULTIPLE,maximum>1)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION|Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            if(kind.equals("visual"))intent.putExtra(Intent.EXTRA_MIME_TYPES,new String[]{"image/*","video/*"});
            host.startActivityForResult(intent,code);
        }
        JSONObject result(Intent data,CancellationSignal cancel)throws Exception {
            LinkedHashSet<Uri> selected=new LinkedHashSet<>();
            if(capture){if(output==null || TaimMediaProvider.file(context,output).length()==0)throw new IOException();selected.add(output);}
            else if(data!=null){
                if(data.getData()!=null)selected.add(uri(data.getData().toString()));
                ClipData clip=data.getClipData();if(clip!=null){if(clip.getItemCount()>32)throw new IllegalArgumentException();for(int i=0;i<clip.getItemCount();i++){Uri uri=clip.getItemAt(i).getUri();if(uri==null)throw new IllegalArgumentException();selected.add(uri(uri.toString()));}}
            }
            if(selected.isEmpty() || selected.size()>maximum)throw new IllegalArgumentException();
            JSONArray items=new JSONArray();for(Uri uri:selected){JSONObject item=info(context,uri,cancel);String mime=item.optString("mime","");
                if(!(kind.equals("visual")?(mime.startsWith("image/") || mime.startsWith("video/")):mime.startsWith(kind+"/")))throw new IllegalArgumentException();items.put(item);}
            return new JSONObject().put("items",items).put("source",source);
        }
        void close(boolean success){if(output!=null){context.revokeUriPermission(output,Intent.FLAG_GRANT_READ_URI_PERMISSION|Intent.FLAG_GRANT_WRITE_URI_PERMISSION);if(!success){try{TaimMediaProvider.file(context,output).delete();}catch(IOException ignored){}}}}
    }
    @SuppressWarnings("UseRequiresApi") // framework-only host: no AndroidX @RequiresApi
    @android.annotation.TargetApi(33)
    private static final class Api33 {
        static Intent picker(String kind,int maximum){Intent intent=new Intent(MediaStore.ACTION_PICK_IMAGES);if(!kind.equals("visual"))intent.setType(kind+"/*");if(maximum>1)intent.putExtra(MediaStore.EXTRA_PICK_IMAGES_MAX,Math.min(maximum,MediaStore.getPickImagesMaxLimit()));return intent;}
    }
}
