package com.theaimartco.taim;
import android.graphics.Rect;
import android.view.View;
import org.json.JSONObject;

/** Explicit UI-thread factory. Instances must not contain SurfaceView children. */
public interface TaimNativeViewFactory {
    Instance create(TaimActivity activity,JSONObject properties,Host host)throws Exception;
    interface Host {
        long id();
        long generation();
        /** Send a bounded object; disposed/paused/hidden instances cannot emit user events. */
        void emit(JSONObject event);
        /** Local physical-pixel focus/caret rectangle; useful for IME panning. */
        void requestFocusRect(Rect rect);
    }
    interface Instance {
        /** Unparented Android view. It owns its native accessibility and input connections. */
        View view();
        /** Idempotent application-property update. Host events during updates are suppressed. */
        void update(JSONObject properties)throws Exception;
        default void resumed(){}
        default void paused(){}
        default void appearance(boolean dark,boolean reducedMotion){}
        /** Release listeners, players, cameras and producer surfaces. Called once. */
        void dispose();
    }
}
