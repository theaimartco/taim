package com.theaimartco.taim.mediadriver;
import android.content.*;
import android.database.*;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.*;
/** Private fixture provider; the responder grants individual test URIs to the target app. */
public final class MediaProvider extends ContentProvider {
    static byte[] bytes(String name)throws IOException {
        if(name.equals("photo.png") || name.equals("second.png") || name.equals("capture.jpg")){
            android.graphics.Bitmap image=android.graphics.Bitmap.createBitmap(2,2,android.graphics.Bitmap.Config.ARGB_8888);image.eraseColor(android.graphics.Color.GREEN);
            ByteArrayOutputStream out=new ByteArrayOutputStream();image.compress(name.equals("capture.jpg")?android.graphics.Bitmap.CompressFormat.JPEG:android.graphics.Bitmap.CompressFormat.PNG,90,out);image.recycle();return out.toByteArray();
        }
        if(name.equals("movie.mp4"))return "fixture-video-container-protocol-only".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        if(name.equals("audio.bin"))return new byte[]{1,2,3};throw new FileNotFoundException();
    }
    @Override public boolean onCreate(){return true;}
    @Override public String getType(Uri uri){String name=uri.getLastPathSegment();return "movie.mp4".equals(name)?"video/mp4":"audio.bin".equals(name)?"audio/wav":"image/png";}
    @Override public ParcelFileDescriptor openFile(Uri uri,String mode)throws FileNotFoundException {
        if(!mode.equals("r"))throw new FileNotFoundException();try{String name=uri.getLastPathSegment();byte[] data=bytes(name);File file=new File(getContext().getCacheDir(),name);try(FileOutputStream out=new FileOutputStream(file)){out.write(data);}return ParcelFileDescriptor.open(file,ParcelFileDescriptor.MODE_READ_ONLY);}catch(IOException error){throw new FileNotFoundException();}
    }
    @Override public Cursor query(Uri uri,String[] projection,String selection,String[] args,String sort){try{String name=uri.getLastPathSegment();byte[] bytes=bytes(name);MatrixCursor result=new MatrixCursor(new String[]{OpenableColumns.DISPLAY_NAME,OpenableColumns.SIZE});result.addRow(new Object[]{name,bytes.length});return result;}catch(IOException error){return null;}}
    @Override public Uri insert(Uri uri,ContentValues values){throw new UnsupportedOperationException();}
    @Override public int update(Uri uri,ContentValues values,String s,String[] a){throw new UnsupportedOperationException();}
    @Override public int delete(Uri uri,String s,String[] a){throw new UnsupportedOperationException();}
}
