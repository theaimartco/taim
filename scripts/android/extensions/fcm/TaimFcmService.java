package com.theaimartco.taim;

import com.google.firebase.messaging.FirebaseMessagingService;
import com.google.firebase.messaging.RemoteMessage;
import org.json.*;
import java.util.Map;

/** Non-exported FCM receiver for {@link TaimFcm}. Message content is untrusted input. */
public final class TaimFcmService extends FirebaseMessagingService {
    @Override public void onMessageReceived(RemoteMessage message){
        try{
            JSONObject data=new JSONObject();
            for(Map.Entry<String,String> entry:message.getData().entrySet())data.put(entry.getKey(),entry.getValue());
            RemoteMessage.Notification n=message.getNotification();
            JSONObject notification=n==null?null:new JSONObject().put("title",n.getTitle()==null?JSONObject.NULL:n.getTitle()).put("body",n.getBody()==null?JSONObject.NULL:n.getBody());
            JSONObject value=new JSONObject().put("messageId",message.getMessageId()==null?JSONObject.NULL:message.getMessageId())
                .put("from",message.getFrom()==null?JSONObject.NULL:message.getFrom()).put("sentTimeMs",message.getSentTime())
                .put("data",data).put("notification",notification==null?JSONObject.NULL:notification);
            // Plugin events are bounded to 64 KiB; FCM payloads are at most 4 KiB.
            TaimFcm.deliver(getApplicationContext(),new JSONObject().put("type","message").put("message",value));
        }catch(JSONException ignored){}
    }
    @Override public void onNewToken(String token){
        try{TaimFcm.deliver(getApplicationContext(),new JSONObject().put("type","token").put("token",token));}catch(JSONException ignored){}
    }
}
