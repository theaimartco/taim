package com.theaimartco.taim;

import android.content.Intent;
import android.nfc.*;
import android.nfc.tech.Ndef;
import android.os.*;
import android.util.Base64;
import org.json.*;
import java.lang.ref.WeakReference;

/**
 * Foreground NFC reader mode and NDEF read/write. Channel {@code taim.nfc}.
 * Android only allows reader mode while the activity is resumed: it is re-enabled on resume
 * and disabled on pause automatically while the app has requested reading.
 */
final class TaimNfc implements TaimPlugin, NfcAdapter.ReaderCallback {
    static final String CHANNEL="taim.nfc";
    private final WeakReference<TaimActivity> activity;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final HandlerThread io=new HandlerThread("taim-nfc");
    private Handler ioHandler;
    private boolean reading;
    private Tag last;
    private long tagSerial;

    TaimNfc(TaimActivity host){activity=new WeakReference<>(host);}

    private NfcAdapter adapter(){TaimActivity host=activity.get();return host==null?null:NfcAdapter.getDefaultAdapter(host);}
    private Handler io(){if(ioHandler==null){io.start();ioHandler=new Handler(io.getLooper());}return ioHandler;}

    @Override public void call(String method,JSONObject args,Result result){
        try{
            NfcAdapter adapter=adapter();
            switch(method){
                case "status":result.success(new JSONObject().put("supported",adapter!=null).put("enabled",adapter!=null && adapter.isEnabled()).put("reading",reading));return;
                case "settings":{TaimActivity host=activity.get();if(host==null){result.error("unavailable");return;}host.startActivity(new Intent(android.provider.Settings.ACTION_NFC_SETTINGS));result.success(null);return;}
                case "startReader":{
                    if(adapter==null){result.error("unsupported");return;}
                    if(!adapter.isEnabled()){result.error("disabled");return;}
                    TaimActivity host=activity.get();if(host==null || !host.isTaimResumed()){result.error("cancelledForeground");return;}
                    reading=true;enable();result.success(null);return;
                }
                case "stopReader":reading=false;disable();result.success(null);return;
                case "writeNdef":write(args,result);return;
                default:result.error("invalid");
            }
        }catch(JSONException | IllegalArgumentException bad){result.error("invalid");}
    }

    private void enable(){
        NfcAdapter adapter=adapter();TaimActivity host=activity.get();
        if(adapter==null || host==null || !reading)return;
        int flags=NfcAdapter.FLAG_READER_NFC_A|NfcAdapter.FLAG_READER_NFC_B|NfcAdapter.FLAG_READER_NFC_F|NfcAdapter.FLAG_READER_NFC_V|NfcAdapter.FLAG_READER_NFC_BARCODE;
        Bundle extras=new Bundle();extras.putInt(NfcAdapter.EXTRA_READER_PRESENCE_CHECK_DELAY,250);
        try{adapter.enableReaderMode(host,this,flags,extras);}catch(RuntimeException ignored){}
    }
    private void disable(){NfcAdapter adapter=adapter();TaimActivity host=activity.get();if(adapter!=null && host!=null){try{adapter.disableReaderMode(host);}catch(RuntimeException ignored){}}}

    /** Binder thread. Reads NDEF here (blocking IO), then reports on the main thread. */
    @Override public void onTagDiscovered(Tag tag){
        JSONObject event;
        try{
            JSONArray techs=new JSONArray();for(String tech:tag.getTechList())techs.put(tech.substring(tech.lastIndexOf('.')+1));
            event=new JSONObject().put("type","tag").put("id",hex(tag.getId())).put("techs",techs).put("ndef",readNdef(tag));
        }catch(JSONException failed){return;}
        main.post(()->{
            last=tag;tagSerial++;
            TaimActivity host=activity.get();
            if(host!=null){try{event.put("tag",tagSerial);}catch(JSONException ignored){}host.emitTaimPluginEvent(CHANNEL,event);}
        });
    }

    private static Object readNdef(Tag tag)throws JSONException {
        Ndef ndef=Ndef.get(tag);if(ndef==null)return JSONObject.NULL;
        JSONObject result=new JSONObject().put("type",ndef.getType()).put("maxSize",ndef.getMaxSize()).put("writable",ndef.isWritable());
        JSONArray records=new JSONArray();
        try{
            ndef.connect();NdefMessage message=ndef.getNdefMessage();
            if(message!=null)for(NdefRecord r:message.getRecords())records.put(new JSONObject().put("tnf",(int)r.getTnf())
                .put("type",Base64.encodeToString(r.getType(),Base64.NO_WRAP)).put("id",Base64.encodeToString(r.getId(),Base64.NO_WRAP))
                .put("payload",Base64.encodeToString(r.getPayload(),Base64.NO_WRAP)));
        }catch(Exception lost){result.put("error","readFailed");}
        finally{try{ndef.close();}catch(Exception ignored){}}
        return result.put("records",records);
    }

    private void write(JSONObject args,Result result)throws JSONException {
        if(last==null || args.getLong("tag")!=tagSerial){result.error("notFound");return;}
        JSONArray input=args.getJSONArray("records");if(input.length()==0 || input.length()>32)throw new IllegalArgumentException();
        NdefRecord[] records=new NdefRecord[input.length()];
        for(int i=0;i<records.length;i++){
            JSONObject r=input.getJSONObject(i);int tnf=r.getInt("tnf");if(tnf<0 || tnf>6)throw new IllegalArgumentException();
            records[i]=new NdefRecord((short)tnf,Base64.decode(r.getString("type"),Base64.NO_WRAP),Base64.decode(r.optString("id",""),Base64.NO_WRAP),Base64.decode(r.getString("payload"),Base64.NO_WRAP));
        }
        NdefMessage message=new NdefMessage(records);Tag tag=last;
        io().post(()->{
            String error=null;Ndef ndef=Ndef.get(tag);
            if(ndef==null)error="unsupported";
            else try{
                ndef.connect();
                if(!ndef.isWritable())error="denied";
                else if(message.getByteArrayLength()>ndef.getMaxSize())error="invalid";
                else ndef.writeNdefMessage(message);
            }catch(Exception lost){error="unavailable";} // tag moved away or IO failed
            finally{try{ndef.close();}catch(Exception ignored){}}
            String code=error;main.post(()->{if(code==null)result.success(null);else result.error(code);});
        });
    }

    private static String hex(byte[] bytes){StringBuilder s=new StringBuilder();for(byte b:bytes)s.append(String.format("%02x",b));return s.toString();}

    @Override public void resumed(){if(reading)enable();}
    @Override public void paused(){if(reading)disable();}
    @Override public void detach(){reading=false;disable();last=null;if(ioHandler!=null)io.quitSafely();}
}
