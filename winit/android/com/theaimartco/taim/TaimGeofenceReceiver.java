package com.theaimartco.taim;

import android.content.*;
import android.location.LocationManager;

/** Opt-in, non-exported receiver for {@link TaimGeofence} proximity alerts. */
public final class TaimGeofenceReceiver extends BroadcastReceiver {
    @Override public void onReceive(Context context,Intent intent){
        String id=intent.getStringExtra(TaimGeofence.EXTRA_ID);
        if(id==null || !intent.hasExtra(LocationManager.KEY_PROXIMITY_ENTERING))return;
        TaimGeofence.transition(context.getApplicationContext(),id,intent.getBooleanExtra(LocationManager.KEY_PROXIMITY_ENTERING,false));
    }
}
