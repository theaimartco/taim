package com.theaimartco.taim;

import android.app.NativeActivity;
import android.os.Build;
import android.os.Bundle;
import android.graphics.Color;
import android.graphics.Rect;
import android.content.res.Configuration;
import android.text.Editable;
import android.text.Selection;
import android.text.TextWatcher;
import android.text.InputType;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.view.KeyEvent;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;

/** NativeActivity shell with an invisible native editor as an InputConnection
 * owner. Taim still owns and paints the actual editable view. */
public class TaimActivity extends NativeActivity {
    private long session;
    private long owner;
    private long connectionEpoch;
    private long editRevision;
    private ProxyEditor editor;
    private boolean synchronizing;
    private int mutations;
    private int batchDepth;
    private boolean multiline;
    private boolean editorEnabled;
    private boolean secure;
    private boolean cursorMonitor;
    private int caretHeight = 1;
    private Boolean appearance;
    private TaimAccessibility accessibility;
    private TaimServices services;
    private TaimBack back;
    private volatile String restoration = "{}";
    private boolean resumed;
    private TaimNativeViews nativeViews;
    private boolean imeVisible;

    private final android.database.ContentObserver motionObserver = new android.database.ContentObserver(new android.os.Handler(android.os.Looper.getMainLooper())) {
        @Override public void onChange(boolean selfChange) { sendInsets(getWindow().getDecorView().getRootWindowInsets()); }
    };

    private static native void nativeInsets(long session, int left, int top, int right, int bottom, boolean dark, boolean reduced);
    private static native void nativeEdit(long session, long owner, long revision, String text, int start, int end, int composingStart, int composingEnd, boolean multiline);
    private static native void nativeSecure(long session, long owner, int kind, String text, int count);
    private static native void nativeSubmit(long session, long owner);

    private static native boolean nativeSemanticAction(long session, String payload);
    private static native void nativeServiceResponse(long session, long request, String payload);
    private static native void nativePlatformEvent(long session, String payload);

    boolean semanticAction(String payload) { return session != 0 && nativeSemanticAction(session, payload); }
    static void serviceResponse(long token, long request, String payload) { nativeServiceResponse(token, request, payload); }
    void platformEvent(org.json.JSONObject payload) { if (session != 0) nativePlatformEvent(session, payload.toString()); }
    private void lifecycle(String state) { try { platformEvent(new org.json.JSONObject().put("type", "lifecycle").put("state", state)); } catch (org.json.JSONException ignored) {} }
    public void requestTaimService(long token, long id, String payload) {
        if (session != token || services == null) { serviceResponse(token,id,"{\"ok\":false,\"error\":\"unavailable\"}"); return; }
        services.request(token,id,payload);
    }
    public void setTaimSemantics(long token, String payload) { if (session == token && accessibility != null) accessibility.publish(payload); }
    public void hoverTaim(float x, float y, boolean exit) { if (accessibility != null) accessibility.hover(x,y,exit); }
    public void registerTaimPlugin(String channel,TaimPlugin plugin) { if(services==null)throw new IllegalStateException("Register after super.onCreate"); services.register(channel,plugin); }
    public void registerTaimViewFactory(String name,TaimNativeViewFactory factory){if(nativeViews==null)throw new IllegalStateException("Register after super.onCreate");nativeViews.register(name,factory);}
    /** Like registerTaimViewFactory, but the factory's views may contain SurfaceViews (map/video SDKs).
     * They render above Taim's surface: Taim content painted over the slot is hidden by it. */
    public void registerTaimSurfaceViewFactory(String name,TaimNativeViewFactory factory){if(nativeViews==null)throw new IllegalStateException("Register after super.onCreate");nativeViews.register(name,factory,true);}
    public void dispatchTaimNativeKey(long token,long id,long generation,int key,boolean pressed,boolean repeat,String text){if(session==token && nativeViews!=null)nativeViews.key(id,generation,key,pressed,repeat,text);}
    public void dispatchTaimNativeTouch(long token,long pointer,int phase,float x,float y){if(session==token && nativeViews!=null)nativeViews.touch(pointer,phase,x,y);}
    public void setTaimNativeViews(long token,String payload){if(session==token && nativeViews!=null)nativeViews.publish(payload);}
    void releaseTaimProxyFocus(){if(editor!=null)editor.clearFocus();}
    @Override public boolean dispatchTouchEvent(android.view.MotionEvent event){if(event.getActionMasked()==android.view.MotionEvent.ACTION_DOWN && nativeViews!=null){int[] offset=new int[2];nativeViews.getLocationOnScreen(offset);if(!nativeViews.hit(event.getRawX()-offset[0],event.getRawY()-offset[1]))nativeViews.clearFocus(true);}return super.dispatchTouchEvent(event);}
    public void emitTaimPluginEvent(String channel, org.json.JSONObject event) { if(services==null || !services.registered(channel))return;try{platformEvent(new org.json.JSONObject().put("type","plugin").put("channel",channel).put("event",event));}catch(org.json.JSONException ignored){} }
    public void setTaimAutofillFocus(int id) { if (session != 0 && accessibility != null) accessibility.focusAutofill(id); }
    public String getTaimRestoration() { return restoration; }
    public void setTaimRestoration(long token, String value) { if (session == token && value.length() <= 1024*1024) restoration = value; }
    public void setTaimBackEnabled(boolean enabled) { if (back != null) back.enabled(enabled); }
    boolean isTaimResumed(){return resumed;}
    public void stopTaimLocation(long token,long watch){if(session==token && services!=null)services.stopLocation(watch);}
    public void cancelTaimLocationRequest(long token,long id){if(session==token && services!=null)services.cancelLocationRequest(id);}

    @Override protected void onSaveInstanceState(Bundle state) { state.putString("taim.state", restoration); super.onSaveInstanceState(state); }
    @Override protected void onResume() { super.onResume(); resumed = true; if(services!=null){services.resumePermissions();services.resumePlugins();} if(nativeViews!=null)nativeViews.resume(); lifecycle("resumed"); }
    @Override protected void onPause() { resumed = false; if(nativeViews!=null)nativeViews.pause(); if(services!=null)services.pauseLocation(); lifecycle("paused"); super.onPause(); }
    @Override protected void onNewIntent(android.content.Intent intent) { super.onNewIntent(intent); setIntent(intent); try { platformEvent(TaimServices.intent(intent)); } catch (org.json.JSONException ignored) {} }
    @Override public void onTrimMemory(int level) { super.onTrimMemory(level); try { platformEvent(new org.json.JSONObject().put("type","memory").put("level",level)); } catch (org.json.JSONException ignored) {} }
    @Override public void onLowMemory() { super.onLowMemory(); onTrimMemory(android.content.ComponentCallbacks2.TRIM_MEMORY_COMPLETE); }
    @Override public void onRequestPermissionsResult(int code,String[] names,int[] results) { if (services != null) services.permissionResult(code,names,results); super.onRequestPermissionsResult(code,names,results); }
    @Override protected void onActivityResult(int request,int result,android.content.Intent data) { if (services != null) services.activityResult(request,result,data); super.onActivityResult(request,result,data); }
    // API33+ gestures use TaimBack OnBackInvoked/OnBackAnimation callbacks; this is the legacy fallback.
    @android.annotation.SuppressLint("GestureBackNavigation")
    @Override public void onBackPressed() {
        if (imeVisible && editor != null) { ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(editor.getWindowToken(),0); return; }
        if (session != 0 && back != null) back.event("invoked",1,0); else super.onBackPressed();
    }

    @Override public void onCreate(Bundle saved) {
        if (saved != null) restoration = saved.getString("taim.state", "{}");
        // NativeActivity dlopen does not register the library with this class loader.
        // Load the manifest-selected library so Java can resolve the JNI callbacks.
        try {
            android.content.pm.ActivityInfo info = getPackageManager().getActivityInfo(
                getComponentName(), android.content.pm.PackageManager.GET_META_DATA);
            System.loadLibrary(info.metaData.getString("android.app.lib_name"));
        } catch (android.content.pm.PackageManager.NameNotFoundException error) {
            throw new IllegalStateException("Missing NativeActivity metadata", error);
        }
        super.onCreate(saved);
        // NativeActivity/SurfaceView defaults can leave GLES with RGB565,
        // while Vulkan negotiates RGBA8. A backend change after process death
        // then quantizes every colour differently. Fix both surfaces before
        // they are attached or their first native surface callback is sent.
        getWindow().setFormat(android.graphics.PixelFormat.RGBA_8888);
        // NativeActivity takes the whole Window Surface, which disables HWUI and
        // makes TextureView/native hierarchy composition impossible. Keep its
        // native callbacks/InputQueue, but render Taim into a SurfaceView behind
        // the hardware-accelerated Android hierarchy instead.
        getWindow().takeSurface(null);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED);
        android.view.SurfaceView renderSurface = new android.view.SurfaceView(this);
        renderSurface.getHolder().setFormat(android.graphics.PixelFormat.RGBA_8888);
        renderSurface.getHolder().addCallback(this);
        addContentView(renderSurface,new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,ViewGroup.LayoutParams.MATCH_PARENT));
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING);
        View decor = getWindow().getDecorView();
        if (Build.VERSION.SDK_INT >= 30) getWindow().setDecorFitsSystemWindows(false);
        else decor.setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        getWindow().setStatusBarColor(Color.TRANSPARENT);
        getWindow().setNavigationBarColor(Color.TRANSPARENT);
        services = new TaimServices(this);
        back = TaimBack.create(this);
        accessibility = new TaimAccessibility(this);
        addContentView(accessibility, new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        nativeViews=new TaimNativeViews(this);
        addContentView(nativeViews,new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,ViewGroup.LayoutParams.MATCH_PARENT));
        editor = new ProxyEditor();
        editor.setAlpha(0f);
        editor.setCursorVisible(false);
        editor.setLongClickable(false);
        editor.setPadding(0, 0, 0, 0);
        editor.setBackgroundColor(Color.TRANSPARENT);
        editor.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        editor.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS);
        editor.setFilters(new android.text.InputFilter[] {
            (source, start, end, destination, from, to) -> {
                if (multiline) return null;
                String value = source.subSequence(start, end).toString();
                return value.indexOf('\n') >= 0 || value.indexOf('\r') >= 0 ? value.replace("\n", "").replace("\r", "") : null;
            },
            new android.text.InputFilter.LengthFilter(262144)
        });
        addContentView(editor, new ViewGroup.LayoutParams(1, 1));
        editor.addTextChangedListener(new TextWatcher() {
            public void beforeTextChanged(CharSequence s, int start, int count, int after) {}
            public void onTextChanged(CharSequence s, int start, int before, int count) {}
            public void afterTextChanged(Editable text) { snapshot(); }
        });
        editor.setOnEditorActionListener((view, action, event) -> {
            if (multiline && action == EditorInfo.IME_NULL) return false;
            snapshot();
            if (session != 0 && editorEnabled) nativeSubmit(session, owner);
            return true;
        });
        decor.setOnApplyWindowInsetsListener((view, insets) -> { sendInsets(insets); return insets; });
        decor.getViewTreeObserver().addOnGlobalLayoutListener(() -> sendInsets(decor.getRootWindowInsets()));
        getContentResolver().registerContentObserver(android.provider.Settings.Global.getUriFor(android.provider.Settings.Global.ANIMATOR_DURATION_SCALE), false, motionObserver);
        decor.requestApplyInsets();
        attachExtensions();
    }

    /** Instantiate Gradle-provided extensions named in application meta-data. A missing or
     * failing extension is a packaging error, so it fails loudly instead of silently degrading. */
    private void attachExtensions() {
        String names;
        try {
            android.os.Bundle meta=getPackageManager().getApplicationInfo(getPackageName(),android.content.pm.PackageManager.GET_META_DATA).metaData;
            names=meta==null?null:meta.getString("com.theaimartco.taim.extensions");
        } catch (android.content.pm.PackageManager.NameNotFoundException error) { throw new IllegalStateException(error); }
        if (names==null || names.trim().isEmpty()) return;
        for (String name : names.split(",")) {
            String className=name.trim();
            if (!className.matches("[A-Za-z_][A-Za-z0-9_.$]{0,255}")) throw new IllegalStateException("Invalid Taim extension name");
            try { ((TaimExtension)Class.forName(className).getDeclaredConstructor().newInstance()).attach(this); }
            catch (Exception error) { throw new IllegalStateException("Taim extension failed: "+className, error); }
        }
    }

    @Override protected void onDestroy() {
        lifecycle("destroyed");
        if(nativeViews!=null)nativeViews.close();
        if (back != null) back.close();
        if (services != null) services.close();
        session = 0;
        getContentResolver().unregisterContentObserver(motionObserver);
        super.onDestroy();
    }

    @Override public void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        getWindow().getDecorView().requestApplyInsets();
        sendInsets(getWindow().getDecorView().getRootWindowInsets());
    }

    public void attachTaim(long token) {
        boolean first = session != token;
        session = token;
        if (first) { try { platformEvent(TaimServices.intent(getIntent())); } catch (org.json.JSONException ignored) {} lifecycle(resumed ? "resumed" : "paused"); }
        getWindow().getDecorView().requestApplyInsets();
        sendInsets(getWindow().getDecorView().getRootWindowInsets());
    }

    private void sendInsets(WindowInsets insets) {
        if (session == 0 || insets == null) return;
        int left, top, right, bottom;
        if (Build.VERSION.SDK_INT >= 30) {
            imeVisible = insets.isVisible(WindowInsets.Type.ime());
            android.graphics.Insets safe = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
            left = safe.left; top = safe.top; right = safe.right; bottom = safe.bottom;
        } else {
            left = insets.getSystemWindowInsetLeft(); top = insets.getSystemWindowInsetTop();
            right = insets.getSystemWindowInsetRight(); bottom = insets.getSystemWindowInsetBottom();
            if (Build.VERSION.SDK_INT >= 28 && insets.getDisplayCutout() != null) {
                left = Math.max(left, insets.getDisplayCutout().getSafeInsetLeft());
                top = Math.max(top, insets.getDisplayCutout().getSafeInsetTop());
                right = Math.max(right, insets.getDisplayCutout().getSafeInsetRight());
                bottom = Math.max(bottom, insets.getDisplayCutout().getSafeInsetBottom());
            }
            Rect visible = new Rect();
            View decor = getWindow().getDecorView();
            decor.getWindowVisibleDisplayFrame(visible);
            int keyboard = decor.getHeight() - visible.bottom;
            imeVisible = keyboard > bottom + 100;
            bottom = Math.max(bottom, keyboard);
        }
        boolean dark = (getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES;
        if(nativeViews!=null)nativeViews.appearance(appearance!=null?appearance:dark,!android.animation.ValueAnimator.areAnimatorsEnabled());
        nativeInsets(session, left, top, right, bottom, dark, !android.animation.ValueAnimator.areAnimatorsEnabled());
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused && appearance != null) setTaimAppearance(appearance);
    }

    public void setTaimAppearance(boolean dark) {
        appearance = dark;
        if(nativeViews!=null)nativeViews.appearance(dark,!android.animation.ValueAnimator.areAnimatorsEnabled());
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = getWindow().getInsetsController();
            if (controller != null) {
                int flags = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                controller.setSystemBarsAppearance(dark ? 0 : flags, flags);
            }
        } else {
            View decor = getWindow().getDecorView();
            int flags = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
            decor.setSystemUiVisibility((decor.getSystemUiVisibility() & ~flags) | (dark ? 0 : flags));
        }
    }

    public void showTaimEditor(long token, long inputOwner) {
        if (session != token || owner != inputOwner || !editorEnabled || editor == null) return;
        editor.requestFocus();
        ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT);
    }

    public void setTaimEditor(long token, long nextOwner, long revision, boolean enabled, String text, int start, int end, boolean isMultiline, boolean isSecure, int x, int y, int width, int height) {
        if (session != token || editor == null || nextOwner < owner) return;
        // UI-thread edits can overtake a queued Rust snapshot. Only overwrite
        // this editor once Rust has consumed its latest InputConnection edit.
        if (nextOwner == owner && revision != editRevision) return;
        if(enabled && nativeViews!=null && nativeViews.claimsInput() && nextOwner==owner)return;
        InputMethodManager manager = (InputMethodManager)getSystemService(INPUT_METHOD_SERVICE);
        boolean restart = owner != nextOwner || multiline != isMultiline || editorEnabled != enabled || secure != isSecure;
        synchronizing = true;
        try {
            if (owner != nextOwner) editRevision = 0;
            owner = nextOwner;
            editorEnabled = enabled;
            multiline = isMultiline;
            secure = isSecure;
            if (restart) { batchDepth = 0; cursorMonitor = false; }
            if (!enabled) {
                if(nativeViews==null || !nativeViews.ownsFocus())manager.hideSoftInputFromWindow(editor.getWindowToken(), 0);
                editor.clearFocus();
                editor.setText("");
                return;
            }
            if(nativeViews!=null)nativeViews.clearFocus(false);
            int input = InputType.TYPE_CLASS_TEXT | (secure ? InputType.TYPE_TEXT_VARIATION_PASSWORD : (multiline ? InputType.TYPE_TEXT_FLAG_MULTI_LINE : 0));
            if (restart) {
                editor.setInputType(input);
                editor.setImeOptions((multiline ? EditorInfo.IME_ACTION_NONE : EditorInfo.IME_ACTION_DONE) | EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING | EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_FULLSCREEN);
            }
            if (!editor.getText().toString().equals(text)) editor.setText(text);
            int length = editor.length();
            editor.setSelection(Math.max(0, Math.min(start, length)), Math.max(0, Math.min(end, length)));
            caretHeight = Math.max(1, height);
            editor.setTranslationX(x);
            editor.setTranslationY(y);
            editor.requestFocus();
            if (restart) { manager.restartInput(editor); manager.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT); }
            manager.updateSelection(editor, editor.getSelectionStart(), editor.getSelectionEnd(), BaseInputConnection.getComposingSpanStart(editor.getText()), BaseInputConnection.getComposingSpanEnd(editor.getText()));
            if (cursorMonitor && !secure) publishCursorAnchor();
        } finally { synchronizing = false; }
    }

    private void snapshot() {
        if (secure || synchronizing || mutations != 0 || batchDepth != 0 || session == 0 || !editorEnabled || editor == null) return;
        Editable text = editor.getText();
        nativeEdit(session, owner, ++editRevision, text.toString(), Selection.getSelectionStart(text), Selection.getSelectionEnd(text), BaseInputConnection.getComposingSpanStart(text), BaseInputConnection.getComposingSpanEnd(text), multiline);
    }

    private void publishCursorAnchor() {
        if (!editorEnabled || secure || editor == null) return;
        int[] location = new int[2];
        editor.getLocationOnScreen(location);
        android.graphics.Matrix matrix = new android.graphics.Matrix();
        matrix.setTranslate(location[0], location[1]);
        android.view.inputmethod.CursorAnchorInfo.Builder builder = new android.view.inputmethod.CursorAnchorInfo.Builder()
            .setMatrix(matrix)
            .setSelectionRange(editor.getSelectionStart(), editor.getSelectionEnd())
            .setInsertionMarkerLocation(0, 0, caretHeight * 0.8f, caretHeight, android.view.inputmethod.CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION);
        int start = BaseInputConnection.getComposingSpanStart(editor.getText());
        int end = BaseInputConnection.getComposingSpanEnd(editor.getText());
        if (start >= 0 && end > start) builder.setComposingText(start, editor.getText().subSequence(start, end));
        ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).updateCursorAnchorInfo(editor, builder.build());
    }

    /** Rasterize only a missing already-shaped color glyph, never app text. */
    public int[] rasterizeTaimGlyph(String file, int index, int glyph, float size, float x, float y) {
        if (Build.VERSION.SDK_INT < 31 || !Float.isFinite(size) || size <= 0 || size > 512) return null;
        return ColorGlyphRasterizer.draw(file, index, glyph, size, x, y);
    }

    @SuppressWarnings("UseRequiresApi") // framework-only host: no AndroidX @RequiresApi

    @android.annotation.TargetApi(31)
    private static final class ColorGlyphRasterizer {
        private static final java.util.LinkedHashMap<String, android.graphics.fonts.Font> fonts = new java.util.LinkedHashMap<>(8, 0.75f, true);
        static synchronized int[] draw(String file, int index, int glyph, float size, float x, float y) {
            try {
                String key = file + "#" + index;
                android.graphics.fonts.Font font = fonts.get(key);
                if (font == null) {
                    font = new android.graphics.fonts.Font.Builder(new java.io.File(file)).setTtcIndex(index).build();
                    if (fonts.size() == 8) fonts.remove(fonts.keySet().iterator().next());
                    fonts.put(key, font);
                }
                android.graphics.Paint paint = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);
                paint.setTextSize(size);
                paint.setColor(Color.WHITE);
                android.graphics.RectF bounds = new android.graphics.RectF();
                font.getGlyphBounds(glyph, paint, bounds);
                int left = (int)Math.floor(bounds.left + x) - 1;
                int top = (int)Math.floor(bounds.top + y) - 1;
                int width = (int)Math.ceil(bounds.right + x) - left + 1;
                int height = (int)Math.ceil(bounds.bottom + y) - top + 1;
                if (width <= 2 || height <= 2 || width > 1024 || height > 1024) return null;
                android.graphics.Bitmap bitmap = android.graphics.Bitmap.createBitmap(width, height, android.graphics.Bitmap.Config.ARGB_8888);
                try {
                    android.graphics.Canvas canvas = new android.graphics.Canvas(bitmap);
                    canvas.drawGlyphs(new int[] {glyph}, 0, new float[] {x - left, y - top}, 0, 1, font, paint);
                    int[] result = new int[4 + width * height];
                    result[0] = left; result[1] = -top; result[2] = width; result[3] = height;
                    bitmap.getPixels(result, 4, width, 0, 0, width, height);
                    return result;
                } finally { bitmap.recycle(); }
            } catch (java.io.IOException | IllegalArgumentException error) { return null; }
        }
    }

    /** A password connection never receives the application secret, selection,
     * or its length. Only newly typed composition exists in this connection. */
    private class SecureConnection extends BaseInputConnection {
        private final long connectionOwner;
        private final long epoch = connectionEpoch;
        private String composing = "";
        private boolean closed;
        SecureConnection(long value) { super(editor, false); connectionOwner = value; }
        private boolean valid() { return !closed && editorEnabled && secure && owner == connectionOwner && epoch == connectionEpoch; }
        @Override public CharSequence getTextBeforeCursor(int length, int flags) { return ""; }
        @Override public CharSequence getTextAfterCursor(int length, int flags) { return ""; }
        @Override public CharSequence getSelectedText(int flags) { return null; }
        @Override public int getCursorCapsMode(int modes) { return 0; }
        @Override public android.view.inputmethod.ExtractedText getExtractedText(android.view.inputmethod.ExtractedTextRequest request, int flags) { return null; }
        @Override public Editable getEditable() { return null; }
        @Override public boolean setSelection(int start, int end) { return false; }
        @Override public boolean setComposingRegion(int start, int end) { return false; }
        @Override public boolean setComposingText(CharSequence text, int position) {
            if (!valid() || text == null || text.length() > 262144) return false;
            composing = text.toString();
            nativeSecure(session, connectionOwner, 1, composing, 1);
            return true;
        }
        @Override public boolean commitText(CharSequence text, int position) {
            if (!valid() || text == null || text.length() > 262144) return false;
            composing = "";
            nativeSecure(session, connectionOwner, 0, text.toString(), 1);
            return true;
        }
        @Override public boolean finishComposingText() {
            if (!valid()) return false;
            String value = composing;
            composing = "";
            if (!value.isEmpty()) nativeSecure(session, connectionOwner, 0, value, 1);
            return true;
        }
        @Override public boolean deleteSurroundingText(int before, int after) {
            if (!valid() || before < 0 || after < 0 || before > 256 || after > 256) return false;
            if (!composing.isEmpty()) finishComposingText();
            if (before > 0) nativeSecure(session, connectionOwner, 4, "", before);
            if (after > 0) nativeSecure(session, connectionOwner, 5, "", after);
            return true;
        }
        @Override public boolean deleteSurroundingTextInCodePoints(int before, int after) {
            if (!valid() || before < 0 || after < 0 || before > 256 || after > 256) return false;
            if (!composing.isEmpty()) finishComposingText();
            if (before > 0) nativeSecure(session, connectionOwner, 6, "", before);
            if (after > 0) nativeSecure(session, connectionOwner, 7, "", after);
            return true;
        }
        @Override public boolean sendKeyEvent(KeyEvent event) {
            if (!valid()) return false;
            if (event.getAction() != KeyEvent.ACTION_DOWN) return true;
            if (event.getKeyCode() == KeyEvent.KEYCODE_DEL) { if (!composing.isEmpty()) finishComposingText(); nativeSecure(session, connectionOwner, 2, "", 1); return true; }
            if (event.getKeyCode() == KeyEvent.KEYCODE_FORWARD_DEL) { if (!composing.isEmpty()) finishComposingText(); nativeSecure(session, connectionOwner, 3, "", 1); return true; }
            if (event.getKeyCode() == KeyEvent.KEYCODE_ENTER) return performEditorAction(EditorInfo.IME_ACTION_DONE);
            int codepoint = event.getUnicodeChar();
            return codepoint != 0 && commitText(new String(Character.toChars(codepoint)), 1);
        }
        @Override public boolean performEditorAction(int action) {
            if (!valid()) return false;
            finishComposingText();
            nativeSubmit(session, connectionOwner);
            return true;
        }
        @Override public boolean performContextMenuAction(int id) { if(!valid() || id!=android.R.id.selectAll)return false;nativeSecure(session,connectionOwner,8,"",1);return true; }
        @Override public void closeConnection() {if(closed)return;composing="";try{if(valid())nativeSecure(session,connectionOwner,1,"",1);}finally{closed=true;super.closeConnection();}}
    }


    // Invisible 1x1 IME proxy: it never receives user clicks; Taim's text fields expose their own actions.
    @android.annotation.SuppressLint("ClickableViewAccessibility")
    private class ProxyEditor extends EditText {
        ProxyEditor() { super(TaimActivity.this); }
        @Override public boolean onTouchEvent(android.view.MotionEvent event) { return false; }
        @Override public boolean requestRectangleOnScreen(Rect rectangle, boolean immediate) { return false; }
        @Override protected void onSelectionChanged(int start, int end) { super.onSelectionChanged(start, end); snapshot(); }
        @Override public InputConnection onCreateInputConnection(EditorInfo info) {
            connectionEpoch++;
            if (secure) {
                info.inputType = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD;
                info.imeOptions = EditorInfo.IME_ACTION_DONE | EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING | EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_FULLSCREEN;
                info.initialSelStart = info.initialSelEnd = -1;
                if (Build.VERSION.SDK_INT >= 30) info.setInitialSurroundingText("");
                return Build.VERSION.SDK_INT >= 33 ? new ModernSecureConnection(owner) : new SecureConnection(owner);
            }
            InputConnection connection = super.onCreateInputConnection(info);
            if (connection == null) return null;
            if (Build.VERSION.SDK_INT >= 34) return new Modern34ProxyConnection(connection, owner);
            if (Build.VERSION.SDK_INT >= 33) return new ModernProxyConnection(connection, owner);
            if (Build.VERSION.SDK_INT >= 31) return new SurroundingProxyConnection(connection, owner);
            return new ProxyConnection(connection, owner);
        }
    }
    private class ModernSecureConnection extends SecureConnection {
        ModernSecureConnection(long value) { super(value); }
        @Override public boolean commitText(CharSequence text, int position, android.view.inputmethod.TextAttribute attributes) { return commitText(text, position); }
        @Override public boolean setComposingText(CharSequence text, int position, android.view.inputmethod.TextAttribute attributes) { return setComposingText(text, position); }
        @Override public boolean setComposingRegion(int start, int end, android.view.inputmethod.TextAttribute attributes) { return false; }
        @Override public boolean replaceText(int start, int end, CharSequence text, int position, android.view.inputmethod.TextAttribute attributes) { return false; }
    }

    private class ProxyConnection extends InputConnectionWrapper {
        protected final long connectionOwner;
        private final long epoch = connectionEpoch;
        ProxyConnection(InputConnection connection, long value) { super(connection, false); connectionOwner = value; }
        protected boolean valid() { return editorEnabled && owner == connectionOwner && epoch == connectionEpoch; }
        protected boolean update(java.util.function.BooleanSupplier action) {
            if (!valid()) return false;
            mutations++;
            try { return action.getAsBoolean(); }
            finally { mutations--; snapshot(); }
        }
        @Override public boolean requestCursorUpdates(int mode) {
            if (!valid()) return false;
            if ((mode & ~(InputConnection.CURSOR_UPDATE_IMMEDIATE | InputConnection.CURSOR_UPDATE_MONITOR)) != 0) return false;
            cursorMonitor = (mode & InputConnection.CURSOR_UPDATE_MONITOR) != 0;
            if (mode != 0) publishCursorAnchor();
            return true;
        }
        @Override public CharSequence getTextBeforeCursor(int length, int flags) { return valid() ? super.getTextBeforeCursor(length, flags) : ""; }
        @Override public CharSequence getTextAfterCursor(int length, int flags) { return valid() ? super.getTextAfterCursor(length, flags) : ""; }
        @Override public CharSequence getSelectedText(int flags) { return valid() ? super.getSelectedText(flags) : null; }
        @Override public android.view.inputmethod.ExtractedText getExtractedText(android.view.inputmethod.ExtractedTextRequest request, int flags) { return valid() ? super.getExtractedText(request, flags) : null; }
        @Override public boolean performEditorAction(int action) { return update(() -> super.performEditorAction(action)); }
        @Override public void closeConnection() {
            if (!valid()) return;
            mutations++;
            try { super.closeConnection(); }
            finally { batchDepth = 0; mutations--; snapshot(); }
        }
        @Override public boolean beginBatchEdit() { if (!valid()) return false; boolean ok = super.beginBatchEdit(); if (ok) batchDepth++; return ok; }
        @Override public boolean endBatchEdit() { if (!valid()) return false; boolean ok = super.endBatchEdit(); batchDepth = Math.max(0, batchDepth - 1); snapshot(); return ok; }
        @Override public boolean commitCompletion(android.view.inputmethod.CompletionInfo completion) { return update(() -> super.commitCompletion(completion)); }
        @Override public boolean commitCorrection(android.view.inputmethod.CorrectionInfo correction) { return update(() -> super.commitCorrection(correction)); }
        @Override public boolean commitText(CharSequence text, int position) { return update(() -> super.commitText(text, position)); }
        @Override public boolean setComposingText(CharSequence text, int position) { return update(() -> super.setComposingText(text, position)); }
        @Override public boolean setComposingRegion(int start, int end) { return update(() -> super.setComposingRegion(start, end)); }
        @Override public boolean finishComposingText() { return update(() -> super.finishComposingText()); }
        @Override public boolean deleteSurroundingText(int before, int after) { return update(() -> super.deleteSurroundingText(before, after)); }
        @Override public boolean deleteSurroundingTextInCodePoints(int before, int after) { return update(() -> super.deleteSurroundingTextInCodePoints(before, after)); }
        @Override public boolean setSelection(int start, int end) { return update(() -> super.setSelection(start, end)); }
        @Override public boolean sendKeyEvent(KeyEvent event) { return update(() -> super.sendKeyEvent(event)); }
        @Override public boolean performContextMenuAction(int id) { return update(() -> super.performContextMenuAction(id)); }
    }

    private class SurroundingProxyConnection extends ProxyConnection {
        SurroundingProxyConnection(InputConnection connection, long value) { super(connection, value); }
        @Override public android.view.inputmethod.SurroundingText getSurroundingText(int before, int after, int flags) { return valid() ? super.getSurroundingText(before, after, flags) : null; }
    }

    private class ModernProxyConnection extends SurroundingProxyConnection {
        ModernProxyConnection(InputConnection connection, long value) { super(connection, value); }
        @Override public boolean requestCursorUpdates(int mode, int filter) {
            if ((filter & ~InputConnection.CURSOR_UPDATE_FILTER_INSERTION_MARKER) != 0) return false;
            return requestCursorUpdates(mode);
        }
        @Override public boolean commitText(CharSequence text, int position, android.view.inputmethod.TextAttribute attributes) { return update(() -> super.commitText(text, position, attributes)); }
        @Override public boolean setComposingText(CharSequence text, int position, android.view.inputmethod.TextAttribute attributes) { return update(() -> super.setComposingText(text, position, attributes)); }
        @Override public boolean setComposingRegion(int start, int end, android.view.inputmethod.TextAttribute attributes) { return update(() -> super.setComposingRegion(start, end, attributes)); }
    }

    private class Modern34ProxyConnection extends ModernProxyConnection {
        Modern34ProxyConnection(InputConnection connection, long value) { super(connection, value); }
        @Override public boolean replaceText(int start, int end, CharSequence text, int position, android.view.inputmethod.TextAttribute attributes) { return update(() -> super.replaceText(start, end, text, position, attributes)); }
    }

}
