package com.theaimartco.taim.imetest;

import android.inputmethodservice.InputMethodService;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.Build;
import android.util.Base64;
import android.view.inputmethod.InputConnection;
import java.nio.charset.StandardCharsets;

/** Test-device-only driver of the actual Android InputConnection protocol.
 * Never include this service in a production app or enable it on personal input. */
public final class TestIme extends InputMethodService {
    private InputConnection saved;
    private String decode(Intent intent, String key) {
        String value = intent.getStringExtra(key);
        return value == null ? "" : new String(Base64.decode(value, Base64.DEFAULT), StandardCharsets.UTF_8);
    }
    private final BroadcastReceiver commands = new BroadcastReceiver() {
        @Override public void onReceive(Context context, Intent intent) {
            InputConnection connection = getCurrentInputConnection();
            boolean ok = false;
            String operation = intent.getStringExtra("operation");
            if (connection != null && operation != null) {
                String text = decode(intent, "text");
                int before = intent.getIntExtra("before", 0);
                int after = intent.getIntExtra("after", 0);
                switch (operation) {
                    case "commit":
                        ok = Build.VERSION.SDK_INT >= 33 ? connection.commitText(text, 1, new android.view.inputmethod.TextAttribute.Builder().build()) : connection.commitText(text, 1);
                        break;
                    case "compose":
                        ok = Build.VERSION.SDK_INT >= 33 ? connection.setComposingText(text, 1, new android.view.inputmethod.TextAttribute.Builder().build()) : connection.setComposingText(text, 1);
                        break;
                    case "finish": ok = connection.finishComposingText(); break;
                    case "delete": ok = connection.deleteSurroundingText(before, after); break;
                    case "delete-codepoints": ok = connection.deleteSurroundingTextInCodePoints(before, after); break;
                    case "select": ok = connection.setSelection(before, after); break;
                    case "select-all": ok = connection.performContextMenuAction(android.R.id.selectAll); break;
                    case "batch":
                        ok = connection.beginBatchEdit();
                        ok &= connection.commitText("a", 1);
                        ok &= connection.setComposingText(text, 1);
                        connection.endBatchEdit();
                        break;
                    case "assert-all":
                        CharSequence prefix = connection.getTextBeforeCursor(262144, 0);
                        CharSequence selection = connection.getSelectedText(0);
                        CharSequence suffix = connection.getTextAfterCursor(262144, 0);
                        ok = prefix != null && suffix != null &&
                            (prefix.toString() + (selection == null ? "" : selection.toString()) + suffix.toString()).equals(text);
                        break;
                    case "assert":
                        CharSequence left = connection.getTextBeforeCursor(262144, 0);
                        CharSequence right = connection.getTextAfterCursor(262144, 0);
                        ok = left != null && right != null && left.toString().equals(text)
                            && right.toString().equals(decode(intent, "right"));
                        break;
                    case "save": saved = connection; ok = true; break;
                    case "stale":
                        // RemoteInputConnection reports whether an IPC was queued,
                        // not the editor's return value. Assert state after refocus.
                        ok = saved != null && saved != connection;
                        if (ok) saved.commitText(text, 1);
                        break;
                    case "action": ok = connection.performEditorAction(android.view.inputmethod.EditorInfo.IME_ACTION_DONE); break;
                }
            }
            setResultCode(ok ? -1 : 0);
            setResultData(ok ? "ok" : "failed");
        }
    };
    @Override public void onCreate() {
        super.onCreate();
        IntentFilter filter = new IntentFilter("com.theaimartco.taim.imetest.COMMAND");
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(commands, filter, Context.RECEIVER_EXPORTED);
        else registerReceiver(commands, filter);
    }
    @Override public void onDestroy() { unregisterReceiver(commands); saved = null; super.onDestroy(); }
}
