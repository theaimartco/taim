package com.theaimartco.taim;

import android.content.Context;
import android.graphics.Rect;
import android.os.Bundle;
import android.util.SparseArray;
import android.view.*;
import android.view.accessibility.*;
import android.view.autofill.*;
import org.json.*;
import java.util.*;

/** A real Android virtual hierarchy; renderer/widget identity remains in Rust. */
// Programmatic, activity-owned host: no XML constructors. Touch routes to Rust; clicks are exposed
// as actions on the virtual accessibility nodes this view provides, not via performClick.
@android.annotation.SuppressLint({"ViewConstructor","ClickableViewAccessibility"})
final class TaimAccessibility extends View {
    private final TaimActivity host;
    private final AccessibilityManager manager;
    private final LinkedHashMap<Integer,JSONObject> nodes=new LinkedHashMap<>();
    private final AccessibilityNodeProvider provider=new Provider();
    private Rect clip=new Rect();
    private long revision;
    private int accessibilityFocus=-1,hovered=-1,autofillFocus=-1,keyboardFocus=-1;
    TaimAccessibility(TaimActivity host){super(host);this.host=host;manager=(AccessibilityManager)host.getSystemService(Context.ACCESSIBILITY_SERVICE);setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_YES);setImportantForAutofill(IMPORTANT_FOR_AUTOFILL_YES);setWillNotDraw(true);setFocusable(false);}
    @Override public AccessibilityNodeProvider getAccessibilityNodeProvider(){return provider;}
    @Override public boolean onTouchEvent(MotionEvent event){return false;}
    @Override public boolean dispatchHoverEvent(MotionEvent event){
        if(manager.isEnabled() && manager.isTouchExplorationEnabled()){
            hover(event.getX(),event.getY(),event.getActionMasked()==MotionEvent.ACTION_HOVER_EXIT);return true;
        }return super.dispatchHoverEvent(event);
    }
    void publish(String encoded){
        try{
            JSONObject envelope=new JSONObject(encoded);long nextRevision=envelope.getLong("revision");if(nextRevision<=revision)return;
            if(encoded.length()>8*1024*1024)return;JSONObject tree=envelope.getJSONObject("tree");Rect nextClip=rect(tree.getJSONArray("clip"));JSONArray array=tree.getJSONArray("nodes");if(array.length()>16384)return;
            LinkedHashMap<Integer,JSONObject> next=new LinkedHashMap<>();for(int i=0;i<array.length();i++){JSONObject node=array.getJSONObject(i);int id=node.getInt("id");if(id<=0 || next.containsKey(id))return;rect(node.getJSONArray("bounds"));next.put(id,node);}
            HashSet<Integer> checked=new HashSet<>();for(Integer id:next.keySet()){HashSet<Integer> path=new HashSet<>();Integer current=id;while(current!=null && !checked.contains(current)){if(!path.add(current))return;JSONObject node=next.get(current);if(node==null)return;current=node.isNull("parent")?null:node.getInt("parent");}checked.addAll(path);}
            LinkedHashMap<Integer,JSONObject> previous=new LinkedHashMap<>(nodes);
            nodes.clear();nodes.putAll(next);clip=nextClip;revision=nextRevision;
            if(accessibilityFocus!=-1 && !exposed(nodes.get(accessibilityFocus))){event(accessibilityFocus,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);accessibilityFocus=-1;}
            if(hovered!=-1 && !exposed(nodes.get(hovered))){event(hovered,AccessibilityEvent.TYPE_VIEW_HOVER_EXIT);hovered=-1;}
            if(autofillFocus!=-1 && !exposed(nodes.get(autofillFocus)))focusAutofill(-1);
            event(-1,AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
            int focused=-1;
            for(JSONObject node:nodes.values())if(exposed(node)){
                int id=node.optInt("id");if((node.optInt("state")&4)!=0)focused=id;
                JSONObject old=previous.get(id);if(old==null || node.optString("role").equals("SecureField"))continue;
                if(!Objects.equals(text(old),text(node)))event(id,AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED);
                if(!Objects.equals(Objects.toString(old.opt("selection"),""),Objects.toString(node.opt("selection"),"")))event(id,AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED);
            }
            if(focused!=keyboardFocus && focused!=-1)event(focused,AccessibilityEvent.TYPE_VIEW_FOCUSED);keyboardFocus=focused;
        }catch(JSONException ignored){}
    }
    void hover(float x,float y,boolean exit){
        if(!manager.isEnabled() || !manager.isTouchExplorationEnabled())return;
        int next=-1;
        if(!exit && clip.contains((int)x,(int)y))for(JSONObject node:nodes.values()){
            if(exposed(node) && !node.optString("role").equals("Group") && !node.optString("role").equals("Window") && bounds(node).contains((int)x,(int)y))next=node.optInt("id");
        }
        if(next!=hovered){if(next!=-1)event(next,AccessibilityEvent.TYPE_VIEW_HOVER_ENTER);if(hovered!=-1)event(hovered,AccessibilityEvent.TYPE_VIEW_HOVER_EXIT);hovered=next;}
    }
    private Rect rect(JSONArray value)throws JSONException {if(value.length()!=4)throw new JSONException("Invalid rectangle");float x=(float)value.getDouble(0),y=(float)value.getDouble(1),w=(float)value.getDouble(2),h=(float)value.getDouble(3);if(!Float.isFinite(x)||!Float.isFinite(y)||!Float.isFinite(w)||!Float.isFinite(h)||w<0||h<0)throw new JSONException("Invalid rectangle");return new Rect((int)Math.floor(x),(int)Math.floor(y),(int)Math.ceil(x+w),(int)Math.ceil(y+h));}
    private Rect bounds(JSONObject node){try{return rect(node.getJSONArray("bounds"));}catch(JSONException bad){return new Rect();}}
    private String text(JSONObject node){if(node==null || node.optString("role").equals("SecureField"))return "";JSONObject value=node.optJSONObject("value");return value!=null && value.has("text")?value.optString("text",""):node.optString("name","");}
    private boolean exposed(JSONObject node){if(node==null || (node.optInt("state")&128)!=0 || !Rect.intersects(bounds(node),clip))return false;
        JSONObject current=node;for(int i=0;i<=nodes.size();i++){if((current.optInt("state")&128)!=0)return false;if(current.isNull("parent"))return true;current=nodes.get(current.optInt("parent"));if(current==null)return false;}return false;
    }
    private boolean descendant(JSONObject node,int root){if(root==AccessibilityNodeProvider.HOST_VIEW_ID)return true;JSONObject current=node;for(int i=0;i<=nodes.size() && current!=null;i++){if(current.optInt("id")==root)return true;if(current.isNull("parent"))break;current=nodes.get(current.optInt("parent"));}return false;}
    private void event(int id,int type){
        if(!manager.isEnabled() || !isAttachedToWindow())return;
        AccessibilityEvent event=AccessibilityEvent.obtain(type);event.setPackageName(getContext().getPackageName());event.setClassName("android.view.View");event.setSource(this,id);
        JSONObject node=nodes.get(id);if(node!=null){boolean secret=node.optString("role").equals("SecureField");event.setClassName(className(node.optString("role")));event.setPassword(secret);event.setEnabled((node.optInt("state")&1)==0);if(!secret){String value=text(node);if(!value.isEmpty())event.getText().add(value);JSONArray selection=node.optJSONArray("selection");if(selection!=null){event.setFromIndex(selection.optInt(0));event.setToIndex(selection.optInt(1));event.setItemCount(value.length());}}}
        if(type==AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)event.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE);
        ViewParent parent=getParent();if(parent!=null)parent.requestSendAccessibilityEvent(this,event);
    }
    private static String className(String role){switch(role){
        case "Button":return "android.widget.Button";case "Checkbox":return "android.widget.CheckBox";case "RadioButton":return "android.widget.RadioButton";
        case "Tab":return "android.app.ActionBar$Tab";case "MenuItem":return "android.view.MenuItem";case "ListItem":return "android.view.View";
        case "TextField":case "SecureField":return "android.widget.EditText";case "Slider":return "android.widget.SeekBar";
        case "ScrollView":return "android.widget.ScrollView";case "Image":return "android.widget.ImageView";case "StaticText":case "Link":return "android.widget.TextView";default:return "android.view.View";
    }}
    private boolean send(JSONObject node,String action,Object value){
        try {JSONObject request=new JSONObject();request.put("id",node.getInt("id"));request.put("revision",revision);request.put("action",action);
            if(action.equals("text") || action.equals("autofill"))request.put("text",value);else if(value!=null)request.put("value",value);
            return host.semanticAction(request.toString());
        }catch(JSONException bad){return false;}
    }
    private final class Provider extends AccessibilityNodeProvider {
        @Override public AccessibilityNodeInfo createAccessibilityNodeInfo(int id){
            if(id==HOST_VIEW_ID){AccessibilityNodeInfo info=AccessibilityNodeInfo.obtain(TaimAccessibility.this);onInitializeAccessibilityNodeInfo(info);for(JSONObject node:nodes.values())if(node.isNull("parent") && exposed(node))info.addChild(TaimAccessibility.this,node.optInt("id"));return info;}
            JSONObject node=nodes.get(id);if(!exposed(node))return null;
            AccessibilityNodeInfo info=AccessibilityNodeInfo.obtain();info.setSource(TaimAccessibility.this,id);info.setPackageName(getContext().getPackageName());String role=node.optString("role");info.setClassName(className(role));
            if(node.isNull("parent"))info.setParent(TaimAccessibility.this);else info.setParent(TaimAccessibility.this,node.optInt("parent"));
            for(JSONObject child:nodes.values())if(!child.isNull("parent") && child.optInt("parent")==id && exposed(child))info.addChild(TaimAccessibility.this,child.optInt("id"));
            int state=node.optInt("state"),actions=node.optInt("actions");boolean secret=role.equals("SecureField"),editable=secret||role.equals("TextField");
            info.setEnabled((state&1)==0);info.setSelected((state&2)!=0);info.setFocused((state&4)!=0);info.setFocusable((state&8)!=0 || (actions&2)!=0);info.setChecked((state&16)!=0);info.setCheckable(role.equals("Checkbox")||role.equals("RadioButton"));info.setPassword(secret);info.setEditable(editable);
            String name=node.isNull("name")?"":node.optString("name","");JSONObject value=node.optJSONObject("value");
            if(editable){info.setHintText(name);if(!secret && value!=null)info.setText(value.optString("text",""));}
            else if(role.equals("StaticText") || role.equals("Link"))info.setText(name);else info.setContentDescription(name);
            String description=node.isNull("description")?"":node.optString("description","");if(!description.isEmpty())info.setContentDescription(name.isEmpty()?description:name+", "+description);
            info.setLiveRegion(secret?ACCESSIBILITY_LIVE_REGION_NONE:node.optInt("live",0));
            if(android.os.Build.VERSION.SDK_INT>=28)info.setScreenReaderFocusable(role.equals("NamedGroup"));
            if(android.os.Build.VERSION.SDK_INT>=30 && (state&32)!=0)info.setStateDescription("Mixed");
            info.setScrollable(role.equals("ScrollView") && (actions&12)!=0);
            JSONArray selection=node.optJSONArray("selection");if(!secret && editable && selection!=null)info.setTextSelection(selection.optInt(0),selection.optInt(1));
            Rect area=bounds(node);boolean visible=(state&128)==0 && Rect.intersects(area,clip);Rect parent=new Rect();JSONObject parentNode=nodes.get(node.optInt("parent",-1));if(parentNode!=null)parent=bounds(parentNode);
            Rect relative=new Rect(area);relative.offset(-parent.left,-parent.top);info.setBoundsInParent(relative);
            if(!area.intersect(clip))area.setEmpty();int[] location=new int[2];getLocationOnScreen(location);area.offset(location[0],location[1]);info.setBoundsInScreen(area);info.setVisibleToUser(visible && isShown());
            info.setAccessibilityFocused(accessibilityFocus==id);
            if(!role.equals("Group") && !role.equals("Window"))info.addAction(accessibilityFocus==id?AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS:AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS);
            if(info.isEnabled()){
                if((actions&1)!=0){info.setClickable(true);info.addAction(AccessibilityNodeInfo.ACTION_CLICK);}
                if((actions&2)!=0)info.addAction(AccessibilityNodeInfo.ACTION_FOCUS);
                if((actions&4)!=0)info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
                if((actions&8)!=0)info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);
                if((actions&16)!=0 && value!=null && value.has("current")){float min=(float)value.optDouble("min"),max=(float)value.optDouble("max"),current=(float)value.optDouble("current");if(Float.isFinite(min)&&Float.isFinite(max)&&Float.isFinite(current)&&min<=max){info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT,min,max,current));info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS);}}
                if((actions&32)!=0)info.addAction((state&64)!=0?AccessibilityNodeInfo.AccessibilityAction.ACTION_COLLAPSE:AccessibilityNodeInfo.AccessibilityAction.ACTION_EXPAND);
                if((actions&64)!=0)info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN);
                if((actions&128)!=0)info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_DISMISS);
                if(editable && !secret){info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_TEXT);info.addAction(AccessibilityNodeInfo.ACTION_SET_SELECTION);info.addAction(AccessibilityNodeInfo.ACTION_COPY);info.addAction(AccessibilityNodeInfo.ACTION_CUT);info.addAction(AccessibilityNodeInfo.ACTION_PASTE);info.setMovementGranularities(AccessibilityNodeInfo.MOVEMENT_GRANULARITY_CHARACTER|AccessibilityNodeInfo.MOVEMENT_GRANULARITY_WORD|AccessibilityNodeInfo.MOVEMENT_GRANULARITY_PARAGRAPH);info.addAction(AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY);info.addAction(AccessibilityNodeInfo.ACTION_PREVIOUS_AT_MOVEMENT_GRANULARITY);}
                if((state&4)!=0)info.addAction(AccessibilityNodeInfo.ACTION_CLEAR_FOCUS);
            }
            return info;
        }
        @Override public AccessibilityNodeInfo findFocus(int focus){if(focus==AccessibilityNodeInfo.FOCUS_ACCESSIBILITY)return accessibilityFocus==-1?null:createAccessibilityNodeInfo(accessibilityFocus);for(JSONObject node:nodes.values())if((node.optInt("state")&4)!=0)return createAccessibilityNodeInfo(node.optInt("id"));return null;}
        @Override public java.util.List<AccessibilityNodeInfo> findAccessibilityNodeInfosByText(String text,int root){
            ArrayList<AccessibilityNodeInfo> result=new ArrayList<>();if(text==null)return result;String search=text.toLowerCase(Locale.ROOT);
            for(JSONObject node:nodes.values())if(exposed(node) && descendant(node,root) && (node.optString("name","")+" "+node.optString("description","")+" "+text(node)).toLowerCase(Locale.ROOT).contains(search)){AccessibilityNodeInfo info=createAccessibilityNodeInfo(node.optInt("id"));if(info!=null)result.add(info);}return result;
        }
        @Override public boolean performAction(int id,int action,Bundle args){
            JSONObject node=nodes.get(id);if(!exposed(node))return false;
            if(action==AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS){if(accessibilityFocus==id)return false;if(accessibilityFocus!=-1)event(accessibilityFocus,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);accessibilityFocus=id;event(id,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED);invalidate();return true;}
            if(action==AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS){if(accessibilityFocus!=id)return false;accessibilityFocus=-1;event(id,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);invalidate();return true;}
            if((node.optInt("state")&129)!=0)return false;
            int supported=node.optInt("actions");
            if(action==AccessibilityNodeInfo.ACTION_CLEAR_FOCUS && (node.optInt("state")&4)!=0)return send(node,"blur",null);
            if(node.optString("role").equals("TextField")){
                if(action==AccessibilityNodeInfo.ACTION_COPY)return send(node,"copy",null);
                if(action==AccessibilityNodeInfo.ACTION_CUT)return send(node,"cut",null);
                if(action==AccessibilityNodeInfo.ACTION_PASTE)return send(node,"paste",null);
                if((action==AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY || action==AccessibilityNodeInfo.ACTION_PREVIOUS_AT_MOVEMENT_GRANULARITY) && args!=null){
                    int granularity=args.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_MOVEMENT_GRANULARITY_INT);String content=text(node);java.text.BreakIterator iterator;
                    if(granularity==AccessibilityNodeInfo.MOVEMENT_GRANULARITY_CHARACTER)iterator=java.text.BreakIterator.getCharacterInstance();else if(granularity==AccessibilityNodeInfo.MOVEMENT_GRANULARITY_WORD)iterator=java.text.BreakIterator.getWordInstance();else if(granularity==AccessibilityNodeInfo.MOVEMENT_GRANULARITY_PARAGRAPH)iterator=java.text.BreakIterator.getSentenceInstance();else return false;
                    iterator.setText(content);JSONArray selection=node.optJSONArray("selection");int anchor=selection==null?0:selection.optInt(0),caret=selection==null?0:selection.optInt(1);int next;
                    if(granularity==AccessibilityNodeInfo.MOVEMENT_GRANULARITY_PARAGRAPH){if(action==AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY){int newline=content.indexOf('\n',caret);next=newline<0?content.length():newline+1;if(next==caret)return false;}else{if(caret==0)return false;next=content.lastIndexOf('\n',Math.max(-1,caret-2))+1;}}
                    else next=action==AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY?iterator.following(caret):iterator.preceding(caret);if(next==java.text.BreakIterator.DONE)return false;
                    Bundle moved=new Bundle();moved.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT,args.getBoolean(AccessibilityNodeInfo.ACTION_ARGUMENT_EXTEND_SELECTION_BOOLEAN)?anchor:next);moved.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT,next);return performAction(id,AccessibilityNodeInfo.ACTION_SET_SELECTION,moved);
                }
            }
            if(action==AccessibilityNodeInfo.ACTION_CLICK && (supported&1)!=0)return send(node,"activate",null);
            if(action==AccessibilityNodeInfo.ACTION_FOCUS && (supported&2)!=0)return send(node,"focus",null);
            if(action==AccessibilityNodeInfo.ACTION_SCROLL_FORWARD && (supported&4)!=0)return send(node,"increment",null);
            if(action==AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD && (supported&8)!=0)return send(node,"decrement",null);
            if((supported&32)!=0 && ((action==AccessibilityNodeInfo.ACTION_EXPAND && (node.optInt("state")&64)==0) || (action==AccessibilityNodeInfo.ACTION_COLLAPSE && (node.optInt("state")&64)!=0)))return send(node,"expand",null);
            if(action==AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId() && (supported&64)!=0)return send(node,"scroll",null);
            if(action==AccessibilityNodeInfo.ACTION_DISMISS && (supported&128)!=0)return send(node,"dismiss",null);
            if(action==AccessibilityNodeInfo.ACTION_SET_TEXT && node.optString("role").equals("TextField") && args!=null && args.containsKey(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE))return send(node,"text",args.getCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,""));
            if(action==AccessibilityNodeInfo.ACTION_SET_SELECTION && node.optString("role").equals("TextField") && args!=null){int start=args.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT,-1),end=args.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT,-1);if(start<0 || end<0 || start>text(node).length() || end>text(node).length())return false;try{return host.semanticAction(new JSONObject().put("id",id).put("revision",revision).put("action","selection").put("start",start).put("end",end).toString());}catch(JSONException invalid){return false;}}
            if(action==AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS.getId() && (supported&16)!=0 && args!=null)return send(node,"progress",args.getFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE));
            return false;
        }
    }
    void focusAutofill(int id){
        if(autofillFocus==id)return;AutofillManager autofill=getContext().getSystemService(AutofillManager.class);if(autofill==null)return;
        if(autofillFocus!=-1)autofill.notifyViewExited(this,autofillFocus);autofillFocus=-1;
        JSONObject node=nodes.get(id);if(!exposed(node) || node.isNull("autofill") || (node.optInt("state")&129)!=0)return;
        Rect area=bounds(node);int[] location=new int[2];getLocationOnScreen(location);area.offset(location[0],location[1]);autofillFocus=id;autofill.notifyViewEntered(this,id,area);
    }
    private static String hint(String value){switch(value){case "Name":return AUTOFILL_HINT_NAME;case "Username":return AUTOFILL_HINT_USERNAME;case "Email":return AUTOFILL_HINT_EMAIL_ADDRESS;case "Password":return AUTOFILL_HINT_PASSWORD;case "NewPassword":return "newPassword";case "OneTimeCode":return "smsOTPCode";case "Phone":return AUTOFILL_HINT_PHONE;case "Address":return AUTOFILL_HINT_POSTAL_ADDRESS;default:return null;}}
    @Override public void onProvideAutofillVirtualStructure(ViewStructure structure,int flags){
        super.onProvideAutofillVirtualStructure(structure,flags);ArrayList<JSONObject> fields=new ArrayList<>();for(JSONObject node:nodes.values())if(exposed(node) && !node.isNull("autofill") && (node.optInt("state")&129)==0 && (node.optString("role").equals("TextField") || node.optString("role").equals("SecureField")))fields.add(node);
        structure.setChildCount(fields.size());for(int i=0;i<fields.size();i++){JSONObject node=fields.get(i);int id=node.optInt("id");ViewStructure child=structure.newChild(i);child.setAutofillId(structure.getAutofillId(),id);child.setClassName("android.widget.EditText");child.setAutofillType(AUTOFILL_TYPE_TEXT);String purpose=hint(node.optString("autofill"));if(purpose!=null)child.setAutofillHints(new String[]{purpose});child.setDataIsSensitive(true);Rect area=bounds(node);child.setDimens(area.left,area.top,0,0,area.width(),area.height());
            if(!node.optString("role").equals("SecureField")){JSONObject value=node.optJSONObject("value");if(value!=null)child.setAutofillValue(AutofillValue.forText(value.optString("text","")));}
        }
    }
    @Override public void autofill(SparseArray<AutofillValue> values){for(int i=0;i<values.size();i++){JSONObject node=nodes.get(values.keyAt(i));AutofillValue value=values.valueAt(i);if(exposed(node) && !node.isNull("autofill") && value.isText() && (node.optInt("state")&129)==0)send(node,"autofill",value.getTextValue().toString());}}
}
