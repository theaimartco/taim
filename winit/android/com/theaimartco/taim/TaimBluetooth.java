package com.theaimartco.taim;

import android.Manifest;
import android.bluetooth.*;
import android.bluetooth.le.*;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.*;
import android.util.Base64;
import org.json.*;
import java.lang.ref.WeakReference;
import java.util.*;

/**
 * Bluetooth Low Energy central role: bounded scans, GATT connections, characteristic
 * read/write and notifications. Channel {@code taim.bluetooth}.
 *
 * GATT allows one outstanding operation per connection; a second concurrent call fails
 * with {@code busy} rather than being queued invisibly. Scans stop when the activity pauses;
 * connections stay open until disconnected, the activity is destroyed or the peer drops.
 */
// Every entry point checks scanGranted()/connectGranted() before touching the adapter, and call()
// maps a SecurityException from a concurrent revocation to "denied"; lint cannot see through the helpers.
@android.annotation.SuppressLint("MissingPermission")
final class TaimBluetooth implements TaimPlugin {
    static final String CHANNEL="taim.bluetooth";
    private static final UUID CCCD=UUID.fromString("00002902-0000-1000-8000-00805f9b34fb");
    private static final int MAX_CONNECTIONS=7, MAX_RESULTS=128, MAX_VALUE=512;
    private final WeakReference<TaimActivity> activity;
    private final Context context;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final Map<Long,Connection> connections=new HashMap<>();
    private long nextHandle=1;
    private Scan scan;

    TaimBluetooth(TaimActivity host){activity=new WeakReference<>(host);context=host.getApplicationContext();}

    private BluetoothAdapter adapter(){BluetoothManager manager=context.getSystemService(BluetoothManager.class);return manager==null?null:manager.getAdapter();}
    private boolean granted(String permission){return context.checkSelfPermission(permission)==PackageManager.PERMISSION_GRANTED;}
    private boolean scanGranted(){return Build.VERSION.SDK_INT>=31?granted(Manifest.permission.BLUETOOTH_SCAN):granted(Manifest.permission.ACCESS_FINE_LOCATION);}
    private boolean connectGranted(){return Build.VERSION.SDK_INT<31 || granted(Manifest.permission.BLUETOOTH_CONNECT);}
    private void emit(JSONObject event){TaimActivity host=activity.get();if(host!=null)host.emitTaimPluginEvent(CHANNEL,event);}

    @Override public void call(String method,JSONObject args,Result result){
        try {
            switch(method){
                case "status":result.success(status());return;
                case "settings":{TaimActivity host=activity.get();if(host==null){result.error("unavailable");return;}host.startActivity(new Intent(android.provider.Settings.ACTION_BLUETOOTH_SETTINGS));result.success(null);return;}
                case "scan":scan(args,result);return;
                case "stopScan":if(scan!=null)scan.finish();result.success(null);return;
                case "connect":connect(args,result);return;
                case "read":case "write":case "subscribe":gatt(method,args,result);return;
                case "disconnect":{Connection c=connections.get(args.getLong("handle"));if(c!=null)c.close(0);result.success(null);return;}
                default:result.error("invalid");
            }
        }catch(SecurityException denied){result.error("denied");}
        catch(JSONException | IllegalArgumentException bad){result.error("invalid");}
    }

    private JSONObject status()throws JSONException {
        BluetoothAdapter adapter=adapter();
        boolean le=context.getPackageManager().hasSystemFeature(PackageManager.FEATURE_BLUETOOTH_LE);
        boolean enabled=false;
        try{enabled=adapter!=null && adapter.isEnabled();}catch(SecurityException ignored){}
        return new JSONObject().put("supported",adapter!=null).put("le",le).put("enabled",enabled)
            .put("scanPermission",scanGranted()).put("connectPermission",connectGranted());
    }

    private BluetoothAdapter ready(boolean forScan,Result result){
        BluetoothAdapter adapter=adapter();
        if(adapter==null || !context.getPackageManager().hasSystemFeature(PackageManager.FEATURE_BLUETOOTH_LE)){result.error("unsupported");return null;}
        if(forScan?!scanGranted():!connectGranted()){result.error("denied");return null;}
        if(!adapter.isEnabled()){result.error("disabled");return null;}
        return adapter;
    }

    private static UUID uuid(String value){if(value.length()>64)throw new IllegalArgumentException();return UUID.fromString(value);}

    // ---- scanning ----
    private void scan(JSONObject args,Result result)throws JSONException {
        TaimActivity host=activity.get();
        if(host==null || !host.isTaimResumed()){result.error("cancelledForeground");return;}
        if(scan!=null){result.error("busy");return;}
        int duration=args.getInt("durationMs");if(duration<500 || duration>30000)throw new IllegalArgumentException();
        List<ScanFilter> filters=new ArrayList<>();
        JSONArray services=args.optJSONArray("services");
        if(services!=null){if(services.length()>16)throw new IllegalArgumentException();for(int i=0;i<services.length();i++)filters.add(new ScanFilter.Builder().setServiceUuid(new ParcelUuid(uuid(services.getString(i)))).build());}
        String prefix=args.isNull("namePrefix")?null:args.optString("namePrefix",null);
        if(prefix!=null && (prefix.isEmpty() || prefix.length()>64))throw new IllegalArgumentException();
        BluetoothAdapter adapter=ready(true,result);if(adapter==null)return;
        BluetoothLeScanner scanner=adapter.getBluetoothLeScanner();if(scanner==null){result.error("disabled");return;}
        scan=new Scan(scanner,prefix,result);
        ScanSettings settings=new ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build();
        scanner.startScan(filters.isEmpty()?null:filters,settings,scan);
        main.postDelayed(scan.deadline,duration);
    }

    private final class Scan extends ScanCallback {
        final BluetoothLeScanner scanner;final String prefix;final Result result;
        final LinkedHashMap<String,JSONObject> seen=new LinkedHashMap<>();
        final Runnable deadline=this::finish;boolean done;
        Scan(BluetoothLeScanner scanner,String prefix,Result result){this.scanner=scanner;this.prefix=prefix;this.result=result;}
        @Override public void onScanResult(int type,ScanResult found){main.post(()->add(found));}
        @Override public void onBatchScanResults(List<ScanResult> found){main.post(()->{for(ScanResult r:found)add(r);});}
        @Override public void onScanFailed(int code){main.post(()->end(null,code==ScanCallback.SCAN_FAILED_ALREADY_STARTED?"busy":"failed"));}
        private void add(ScanResult found){
            if(done)return;
            try{
                BluetoothDevice device=found.getDevice();String name=null;
                ScanRecord record=found.getScanRecord();
                if(record!=null)name=record.getDeviceName();
                if(name==null){try{name=device.getName();}catch(SecurityException ignored){}}
                if(prefix!=null && (name==null || !name.startsWith(prefix)))return;
                String address=device.getAddress();
                if(!seen.containsKey(address) && seen.size()>=MAX_RESULTS)return;
                JSONArray uuids=new JSONArray();
                if(record!=null && record.getServiceUuids()!=null)for(ParcelUuid u:record.getServiceUuids())uuids.put(u.toString());
                JSONObject entry=new JSONObject().put("address",address).put("name",name==null?JSONObject.NULL:name)
                    .put("rssi",found.getRssi()).put("services",uuids)
                    .put("connectable",found.isConnectable());
                boolean first=!seen.containsKey(address);
                seen.put(address,entry);
                if(first)emit(new JSONObject().put("type","scanResult").put("device",entry));
            }catch(JSONException ignored){}
        }
        void finish(){
            JSONArray devices=new JSONArray();for(JSONObject entry:seen.values())devices.put(entry);
            end(devices,null);
        }
        void end(Object value,String error){
            if(done)return;done=true;main.removeCallbacks(deadline);
            try{scanner.stopScan(this);}catch(RuntimeException ignored){}
            if(scan==this)scan=null;
            if(error==null)result.success(value);else result.error(error);
        }
    }

    // ---- GATT ----
    private void connect(JSONObject args,Result result)throws JSONException {
        String address=args.getString("address");
        if(!BluetoothAdapter.checkBluetoothAddress(address))throw new IllegalArgumentException();
        int timeout=args.optInt("timeoutMs",15000);if(timeout<1000 || timeout>60000)throw new IllegalArgumentException();
        if(connections.size()>=MAX_CONNECTIONS){result.error("busy");return;}
        BluetoothAdapter adapter=ready(false,result);if(adapter==null)return;
        long handle=nextHandle++;
        Connection connection=new Connection(handle);connections.put(handle,connection);
        connection.pending=result;connection.pendingKind="connect";
        main.postDelayed(connection.connectTimeout,timeout);
        BluetoothDevice device=adapter.getRemoteDevice(address);
        connection.gatt=device.connectGatt(context,false,connection,BluetoothDevice.TRANSPORT_LE);
        if(connection.gatt==null){connections.remove(handle);main.removeCallbacks(connection.connectTimeout);result.error("failed");}
    }

    private void gatt(String method,JSONObject args,Result result)throws JSONException {
        Connection c=connections.get(args.getLong("handle"));
        if(c==null || !c.ready){result.error("notFound");return;}
        if(!connectGranted()){result.error("denied");return;}
        if(c.pending!=null){result.error("busy");return;}
        BluetoothGattService service=c.gatt.getService(uuid(args.getString("service")));
        BluetoothGattCharacteristic ch=service==null?null:service.getCharacteristic(uuid(args.getString("characteristic")));
        if(ch==null){result.error("notFound");return;}
        boolean started;
        switch(method){
            case "read":started=c.gatt.readCharacteristic(ch);break;
            case "write":{
                byte[] value=Base64.decode(args.getString("value"),Base64.NO_WRAP);
                if(value.length>MAX_VALUE)throw new IllegalArgumentException();
                int type=args.optBoolean("withResponse",true)?BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT:BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE;
                if(Build.VERSION.SDK_INT>=33)started=c.gatt.writeCharacteristic(ch,value,type)==BluetoothStatusCodes.SUCCESS;
                else {ch.setWriteType(type);ch.setValue(value);started=c.gatt.writeCharacteristic(ch);}
                break;
            }
            default:{ // subscribe
                boolean enable=args.getBoolean("enable");
                int props=ch.getProperties();
                boolean indicate=(props&BluetoothGattCharacteristic.PROPERTY_INDICATE)!=0 && (props&BluetoothGattCharacteristic.PROPERTY_NOTIFY)==0;
                if(enable && (props&(BluetoothGattCharacteristic.PROPERTY_NOTIFY|BluetoothGattCharacteristic.PROPERTY_INDICATE))==0){result.error("unsupported");return;}
                BluetoothGattDescriptor cccd=ch.getDescriptor(CCCD);if(cccd==null){result.error("unsupported");return;}
                if(!c.gatt.setCharacteristicNotification(ch,enable)){result.error("failed");return;}
                byte[] value=!enable?BluetoothGattDescriptor.DISABLE_NOTIFICATION_VALUE:indicate?BluetoothGattDescriptor.ENABLE_INDICATION_VALUE:BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE;
                if(Build.VERSION.SDK_INT>=33)started=c.gatt.writeDescriptor(cccd,value)==BluetoothStatusCodes.SUCCESS;
                else {cccd.setValue(value);started=c.gatt.writeDescriptor(cccd);}
            }
        }
        if(!started){result.error("busy");return;}
        c.pending=result;c.pendingKind=method;
        main.postDelayed(c.opTimeout,10000);
    }

    private final class Connection extends BluetoothGattCallback {
        final long handle;BluetoothGatt gatt;boolean ready,closed;Result pending;String pendingKind;
        final Runnable connectTimeout=()->{if(!ready)fail("timeout");};
        final Runnable opTimeout=()->reply(null,"timeout");
        Connection(long handle){this.handle=handle;}
        private void reply(Object value,String error){
            main.removeCallbacks(opTimeout);
            Result r=pending;pending=null;pendingKind=null;
            if(r==null)return;
            if(error==null)r.success(value);else r.error(error);
        }
        private void fail(String error){reply(null,error);close(-1);}
        void close(int status){
            if(closed)return;closed=true;ready=false;connections.remove(handle);
            main.removeCallbacks(connectTimeout);
            if(pending!=null)reply(null,"unavailable");
            try{if(gatt!=null){gatt.disconnect();gatt.close();}}catch(RuntimeException ignored){}
            try{emit(new JSONObject().put("type","disconnected").put("handle",handle).put("status",status));}catch(JSONException ignored){}
        }
        @Override public void onConnectionStateChange(BluetoothGatt g,int status,int state){main.post(()->{
            if(closed)return;
            if(state==BluetoothProfile.STATE_CONNECTED && status==BluetoothGatt.GATT_SUCCESS){
                try{if(!g.discoverServices())fail("failed");}catch(SecurityException denied){fail("denied");}
            }else if(state==BluetoothProfile.STATE_DISCONNECTED){
                if(!ready && pending!=null)reply(null,"unavailable");
                close(status);
            }
        });}
        @Override public void onServicesDiscovered(BluetoothGatt g,int status){main.post(()->{
            if(closed || !"connect".equals(pendingKind))return;
            if(status!=BluetoothGatt.GATT_SUCCESS){fail("failed");return;}
            main.removeCallbacks(connectTimeout);ready=true;
            try{
                JSONArray services=new JSONArray();
                for(BluetoothGattService s:g.getServices()){
                    JSONArray chars=new JSONArray();
                    for(BluetoothGattCharacteristic ch:s.getCharacteristics())chars.put(new JSONObject().put("uuid",ch.getUuid().toString()).put("properties",ch.getProperties()));
                    services.put(new JSONObject().put("uuid",s.getUuid().toString()).put("characteristics",chars));
                }
                reply(new JSONObject().put("handle",handle).put("services",services),null);
            }catch(JSONException failed){fail("failed");}
        });}
        // API 33+ value-carrying callbacks; older devices use the deprecated forms below.
        @Override public void onCharacteristicRead(BluetoothGatt g,BluetoothGattCharacteristic ch,byte[] value,int status){done("read",status,value);}
        @Override @SuppressWarnings("deprecation") public void onCharacteristicRead(BluetoothGatt g,BluetoothGattCharacteristic ch,int status){if(Build.VERSION.SDK_INT<33)done("read",status,ch.getValue());}
        @Override public void onCharacteristicWrite(BluetoothGatt g,BluetoothGattCharacteristic ch,int status){done("write",status,null);}
        @Override public void onDescriptorWrite(BluetoothGatt g,BluetoothGattDescriptor d,int status){done("subscribe",status,null);}
        @Override public void onCharacteristicChanged(BluetoothGatt g,BluetoothGattCharacteristic ch,byte[] value){notify(ch,value);}
        @Override @SuppressWarnings("deprecation") public void onCharacteristicChanged(BluetoothGatt g,BluetoothGattCharacteristic ch){if(Build.VERSION.SDK_INT<33)notify(ch,ch.getValue());}
        private void done(String kind,int status,byte[] value){
            byte[] copy=value==null?null:value.clone();
            main.post(()->{
                if(closed || !kind.equals(pendingKind))return;
                if(status!=BluetoothGatt.GATT_SUCCESS){reply(null,status==BluetoothGatt.GATT_READ_NOT_PERMITTED || status==BluetoothGatt.GATT_WRITE_NOT_PERMITTED || status==BluetoothGatt.GATT_INSUFFICIENT_AUTHENTICATION?"denied":"failed");return;}
                reply(copy==null?null:Base64.encodeToString(copy,Base64.NO_WRAP),null);
            });
        }
        private void notify(BluetoothGattCharacteristic ch,byte[] value){
            if(value==null)return;byte[] copy=value.clone();String service=ch.getService().getUuid().toString(),uuid=ch.getUuid().toString();
            main.post(()->{if(closed)return;try{emit(new JSONObject().put("type","notify").put("handle",handle).put("service",service).put("characteristic",uuid).put("value",Base64.encodeToString(copy,Base64.NO_WRAP)));}catch(JSONException ignored){}});
        }
    }

    @Override public void paused(){if(scan!=null)scan.end(null,"cancelledForeground");}
    @Override public void detach(){paused();for(Connection c:new ArrayList<>(connections.values()))c.close(0);main.removeCallbacksAndMessages(null);}
}
