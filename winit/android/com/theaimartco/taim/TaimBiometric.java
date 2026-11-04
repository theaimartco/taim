package com.theaimartco.taim;

import android.app.KeyguardManager;
import android.content.pm.PackageManager;
import android.hardware.biometrics.BiometricManager;
import android.hardware.biometrics.BiometricPrompt;
import android.os.Build;
import android.os.CancellationSignal;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Platform BiometricPrompt (API 29+), without AndroidX. A successful result proves
 * only that the local user passed the system prompt; it is not a server credential.
 * Callers needing cryptographic proof must bind a Keystore key to user authentication.
 */
// Authenticators.* are compile-time int flags; they reach API-30-only methods only on API 30+.
@android.annotation.SuppressLint("InlinedApi")
final class TaimBiometric {
    interface Reply {void complete(Object value,String error);}
    private TaimBiometric(){}

    private static int authenticators(JSONObject request)throws JSONException {
        String strength=request.getString("strength");
        int biometric;
        if(strength.equals("strong"))biometric=BiometricManager.Authenticators.BIOMETRIC_STRONG;
        else if(strength.equals("weak"))biometric=BiometricManager.Authenticators.BIOMETRIC_WEAK;
        else throw new IllegalArgumentException();
        return request.getBoolean("deviceCredential")?biometric|BiometricManager.Authenticators.DEVICE_CREDENTIAL:biometric;
    }

    static JSONObject status(TaimActivity host,JSONObject request)throws JSONException {
        int flags=authenticators(request);
        String state;
        if(Build.VERSION.SDK_INT<29)state="unsupportedApi";
        else {
            BiometricManager manager=host.getSystemService(BiometricManager.class);
            if(manager==null)state="noHardware";
            else {
                int code;
                if(Build.VERSION.SDK_INT>=30)code=manager.canAuthenticate(flags);
                else {
                    // API 29 cannot query a class or credential combination; report the
                    // generic biometric state and credential availability conservatively.
                    code=manager.canAuthenticate();
                    if(code!=BiometricManager.BIOMETRIC_SUCCESS && (flags&BiometricManager.Authenticators.DEVICE_CREDENTIAL)!=0 && secure(host))code=BiometricManager.BIOMETRIC_SUCCESS;
                }
                state=state(code);
            }
        }
        PackageManager packages=host.getPackageManager();
        return new JSONObject().put("status",state)
            .put("deviceSecure",secure(host))
            .put("fingerprint",packages.hasSystemFeature(PackageManager.FEATURE_FINGERPRINT))
            .put("face",Build.VERSION.SDK_INT>=29 && packages.hasSystemFeature(PackageManager.FEATURE_FACE))
            .put("iris",Build.VERSION.SDK_INT>=29 && packages.hasSystemFeature(PackageManager.FEATURE_IRIS));
    }

    private static boolean secure(TaimActivity host){KeyguardManager keyguard=host.getSystemService(KeyguardManager.class);return keyguard!=null && keyguard.isDeviceSecure();}

    private static String state(int code){
        switch(code){
            case BiometricManager.BIOMETRIC_SUCCESS:return "available";
            case BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED:return "notEnrolled";
            case BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE:return "noHardware";
            case BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE:return "hardwareUnavailable";
            case 15:return "securityUpdateRequired"; // BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED, API 31
            default:return "unsupported";
        }
    }

    private static String text(JSONObject request,String key,boolean required,int max)throws JSONException {
        if(!request.has(key) || request.isNull(key)){if(required)throw new IllegalArgumentException();return null;}
        String value=request.getString(key);
        if(value.isEmpty() || value.length()>max)throw new IllegalArgumentException();
        return value;
    }

    interface Success {Object complete(BiometricPrompt.AuthenticationResult result)throws Exception;}

    /** Shows the system prompt. The caller owns `cancel`; cancelling dismisses the prompt. */
    static void authenticate(TaimActivity host,JSONObject request,CancellationSignal cancel,Reply reply)throws Exception {
        show(host,request,cancel,null,result->new JSONObject().put("authenticator",kind(result)),reply);
    }

    private static String kind(BiometricPrompt.AuthenticationResult result){
        if(Build.VERSION.SDK_INT<30)return "unknown";
        int type=result.getAuthenticationType();
        return type==BiometricPrompt.AUTHENTICATION_RESULT_TYPE_DEVICE_CREDENTIAL?"deviceCredential":type==BiometricPrompt.AUTHENTICATION_RESULT_TYPE_BIOMETRIC?"biometric":"unknown";
    }

    private static void show(TaimActivity host,JSONObject request,CancellationSignal cancel,BiometricPrompt.CryptoObject crypto,Success success,Reply reply)throws Exception {
        if(Build.VERSION.SDK_INT<29)throw new UnsupportedOperationException();
        if(!host.isTaimResumed())throw new TaimLocation.ForegroundException();
        int flags=authenticators(request);
        boolean credential=(flags&BiometricManager.Authenticators.DEVICE_CREDENTIAL)!=0;
        String title=text(request,"title",true,128),subtitle=text(request,"subtitle",false,256),
            description=text(request,"description",false,512),negative=text(request,"negative",!credential,64);
        if(credential && negative!=null)throw new IllegalArgumentException(); // Android forbids both.
        BiometricPrompt.Builder builder=new BiometricPrompt.Builder(host).setTitle(title);
        if(subtitle!=null)builder.setSubtitle(subtitle);
        if(description!=null)builder.setDescription(description);
        builder.setConfirmationRequired(request.optBoolean("confirmationRequired",true));
        if(Build.VERSION.SDK_INT>=30)builder.setAllowedAuthenticators(flags);
        else if(credential)builder.setDeviceCredentialAllowed(true);
        // The framework reports the negative button through its listener, not as an error.
        if(!credential)builder.setNegativeButton(negative,host.getMainExecutor(),(dialog,which)->reply.complete(null,"cancelled"));
        BiometricPrompt prompt=builder.build();
        BiometricPrompt.AuthenticationCallback callback=new BiometricPrompt.AuthenticationCallback(){
            @Override public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult result){
                Object value;
                try{value=success.complete(result);}catch(Exception failed){reply.complete(null,"failed");return;}
                reply.complete(value,null);
            }
            // A rejected finger/face is not terminal; the system prompt lets the user retry.
            @Override public void onAuthenticationFailed(){}
            @Override public void onAuthenticationError(int code,CharSequence ignoredMessage){reply.complete(null,error(code));}
        };
        if(crypto==null)prompt.authenticate(cancel,host.getMainExecutor(),callback);
        else prompt.authenticate(crypto,cancel,host.getMainExecutor(),callback);
    }

    // ---- Keystore keys bound to biometric authentication ----
    // Per-use keys: every signature/encryption/decryption needs a fresh successful prompt, and the
    // key is unusable without one even to code running in this process.

    private static final String KEYSTORE="AndroidKeyStore",PREFIX="taim.biometric.";
    private static String alias(JSONObject request)throws JSONException{
        String alias=request.getString("alias");if(!alias.matches("[A-Za-z0-9._-]{1,64}"))throw new IllegalArgumentException();return PREFIX+alias;
    }
    private static java.security.KeyStore store()throws Exception{java.security.KeyStore ks=java.security.KeyStore.getInstance(KEYSTORE);ks.load(null);return ks;}

    /** Create (replace) a key. kind "sign": EC P-256/SHA-256, returns the X.509 public key; kind "encrypt": AES-256-GCM. */
    static Object createKey(JSONObject request)throws Exception {
        if(Build.VERSION.SDK_INT<30)throw new UnsupportedOperationException(); // per-use auth parameters need API 30
        String alias=alias(request),kind=request.getString("kind");
        boolean credential=request.optBoolean("deviceCredential",false);
        int auth=android.security.keystore.KeyProperties.AUTH_BIOMETRIC_STRONG|(credential?android.security.keystore.KeyProperties.AUTH_DEVICE_CREDENTIAL:0);
        boolean invalidate=request.optBoolean("invalidateOnEnrollment",true);
        if(kind.equals("sign")){
            android.security.keystore.KeyGenParameterSpec spec=new android.security.keystore.KeyGenParameterSpec.Builder(alias,android.security.keystore.KeyProperties.PURPOSE_SIGN)
                .setAlgorithmParameterSpec(new java.security.spec.ECGenParameterSpec("secp256r1")).setDigests(android.security.keystore.KeyProperties.DIGEST_SHA256)
                .setUserAuthenticationRequired(true).setUserAuthenticationParameters(0,auth).setInvalidatedByBiometricEnrollment(invalidate).build();
            java.security.KeyPairGenerator generator=java.security.KeyPairGenerator.getInstance(android.security.keystore.KeyProperties.KEY_ALGORITHM_EC,KEYSTORE);
            generator.initialize(spec);
            return android.util.Base64.encodeToString(generator.generateKeyPair().getPublic().getEncoded(),android.util.Base64.NO_WRAP);
        }
        if(kind.equals("encrypt")){
            android.security.keystore.KeyGenParameterSpec spec=new android.security.keystore.KeyGenParameterSpec.Builder(alias,android.security.keystore.KeyProperties.PURPOSE_ENCRYPT|android.security.keystore.KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(256).setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE)
                .setUserAuthenticationRequired(true).setUserAuthenticationParameters(0,auth).setInvalidatedByBiometricEnrollment(invalidate).build();
            javax.crypto.KeyGenerator generator=javax.crypto.KeyGenerator.getInstance(android.security.keystore.KeyProperties.KEY_ALGORITHM_AES,KEYSTORE);
            generator.init(spec);generator.generateKey();return null;
        }
        throw new IllegalArgumentException();
    }

    static Object deleteKey(JSONObject request)throws Exception{store().deleteEntry(alias(request));return null;}

    /** Public key of an existing signing key, or null. Needs no authentication. */
    static Object publicKey(JSONObject request)throws Exception{
        java.security.cert.Certificate certificate=store().getCertificate(alias(request));
        return certificate==null?JSONObject.NULL:android.util.Base64.encodeToString(certificate.getPublicKey().getEncoded(),android.util.Base64.NO_WRAP);
    }

    private static byte[] bytes(JSONObject request,String key,int max)throws JSONException{
        byte[] value=android.util.Base64.decode(request.getString(key),android.util.Base64.NO_WRAP);
        if(value.length==0 || value.length>max)throw new IllegalArgumentException();return value;
    }

    /** Thrown for a missing key; KeyPermanentlyInvalidatedException signals a new enrollment. */
    static final class MissingKeyException extends Exception{}

    static void cryptoOperation(TaimActivity host,String op,JSONObject request,CancellationSignal cancel,Reply reply)throws Exception {
        if(!request.getString("strength").equals("strong"))throw new IllegalArgumentException(); // weak biometrics cannot unlock keys
        java.security.KeyStore ks=store();String alias=alias(request);
        if(!ks.containsAlias(alias))throw new MissingKeyException();
        switch(op){
            case "biometricSign":{
                byte[] challenge=bytes(request,"data",4096);
                java.security.Signature signature=java.security.Signature.getInstance("SHA256withECDSA");
                signature.initSign((java.security.PrivateKey)ks.getKey(alias,null));
                show(host,request,cancel,new BiometricPrompt.CryptoObject(signature),result->{
                    java.security.Signature unlocked=result.getCryptoObject().getSignature();unlocked.update(challenge);
                    return new JSONObject().put("signature",android.util.Base64.encodeToString(unlocked.sign(),android.util.Base64.NO_WRAP)).put("authenticator",kind(result));
                },reply);return;
            }
            case "biometricEncrypt":{
                byte[] plaintext=bytes(request,"data",65536);
                javax.crypto.Cipher cipher=javax.crypto.Cipher.getInstance("AES/GCM/NoPadding");
                cipher.init(javax.crypto.Cipher.ENCRYPT_MODE,(javax.crypto.SecretKey)ks.getKey(alias,null));
                show(host,request,cancel,new BiometricPrompt.CryptoObject(cipher),result->{
                    javax.crypto.Cipher unlocked=result.getCryptoObject().getCipher();byte[] sealed=unlocked.doFinal(plaintext);
                    return new JSONObject().put("iv",android.util.Base64.encodeToString(unlocked.getIV(),android.util.Base64.NO_WRAP))
                        .put("ciphertext",android.util.Base64.encodeToString(sealed,android.util.Base64.NO_WRAP)).put("authenticator",kind(result));
                },reply);return;
            }
            case "biometricDecrypt":{
                byte[] iv=bytes(request,"iv",32),sealed=bytes(request,"ciphertext",65536+16);
                javax.crypto.Cipher cipher=javax.crypto.Cipher.getInstance("AES/GCM/NoPadding");
                cipher.init(javax.crypto.Cipher.DECRYPT_MODE,(javax.crypto.SecretKey)ks.getKey(alias,null),new javax.crypto.spec.GCMParameterSpec(128,iv));
                show(host,request,cancel,new BiometricPrompt.CryptoObject(cipher),result->new JSONObject()
                    .put("data",android.util.Base64.encodeToString(result.getCryptoObject().getCipher().doFinal(sealed),android.util.Base64.NO_WRAP)).put("authenticator",kind(result)),reply);
                return;
            }
            default:throw new IllegalArgumentException();
        }
    }

    private static String error(int code){
        switch(code){
            case BiometricPrompt.BIOMETRIC_ERROR_USER_CANCELED:
            case 13: // AndroidX BIOMETRIC_ERROR_NEGATIVE_BUTTON, defensive
                return "cancelled";
            case BiometricPrompt.BIOMETRIC_ERROR_CANCELED:return "cancelledForeground";
            case BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT:return "lockout";
            case BiometricPrompt.BIOMETRIC_ERROR_LOCKOUT_PERMANENT:return "lockoutPermanent";
            case BiometricPrompt.BIOMETRIC_ERROR_TIMEOUT:return "timeout";
            case BiometricPrompt.BIOMETRIC_ERROR_NO_BIOMETRICS:return "notEnrolled";
            case BiometricPrompt.BIOMETRIC_ERROR_HW_NOT_PRESENT:
            case BiometricPrompt.BIOMETRIC_ERROR_HW_UNAVAILABLE:
            case BiometricPrompt.BIOMETRIC_ERROR_NO_DEVICE_CREDENTIAL:
            case 15: // BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED
                return "unsupported";
            default:return "failed";
        }
    }
}
