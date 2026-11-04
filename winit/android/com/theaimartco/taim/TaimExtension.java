package com.theaimartco.taim;

/**
 * Optional Gradle-provided integration (FCM, Maps, Billing...). Listed by class name in the
 * {@code com.theaimartco.taim.extensions} application meta-data and attached after the activity's
 * services exist, so implementations can register plugins and native-view factories.
 */
public interface TaimExtension {
    void attach(TaimActivity activity) throws Exception;
}
