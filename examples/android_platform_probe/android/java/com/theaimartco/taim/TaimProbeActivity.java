package com.theaimartco.taim;
public final class TaimProbeActivity extends TaimActivity {
    @Override public void onCreate(android.os.Bundle saved){
        super.onCreate(saved);
        registerTaimPlugin("probe.echo",new TaimPlugin(){public void call(String method,org.json.JSONObject value,TaimPlugin.Result result){if(!method.equals("echo")){result.error();return;}emitTaimPluginEvent("probe.echo",value);result.success(value);}});
    }
}
