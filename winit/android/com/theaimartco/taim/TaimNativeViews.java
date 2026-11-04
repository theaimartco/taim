package com.theaimartco.taim;
import android.content.*;
import android.graphics.*;
import android.os.*;
import android.view.*;
import android.view.inputmethod.InputMethodManager;
import android.widget.FrameLayout;
import org.json.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Axis-aligned native hierarchy overlays. TextureView works; independent SurfaceView layers do not. */
// Programmatic routing host; embedded native children implement their own clicks/accessibility.
@android.annotation.SuppressLint({"ViewConstructor","ClickableViewAccessibility"})
final class TaimNativeViews extends FrameLayout {
    final TaimActivity host;private final Map<String,TaimNativeViewFactory> factories=new HashMap<>();
    private final LinkedHashMap<Long,Entry> entries=new LinkedHashMap<>();private final Handler main=new Handler(Looper.getMainLooper());
    private final LinkedHashMap<Long,MotionEvent.PointerCoords> touches=new LinkedHashMap<>();
    private final HashMap<Long,Integer> pointerIds=new HashMap<>();private long downTime;
    private static final class HeldKey {
        final Entry entry;final View target;final long down;int repeats;boolean textOnly;
        HeldKey(Entry entry,View target){this.entry=entry;this.target=target;down=SystemClock.uptimeMillis();}
    }
    private final HashMap<Integer,HeldKey> keys=new HashMap<>();private int lockedMeta;
    private long next=1;private boolean resumed,closed,dark,reduced;private Entry focused;
    private final android.view.ViewTreeObserver.OnGlobalFocusChangeListener focusListener=(oldView,newView)->{
        Entry entry=find(newView);if(entry!=null){Rect rect=new Rect();newView.getDrawingRect(rect);entry.frame.offsetDescendantRectToMyCoords(newView,rect);focus(entry,rect);}else if(focused!=null){Entry previous=focused;cancelKeys();focused=null;focusEvent(previous,null);}
    };
    TaimNativeViews(TaimActivity host){super(host);this.host=host;setClipChildren(true);setClipToPadding(true);setFocusable(false);setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);
        host.getWindow().getDecorView().getViewTreeObserver().addOnGlobalFocusChangeListener(focusListener);
        factories.put("taim.texture",(activity,properties,events)->new TaimTextureView(activity,properties,events));
        factories.put("taim.web",(activity,properties,events)->new TaimWebView(activity,properties,events));factories.put("taim.video",TaimVideoView::new);factories.put("taim.camera",TaimCameraView::new);
    }
    /** Factories allowed to contain SurfaceViews; those are placed on the media-overlay sublayer. */
    private final java.util.Set<String> surfaceFactories=new java.util.HashSet<>();
    void register(String name,TaimNativeViewFactory factory,boolean surfaceViews){
        // Mark first: register() immediately creates slots that were waiting for this factory.
        boolean added=surfaceViews && !factories.containsKey(name) && surfaceFactories.add(name);
        try{register(name,factory);}catch(RuntimeException failed){if(added)surfaceFactories.remove(name);throw failed;}
    }
    void register(String name,TaimNativeViewFactory factory){if(Looper.myLooper()!=Looper.getMainLooper())throw new IllegalStateException("Register native factories on the main thread");if(closed || factory==null || !name.matches("[A-Za-z0-9._-]{1,128}") || factories.size()>=32 || factories.containsKey(name))throw new IllegalArgumentException();factories.put(name,factory);
        for(Entry entry:new ArrayList<>(entries.values()))if(entry.name.equals(name) && entry.instance==null){entries.remove(entry.id);entry.dispose();Entry replacement=new Entry(entry.id,name,entry.descriptor);entries.put(replacement.id,replacement);replacement.create();}
    }
    private static Rect rect(JSONArray array)throws JSONException {
        if(array.length()!=4)throw new IllegalArgumentException();double x=array.getDouble(0),y=array.getDouble(1),w=array.getDouble(2),h=array.getDouble(3);
        if(!Double.isFinite(x) || !Double.isFinite(y) || !Double.isFinite(w) || !Double.isFinite(h) || Math.abs(x)>1000000 || Math.abs(y)>1000000 || w<0 || h<0 || w>8192 || h>8192)throw new IllegalArgumentException();
        return new Rect((int)Math.floor(x),(int)Math.floor(y),(int)Math.ceil(x+w),(int)Math.ceil(y+h));
    }
    void publish(String encoded){if(closed)return;
        try{if(encoded.getBytes(StandardCharsets.UTF_8).length>2*1024*1024)throw new IllegalArgumentException();JSONObject scene=new JSONObject(encoded);JSONArray views=scene.getJSONArray("views");if(views.length()>64)throw new IllegalArgumentException();
            LinkedHashMap<Long,JSONObject> next=new LinkedHashMap<>();long area=0;
            for(int i=0;i<views.length();i++){JSONObject value=views.getJSONObject(i);long id=value.getLong("id");if(id<=0 || next.containsKey(id) || !value.getString("factory").matches("[A-Za-z0-9._-]{1,128}"))throw new IllegalArgumentException();
                Rect bounds=rect(value.getJSONArray("bounds"));area+=(long)bounds.width()*bounds.height();if((long)bounds.width()*bounds.height()>8388608 || area>33554432)throw new IllegalArgumentException();
                if(!value.isNull("clip"))rect(value.getJSONArray("clip"));next.put(id,value);
            }
            for(Long id:new ArrayList<>(entries.keySet()))if(!next.containsKey(id)){Entry entry=entries.remove(id);entry.dispose();}
            for(Map.Entry<Long,JSONObject> item:next.entrySet()){
                Entry entry=entries.get(item.getKey());String name=item.getValue().getString("factory");
                if(entry!=null && (!entry.name.equals(name) || entry.instance==null)){entries.remove(entry.id);entry.dispose();entry=null;}
                if(entry==null){entry=new Entry(item.getKey(),name,item.getValue());entries.put(entry.id,entry);entry.create();}else entry.apply(item.getValue());
                if(entry.frame!=null)entry.frame.bringToFront();
            }
            if(scene.has("error"))send(0,0,new JSONObject().put("type","error").put("code",scene.getString("error")));
        }catch(Exception invalid){clear();try{send(0,0,new JSONObject().put("type","error").put("code","invalidScene"));}catch(JSONException ignored){}}
    }
    private Entry find(View view){if(view==null)return null;for(Entry entry:entries.values())if(entry.instance!=null){View current=view;while(current!=null){if(current==entry.frame)return entry;android.view.ViewParent parent=current.getParent();current=parent instanceof View?(View)parent:null;}}return null;}
    private void send(long id,long generation,JSONObject event){try{host.platformEvent(new JSONObject().put("type","nativeView").put("id",id).put("generation",generation).put("event",event));}catch(JSONException ignored){}}
    private void focusEvent(Entry entry,Rect rect){try{host.platformEvent(new JSONObject().put("type","nativeViewFocus").put("id",entry.id).put("generation",entry.generation).put("rect",rect==null?JSONObject.NULL:new JSONArray(new int[]{rect.left,rect.top,rect.width(),rect.height()})));}catch(JSONException ignored){}}
    private void focus(Entry entry,Rect rect){if(!keys.isEmpty() && (focused!=entry || keys.values().iterator().next().target!=findFocus()))cancelKeys();if(entry.disposed || !entry.visible || !entry.enabled || !resumed)return;focused=entry;host.releaseTaimProxyFocus();focusEvent(entry,rect);}
    boolean ownsFocus(){return focused!=null && focused.frame!=null && focused.frame.hasFocus();}
    boolean claimsInput(){return focused!=null;}
    void clearFocus(boolean hideKeyboard){cancelKeys();Entry entry=focused;focused=null;if(entry!=null){if(entry.frame!=null)entry.frame.clearFocus();focusEvent(entry,null);if(hideKeyboard)((InputMethodManager)host.getSystemService(Context.INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(getWindowToken(),0);}}
    boolean hit(float x,float y){for(Entry entry:entries.values())if(entry.visible && entry.clip!=null && entry.clip.contains((int)x,(int)y))return true;return false;}
    void resume(){if(closed)return;resumed=true;setVisibility(VISIBLE);for(Entry entry:entries.values())entry.lifecycle();}
    void pause(){cancelKeys();if(!touches.isEmpty())touch(touches.keySet().iterator().next(),3,0,0);touches.clear();pointerIds.clear();resumed=false;clearFocus(true);for(Entry entry:entries.values())entry.lifecycle();setVisibility(INVISIBLE);}
    void appearance(boolean dark,boolean reduced){if(this.dark==dark && this.reduced==reduced)return;this.dark=dark;this.reduced=reduced;for(Entry entry:entries.values())if(entry.instance!=null)try{entry.instance.appearance(dark,reduced);}catch(RuntimeException failed){entry.fail("appearanceFailed");}}
    void clear(){for(Entry entry:new ArrayList<>(entries.values()))entry.dispose();entries.clear();}
    void close(){if(closed)return;pause();clear();closed=true;main.removeCallbacksAndMessages(null);host.getWindow().getDecorView().getViewTreeObserver().removeOnGlobalFocusChangeListener(focusListener);factories.clear();}
    private int keyMeta(){int meta=lockedMeta;
        int[][] modifiers={{KeyEvent.KEYCODE_SHIFT_LEFT,KeyEvent.META_SHIFT_LEFT_ON},{KeyEvent.KEYCODE_SHIFT_RIGHT,KeyEvent.META_SHIFT_RIGHT_ON},{KeyEvent.KEYCODE_CTRL_LEFT,KeyEvent.META_CTRL_LEFT_ON},{KeyEvent.KEYCODE_CTRL_RIGHT,KeyEvent.META_CTRL_RIGHT_ON},{KeyEvent.KEYCODE_ALT_LEFT,KeyEvent.META_ALT_LEFT_ON},{KeyEvent.KEYCODE_ALT_RIGHT,KeyEvent.META_ALT_RIGHT_ON},{KeyEvent.KEYCODE_META_LEFT,KeyEvent.META_META_LEFT_ON},{KeyEvent.KEYCODE_META_RIGHT,KeyEvent.META_META_RIGHT_ON}};
        for(int[] modifier:modifiers)if(keys.containsKey(modifier[0]))meta|=modifier[1];return KeyEvent.normalizeMetaState(meta);
    }
    private void cancelKeys(){ArrayList<Map.Entry<Integer,HeldKey>> held=new ArrayList<>(keys.entrySet());keys.clear();lockedMeta=0;
        for(Map.Entry<Integer,HeldKey> item:held){HeldKey key=item.getValue();if(!key.textOnly)key.target.dispatchKeyEvent(new KeyEvent(key.down,SystemClock.uptimeMillis(),KeyEvent.ACTION_UP,item.getKey(),key.repeats,0,KeyCharacterMap.VIRTUAL_KEYBOARD,0,KeyEvent.FLAG_CANCELED,InputDevice.SOURCE_KEYBOARD));}
    }
    void key(long id,long generation,int code,boolean pressed,boolean repeat,String text){
        if(closed || !resumed || code<0 || code>KeyEvent.getMaxKeyCode() || text==null || text.length()>4096)return;
        Entry entry=focused;View target=findFocus();
        if(entry==null || entry.id!=id || entry.generation!=generation)return;
        if(!entry.active || !entry.enabled || target==null || find(target)!=entry){cancelKeys();return;}
        HeldKey held=keys.get(code);
        if(held!=null && (held.entry!=entry || held.target!=target)){cancelKeys();held=null;}
        if(!pressed){if(held==null)return;keys.remove(code);if(!held.textOnly)entry.frame.dispatchKeyEvent(new KeyEvent(held.down,SystemClock.uptimeMillis(),KeyEvent.ACTION_UP,code,held.repeats,keyMeta(),KeyCharacterMap.VIRTUAL_KEYBOARD,0,0,InputDevice.SOURCE_KEYBOARD));return;}
        if(held==null && repeat)return;
        if(held==null){if(keys.size()>=64)return;held=new HeldKey(entry,target);keys.put(code,held);
            if(code==KeyEvent.KEYCODE_CAPS_LOCK)lockedMeta^=KeyEvent.META_CAPS_LOCK_ON;
            if(code==KeyEvent.KEYCODE_NUM_LOCK)lockedMeta^=KeyEvent.META_NUM_LOCK_ON;
            if(code==KeyEvent.KEYCODE_SCROLL_LOCK)lockedMeta^=KeyEvent.META_SCROLL_LOCK_ON;
        }else held.repeats++;
        int meta=keyMeta();KeyEvent event=new KeyEvent(held.down,SystemClock.uptimeMillis(),KeyEvent.ACTION_DOWN,code,held.repeats,meta,KeyCharacterMap.VIRTUAL_KEYBOARD,0,0,InputDevice.SOURCE_KEYBOARD);
        boolean printable=!text.isEmpty() && text.codePointAt(0)>=32 && text.codePointAt(0)!=127;
        boolean shortcut=(meta&(KeyEvent.META_CTRL_ON|KeyEvent.META_ALT_ON|KeyEvent.META_META_ON))!=0;
        boolean translated=printable && !shortcut && (text.codePointCount(0,text.length())!=1 || event.getUnicodeChar()!=text.codePointAt(0));
        held.textOnly=translated;
        if(translated)entry.frame.dispatchKeyEvent(new KeyEvent(SystemClock.uptimeMillis(),text,KeyCharacterMap.VIRTUAL_KEYBOARD,0));else if(!shortcut || KeyEvent.isModifierKey(code) || !entry.frame.dispatchKeyShortcutEvent(event))entry.frame.dispatchKeyEvent(event);
    }
    // NativeActivity owns the OS InputQueue: winit forwards claimed gesture streams here.
    void touch(long id,int phase,float x,float y){if(closed || !resumed)return;if(phase==4){clearFocus(true);return;}
        if(!Float.isFinite(x) || !Float.isFinite(y))return;
        if(phase==0){if(touches.containsKey(id) || touches.size()>=16)return;int pointer=0;while(pointerIds.containsValue(pointer))pointer++;pointerIds.put(id,pointer);touches.put(id,new MotionEvent.PointerCoords());if(touches.size()==1)downTime=SystemClock.uptimeMillis();}
        MotionEvent.PointerCoords coordinate=touches.get(id);if(coordinate==null)return;int[] offset=new int[2];getLocationInWindow(offset);coordinate.x=x-offset[0];coordinate.y=y-offset[1];coordinate.pressure=1;coordinate.size=1;
        int index=new ArrayList<>(touches.keySet()).indexOf(id);int action=phase==0?(touches.size()==1?MotionEvent.ACTION_DOWN:MotionEvent.ACTION_POINTER_DOWN|(index<<MotionEvent.ACTION_POINTER_INDEX_SHIFT)):phase==1?MotionEvent.ACTION_MOVE:phase==2?(touches.size()==1?MotionEvent.ACTION_UP:MotionEvent.ACTION_POINTER_UP|(index<<MotionEvent.ACTION_POINTER_INDEX_SHIFT)):MotionEvent.ACTION_CANCEL;
        MotionEvent.PointerProperties[] properties=new MotionEvent.PointerProperties[touches.size()];MotionEvent.PointerCoords[] coords=new MotionEvent.PointerCoords[touches.size()];int i=0;
        for(Map.Entry<Long,MotionEvent.PointerCoords> item:touches.entrySet()){MotionEvent.PointerProperties property=new MotionEvent.PointerProperties();property.id=pointerIds.get(item.getKey());property.toolType=MotionEvent.TOOL_TYPE_FINGER;properties[i]=property;coords[i]=item.getValue();i++;}
        MotionEvent event=MotionEvent.obtain(downTime,SystemClock.uptimeMillis(),action,touches.size(),properties,coords,0,0,1,1,0,0,android.view.InputDevice.SOURCE_TOUCHSCREEN,0);dispatchTouchEvent(event);event.recycle();
        if(phase==2){touches.remove(id);pointerIds.remove(id);}else if(phase==3){touches.clear();pointerIds.clear();}
    }
    @Override public boolean onTouchEvent(MotionEvent event){return false;}
    private static void validateTree(View view,int depth,boolean surfaces){if((!surfaces && view instanceof SurfaceView) || depth>64)throw new IllegalArgumentException();if(view instanceof android.view.ViewGroup){android.view.ViewGroup group=(android.view.ViewGroup)view;for(int i=0;i<group.getChildCount();i++)validateTree(group.getChildAt(i),depth+1,surfaces);}}
    /**
     * Opted-in SurfaceViews (e.g. Google Maps) compose between Taim's render surface (media
     * sublayer) and the window: setZOrderMediaOverlay must run before the SurfaceView creates its
     * surface, which happens in its own pre-draw pass, so scan in an earlier-registered pre-draw.
     * Taim content painted over such a slot is hidden by it; modal occlusion hides the slot.
     */
    private static void overlaySurfaces(View view,int depth){if(depth>64)return;if(view instanceof SurfaceView)((SurfaceView)view).setZOrderMediaOverlay(true);if(view instanceof android.view.ViewGroup){android.view.ViewGroup group=(android.view.ViewGroup)view;for(int i=0;i<group.getChildCount();i++)overlaySurfaces(group.getChildAt(i),depth+1);}}
    final class Entry implements TaimNativeViewFactory.Host {
        final long id,generation;final String name;JSONObject descriptor;TaimNativeViewFactory.Instance instance;Slot frame;Rect clip;boolean visible,enabled,active,disposed,updating;String properties;android.view.ViewTreeObserver.OnPreDrawListener surfaceScan;
        final WeakHashMap<View,Boolean> savedEnabled=new WeakHashMap<>();
        Entry(long id,String name,JSONObject descriptor){this.id=id;this.name=name;this.descriptor=descriptor;generation=next++;}
        public long id(){return id;}public long generation(){return generation;}
        void create(){if(disposed)return;try{
            JSONObject props=descriptor.getJSONObject("properties");if(props.toString().getBytes(StandardCharsets.UTF_8).length>16384)throw new IllegalArgumentException();
            TaimNativeViewFactory factory=factories.get(name);if(factory==null){fail("unknownFactory");return;}
            updating=true;instance=factory.create(host,new JSONObject(props.toString()),this);if(instance==null || instance.view()==null || instance.view().getParent()!=null)throw new IllegalArgumentException();
            boolean surfaces=surfaceFactories.contains(name);validateTree(instance.view(),0,surfaces);frame=new Slot(this);
            if(surfaces){View root=instance.view();overlaySurfaces(root,0);surfaceScan=()->{overlaySurfaces(root,0);return true;};frame.getViewTreeObserver().addOnPreDrawListener(surfaceScan);}frame.addView(instance.view());addView(frame);instance.appearance(dark,reduced);updating=false;
            send(id,generation,new JSONObject().put("type","created"));apply(descriptor);
        }catch(Exception error){updating=false;fail("creationFailed");}}
        void apply(JSONObject value){descriptor=value;if(disposed || instance==null)return;try{
            JSONObject props=value.getJSONObject("properties");String encoded=props.toString();if(encoded.getBytes(StandardCharsets.UTF_8).length>16384)throw new IllegalArgumentException();
            if(!encoded.equals(properties)){updating=true;instance.update(new JSONObject(encoded));validateTree(instance.view(),0,surfaceFactories.contains(name));updating=false;properties=encoded;}
            Rect bounds=rect(value.getJSONArray("bounds"));clip=value.isNull("clip")?null:rect(value.getJSONArray("clip"));if(clip!=null && !clip.intersect(bounds))clip=null;
            visible=clip!=null && !clip.isEmpty() && bounds.width()>0 && bounds.height()>0;boolean previous=enabled;enabled=value.getBoolean("enabled");
            if(!enabled){disable(instance.view());frame.setDescendantFocusability(FOCUS_BLOCK_DESCENDANTS);frame.setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);}
            else{if(!previous){for(Map.Entry<View,Boolean> saved:new ArrayList<>(savedEnabled.entrySet()))saved.getKey().setEnabled(saved.getValue());savedEnabled.clear();updating=true;instance.update(new JSONObject(encoded));updating=false;}frame.setDescendantFocusability(FOCUS_AFTER_DESCENDANTS);frame.setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);}
            FrameLayout.LayoutParams layout=new FrameLayout.LayoutParams(bounds.width(),bounds.height());layout.leftMargin=bounds.left;layout.topMargin=bounds.top;frame.setLayoutParams(layout);
            frame.setClipBounds(visible?new Rect(clip.left-bounds.left,clip.top-bounds.top,clip.right-bounds.left,clip.bottom-bounds.top):new Rect());
            frame.setVisibility(visible?VISIBLE:GONE);if((!visible || !enabled) && focused==this)clearFocus(true);lifecycle();
        }catch(Exception failed){updating=false;fail("updateFailed");}}
        void disable(View view){if(!savedEnabled.containsKey(view))savedEnabled.put(view,view.isEnabled());view.setEnabled(false);if(view instanceof android.view.ViewGroup){android.view.ViewGroup group=(android.view.ViewGroup)view;for(int i=0;i<group.getChildCount();i++)disable(group.getChildAt(i));}}
        void lifecycle(){boolean desired=resumed && visible && !disposed && instance!=null;if(desired==active)return;active=desired;try{if(active)instance.resumed();else{cancelTouch();instance.paused();}send(id,generation,new JSONObject().put("type",active?"resumed":"paused"));}catch(Exception failed){fail("lifecycleFailed");}}
        void cancelTouch(){if(frame==null)return;long now=SystemClock.uptimeMillis();MotionEvent event=MotionEvent.obtain(now,now,MotionEvent.ACTION_CANCEL,0,0,0);frame.dispatchTouchEvent(event);event.recycle();}
        void fail(String code){try{send(id,generation,new JSONObject().put("type","error").put("code",code));}catch(JSONException ignored){}release();}
        void release(){if(focused==this)clearFocus(true);cancelTouch();if(instance!=null){try{if(active)instance.paused();}catch(RuntimeException ignored){}try{instance.dispose();}catch(RuntimeException ignored){}}active=false;visible=false;if(frame!=null){if(surfaceScan!=null)frame.getViewTreeObserver().removeOnPreDrawListener(surfaceScan);removeView(frame);}surfaceScan=null;frame=null;instance=null;savedEnabled.clear();}
        void dispose(){if(disposed)return;disposed=true;release();try{send(id,generation,new JSONObject().put("type","disposed"));}catch(JSONException ignored){}}
        public void emit(JSONObject value){if(value==null || (Looper.myLooper()==Looper.getMainLooper() && updating))return;String encoded=value.toString();if(encoded.getBytes(StandardCharsets.UTF_8).length>16384)return;main.post(()->{if(closed || disposed || updating || !active || !enabled || entries.get(id)!=this)return;try{send(id,generation,new JSONObject(encoded));}catch(JSONException ignored){}});}
        public void requestFocusRect(Rect rect){if(rect==null)return;Rect copy=new Rect(rect);main.post(()->{if(!disposed && entries.get(id)==this && instance!=null && frame.hasFocus() && copy.intersect(0,0,frame.getWidth(),frame.getHeight()))focus(this,copy);});}
    }
    final class Slot extends FrameLayout {
        final Entry entry;Slot(Entry entry){super(host);this.entry=entry;setClipChildren(true);setClipToPadding(true);setSaveFromParentEnabled(false);}
        @Override public boolean dispatchTouchEvent(MotionEvent event){if(event.getActionMasked()!=MotionEvent.ACTION_CANCEL && (!resumed || !entry.visible || entry.clip==null))return false;
            if(event.getActionMasked()==MotionEvent.ACTION_CANCEL)return super.dispatchTouchEvent(event);
            if(event.getActionMasked()==MotionEvent.ACTION_DOWN){if(!entry.clip.contains((int)(event.getX()+getLeft()),(int)(event.getY()+getTop())))return false;if(entry.enabled)focus(entry,new Rect(0,0,getWidth(),getHeight()));}
            if(!entry.enabled)return true;return super.dispatchTouchEvent(event);
        }
        @Override public boolean onTouchEvent(MotionEvent event){return entry.visible;}
        @Override public boolean requestChildRectangleOnScreen(View child,Rect rectangle,boolean immediate){Rect local=new Rect(rectangle);offsetDescendantRectToMyCoords(child,local);focus(entry,local);return true;}
    }
}
