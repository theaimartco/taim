//! Android WebView slots with explicit scripting and bounded content.
use crate::{AndroidView, NativeViewError, android_view};
use serde_json::json;

/// Initial content for the built-in `taim.web` native factory.
#[derive(Debug, Clone)]
pub enum AndroidWebContent {
    /// HTTPS document, at most 4096 bytes. The native adapter validates its URI
    /// and restricts user navigation to the initial origin. Requires INTERNET.
    Https(String),
    /// Inline document, at most 8192 UTF-8 bytes. Network loads are blocked.
    Html(String),
}

/// Create a retained Android WebView slot. Scripting is disabled by default.
///
/// There is no JavaScript bridge, file access, implicit permission request,
/// external-intent dispatch or download handler. Native WebView owns text input
/// and accessibility. This function does not make HTML content trusted.
pub fn android_web_view(content: AndroidWebContent) -> Result<AndroidView, NativeViewError> {
    android_web_view_with_script(content, false)
}

/// Create a WebView with explicit JavaScript policy. Enable scripting only for
/// content the application has chosen to execute. No native bridge is exposed.
pub fn android_web_view_with_script(
    content: AndroidWebContent,
    javascript: bool,
) -> Result<AndroidView, NativeViewError> {
    let properties = match content {
        AndroidWebContent::Https(url) => {
            if url.len() > 4096 || !url.to_ascii_lowercase().starts_with("https://") {
                return Err(NativeViewError::Properties);
            }
            json!({"url":url,"javascript":javascript})
        }
        AndroidWebContent::Html(html) => {
            if html.len() > 8192 {
                return Err(NativeViewError::Properties);
            }
            json!({"html":html,"javascript":javascript})
        }
    };
    android_view("taim.web", properties)
}
