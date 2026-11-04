package com.theaimartco.taim;
import android.os.Bundle;
import android.view.View;
import android.widget.*;
import org.json.JSONObject;
/** Real controls with observable lifecycle for the embedding acceptance driver. */
public final class TaimNativeProbeActivity extends TaimActivity {
    public int created,disposed,resumes,pauses,clicks,badDisposed;
    @Override public void onCreate(Bundle state){super.onCreate(state);
        registerTaimViewFactory("fixture.surface",(activity,properties,host)->new TaimNativeViewFactory.Instance(){
            final android.view.SurfaceView surface=new android.view.SurfaceView(activity);
            public View view(){return surface;}
            public void update(JSONObject props){}
            public void dispose(){badDisposed++;}
        });
        registerTaimViewFactory("fixture.controls",(activity,properties,host)->{
            created++;LinearLayout group=new LinearLayout(activity);group.setOrientation(LinearLayout.VERTICAL);
            Button button=new Button(activity);button.setAllCaps(false);button.setText("Native button");group.addView(button,new LinearLayout.LayoutParams(-1,130));
            EditText editor=new EditText(activity);editor.setSingleLine(true);editor.setHint("Native editor");group.addView(editor,new LinearLayout.LayoutParams(-1,130));
            button.setOnClickListener(view->{clicks++;try{host.emit(new JSONObject().put("type","click").put("count",clicks));}catch(Exception ignored){}});
            return new TaimNativeViewFactory.Instance(){
                public View view(){return group;}
                public void update(JSONObject props){button.setText(props.optString("label","Native button"));}
                public void resumed(){resumes++;}
                public void paused(){pauses++;}
                public void dispose(){disposed++;button.setOnClickListener(null);}
            };
        });
    }
}
