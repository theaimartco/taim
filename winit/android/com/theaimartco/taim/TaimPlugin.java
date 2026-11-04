package com.theaimartco.taim;

import org.json.JSONObject;

/** Explicit host-registered method channel. Calls arrive on the activity UI thread.
 * Async results are lifecycle checked, bounded and one-shot by the host. */
public interface TaimPlugin {
    void call(String method, JSONObject arguments, Result result);
    default void detach() {}
    /** Activity resumed/paused. Foreground-only resources (NFC reader mode, scans) follow these. */
    default void resumed() {}
    default void paused() {}
    interface Result {
        void success(Object jsonValue);
        void error();
        /** Typed failure: denied, cancelled, cancelledForeground, unsupported, unavailable, disabled,
         * busy, invalid, timeout, notFound. Other codes are reported as a generic failure. */
        default void error(String code) { error(); }
    }
}
