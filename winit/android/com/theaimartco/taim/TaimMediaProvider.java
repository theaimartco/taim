package com.theaimartco.taim;

import android.content.*;
import android.content.pm.ProviderInfo;
import android.database.*;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.*;
import java.util.UUID;

/** Optional, non-exported grant-only provider exposing only framework-created capture files. */
public final class TaimMediaProvider extends ContentProvider {
    static String authority(Context context){return context.getPackageName()+".taim.media";}
    static File directory(Context context){return new File(context.getFilesDir(),"taim-media");}
    static File file(Context context,Uri uri)throws FileNotFoundException {
        if(!"content".equals(uri.getScheme()) || !authority(context).equals(uri.getAuthority())
            || uri.getQuery()!=null || uri.getFragment()!=null || uri.getPathSegments().size()!=2
            || !"capture".equals(uri.getPathSegments().get(0)))throw new FileNotFoundException();
        String name=uri.getLastPathSegment();
        if(name==null || !name.matches("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\\.(jpg|mp4)"))throw new FileNotFoundException();
        try {File root=directory(context).getCanonicalFile();File file=new File(root,name).getCanonicalFile();
            if(!root.equals(file.getParentFile()))throw new FileNotFoundException();return file;
        }catch(IOException error){throw new FileNotFoundException();}
    }
    static Uri create(Context context,boolean video)throws IOException {
        File root=directory(context);if(!root.isDirectory() && !root.mkdirs())throw new IOException();
        String[] files=root.list();if(files==null)throw new IOException();
        if(files.length>=64)throw new java.util.concurrent.RejectedExecutionException();
        String name=UUID.randomUUID().toString()+(video?".mp4":".jpg");File output=new File(root,name);
        if(!output.createNewFile())throw new IOException();
        return new Uri.Builder().scheme("content").authority(authority(context)).appendPath("capture").appendPath(name).build();
    }
    @Override public void attachInfo(Context context,ProviderInfo info){
        if(info.exported || !info.grantUriPermissions || !authority(context).equals(info.authority))throw new SecurityException("Taim media provider must be private and grant-enabled");
        super.attachInfo(context,info);
    }
    @Override public boolean onCreate(){return true;}
    @Override public String getType(Uri uri){try{File file=file(getContext(),uri);return file.getName().endsWith(".jpg")?"image/jpeg":"video/mp4";}catch(FileNotFoundException error){return null;}}
    @Override public ParcelFileDescriptor openFile(Uri uri,String mode)throws FileNotFoundException {
        File file=file(getContext(),uri);if(!file.isFile())throw new FileNotFoundException();
        return ParcelFileDescriptor.open(file,ParcelFileDescriptor.parseMode(mode)&~ParcelFileDescriptor.MODE_CREATE);
    }
    @Override public Cursor query(Uri uri,String[] projection,String selection,String[] args,String order){
        try {File file=file(getContext(),uri);if(!file.isFile())return null;
            String[] columns=projection==null?new String[]{OpenableColumns.DISPLAY_NAME,OpenableColumns.SIZE}:projection;
            MatrixCursor result=new MatrixCursor(columns);Object[] values=new Object[columns.length];
            for(int i=0;i<columns.length;i++){if(OpenableColumns.DISPLAY_NAME.equals(columns[i]))values[i]=file.getName();else if(OpenableColumns.SIZE.equals(columns[i]))values[i]=file.length();}
            result.addRow(values);return result;
        }catch(FileNotFoundException error){return null;}
    }
    @Override public Uri insert(Uri uri,ContentValues values){throw new UnsupportedOperationException();}
    @Override public int update(Uri uri,ContentValues values,String selection,String[] args){throw new UnsupportedOperationException();}
    @Override public int delete(Uri uri,String selection,String[] args){throw new UnsupportedOperationException();}
}
