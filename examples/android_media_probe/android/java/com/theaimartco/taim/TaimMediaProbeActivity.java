package com.theaimartco.taim;
/** Test seam is confined to this example; production host never rewrites camera/picker intents. */
public final class TaimMediaProbeActivity extends TaimActivity {
    private String mediaMode="normal";
    public void setMediaMode(String mode){mediaMode=mode;}
    private boolean fixture(android.content.Intent intent){
        if(!getIntent().getBooleanExtra("taim.media.fixture",false) || mediaMode.equals("system"))return false;
        String action=intent.getAction();return "android.provider.action.PICK_IMAGES".equals(action)
            || android.content.Intent.ACTION_OPEN_DOCUMENT.equals(action) || android.provider.MediaStore.ACTION_IMAGE_CAPTURE.equals(action)
            || android.provider.MediaStore.ACTION_VIDEO_CAPTURE.equals(action) || android.content.Intent.ACTION_VIEW.equals(action);
    }
    private void rewrite(android.content.Intent intent){if(fixture(intent))intent.setComponent(new android.content.ComponentName("com.theaimartco.taim.mediadriver","com.theaimartco.taim.mediadriver.MediaResponder")).putExtra("taim.fixture.mode",mediaMode);}
    @Override public void startActivityForResult(android.content.Intent intent,int request){rewrite(intent);super.startActivityForResult(intent,request);}
    @Override public void startActivity(android.content.Intent intent){rewrite(intent);super.startActivity(intent);}
}
