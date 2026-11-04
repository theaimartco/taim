package com.theaimartco.taim;

import android.graphics.Bitmap;
import android.net.http.SslError;
import android.view.View;
import android.webkit.*;
import android.widget.FrameLayout;
import org.json.*;
import java.io.ByteArrayInputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Iterator;

/** Retained, origin-restricted WebView. No Javascript bridge, external intents or implicit grants. */
final class TaimWebView implements TaimNativeViewFactory.Instance {
    private final FrameLayout frame;
    private final TaimNativeViewFactory.Host host;
    private WebView web;
    private volatile Policy policy;
    private boolean closed;
    private static final class Policy {
        final String url,html;final boolean javascript;
        Policy(JSONObject value)throws Exception {
            for(Iterator<String> keys=value.keys();keys.hasNext();) {
                String key=keys.next();if(!key.equals("url") && !key.equals("html") && !key.equals("javascript"))throw new IllegalArgumentException();
            }
            if(value.has("url")==value.has("html"))throw new IllegalArgumentException();
            if(value.has("javascript") && !(value.get("javascript") instanceof Boolean))throw new IllegalArgumentException();
            javascript=value.optBoolean("javascript",false);
            if(value.has("url")) {
                if(!(value.get("url") instanceof String))throw new IllegalArgumentException();
                url=value.getString("url");html=null;
                if(url.length()>4096 || !https(url))throw new IllegalArgumentException();
            }else {
                if(!(value.get("html") instanceof String))throw new IllegalArgumentException();
                html=value.getString("html");url=null;
                if(html.getBytes(StandardCharsets.UTF_8).length>8192)throw new IllegalArgumentException();
            }
        }
        static boolean https(String value) {
            try {URI uri=new URI(value);return "https".equalsIgnoreCase(uri.getScheme()) && uri.getHost()!=null && !uri.getHost().isEmpty() && uri.getRawUserInfo()==null && (uri.getPort()==-1 || (uri.getPort()>0 && uri.getPort()<=65535));}
            catch(Exception invalid){return false;}
        }
        boolean allows(String value) {
            if(url==null)return "about:blank".equals(value);
            if(!https(value))return false;
            try {URI source=new URI(url),target=new URI(value);return source.getHost().equalsIgnoreCase(target.getHost()) && port(source)==port(target);}
            catch(Exception invalid){return false;}
        }
        private static int port(URI uri){return uri.getPort()==-1?443:uri.getPort();}
        boolean same(Policy other){return other!=null && java.util.Objects.equals(url,other.url) && java.util.Objects.equals(html,other.html) && javascript==other.javascript;}
    }
    TaimWebView(TaimActivity activity,JSONObject properties,TaimNativeViewFactory.Host host)throws Exception {
        // Validate before allocating Chromium resources.
        Policy initial=new Policy(properties);this.host=host;frame=new FrameLayout(activity);
        web=new WebView(activity);
        try {
            WebSettings settings=web.getSettings();settings.setAllowFileAccess(false);settings.setAllowContentAccess(false);
            settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);settings.setSafeBrowsingEnabled(true);
            settings.setMediaPlaybackRequiresUserGesture(true);settings.setGeolocationEnabled(false);settings.setDomStorageEnabled(false);
            CookieManager.getInstance().setAcceptThirdPartyCookies(web,false);
            web.setWebViewClient(new Client());
            web.setWebChromeClient(new WebChromeClient(){
                @Override public void onPermissionRequest(PermissionRequest request){request.deny();}
                @Override public void onGeolocationPermissionsShowPrompt(String origin,GeolocationPermissions.Callback callback){callback.invoke(origin,false,false);}
            });
            web.setDownloadListener((url,agent,disposition,mime,size)->event("blocked"));
            frame.addView(web,new FrameLayout.LayoutParams(-1,-1));apply(initial);
        }catch(Exception failure){dispose();throw failure;}
    }
    private void event(String type){try{host.emit(new JSONObject().put("type",type));}catch(JSONException ignored){}}
    @Override public View view(){return frame;}
    @Override public void update(JSONObject properties)throws Exception {
        if(closed || web==null)throw new IllegalStateException();Policy next=new Policy(properties);if(!next.same(policy))apply(next);
    }
    private void apply(Policy next) {
        web.stopLoading();policy=next;WebSettings settings=web.getSettings();settings.setJavaScriptEnabled(next.javascript);settings.setBlockNetworkLoads(next.url==null);
        if(next.url!=null)web.loadUrl(next.url);else web.loadDataWithBaseURL("about:blank",next.html,"text/html","UTF-8",null);
    }
    @Override public void resumed(){if(web!=null)web.onResume();}
    @Override public void paused(){if(web!=null){web.onPause();web.stopLoading();}}
    @Override public void dispose(){
        if(closed)return;closed=true;WebView previous=web;web=null;policy=null;
        if(previous!=null){previous.stopLoading();previous.setWebChromeClient(null);previous.setWebViewClient(new WebViewClient());frame.removeView(previous);previous.destroy();}
    }
    private final class Client extends WebViewClient {
        @Override public boolean shouldOverrideUrlLoading(WebView view,WebResourceRequest request){Policy current=policy;boolean blocked=current==null || !current.allows(request.getUrl().toString());if(blocked)event("blocked");return blocked;}
        @Override public WebResourceResponse shouldInterceptRequest(WebView view,WebResourceRequest request){
            Policy current=policy;if(current!=null && current.url!=null && current.allows(request.getUrl().toString()))return null;
            return new WebResourceResponse("text/plain","UTF-8",new ByteArrayInputStream(new byte[0]));
        }
        @Override public void onPageStarted(WebView view,String url,Bitmap icon){event("started");}
        @Override public void onPageFinished(WebView view,String url){event("finished");}
        @Override public void onReceivedError(WebView view,WebResourceRequest request,WebResourceError error){if(request.isForMainFrame())event("loadFailed");}
        @Override public void onReceivedHttpError(WebView view,WebResourceRequest request,WebResourceResponse response){if(request.isForMainFrame())event("httpFailed");}
        @Override public void onReceivedSslError(WebView view,SslErrorHandler handler,SslError error){handler.cancel();event("tlsFailed");}
        @Override public void onReceivedHttpAuthRequest(WebView view,HttpAuthHandler handler,String host,String realm){handler.cancel();}
        @Override public void onReceivedClientCertRequest(WebView view,ClientCertRequest request){request.cancel();}
        @Override public boolean onRenderProcessGone(WebView view,RenderProcessGoneDetail detail){dispose();event("rendererGone");return true;}
    }
}
