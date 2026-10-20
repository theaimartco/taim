//! Language Server Protocol transport plus legacy demo presentation state.

use serde_json::{Value, json};
use std::collections::HashSet;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use taim_futures::futures::channel::mpsc;
use taim_futures::futures::{SinkExt, executor::block_on};

use crate::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServerStatus {
    #[default]
    Unavailable,
    Starting,
    Ready,
    Failed,
    Stopped,
}

#[derive(Debug, Clone)]
pub enum ServerEvent {
    Message(Value),
    Stderr(String),
    Exited,
}

/// A real stdio JSON-RPC language server process.
pub struct LanguageServer {
    child: Child,
    writer: Arc<Mutex<ChildStdin>>,
    next_request_id: u64,
    root: PathBuf,
    opened: HashSet<String>,
    versions: std::collections::HashMap<String, i32>,
}

impl LanguageServer {
    pub fn start(root: &Path) -> Result<(Self, taim::Task<Message>), String> {
        let executable =
            std::env::var_os("VIRGO_RUST_ANALYZER").unwrap_or_else(|| "rust-analyzer".into());
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(root)
            .spawn()
            .map_err(|error| error.to_string())?;
        let writer =
            Arc::new(Mutex::new(child.stdin.take().ok_or_else(|| {
                "language server stdin is unavailable".to_string()
            })?));
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "language server stdout is unavailable".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "language server stderr is unavailable".to_string())?;
        let (sender, receiver) = mpsc::channel(64);
        let mut stdout_sender = sender.clone();
        std::thread::Builder::new()
            .name("virgo-lsp-stdout".to_string())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    match read_message(&mut reader) {
                        Ok(Some(value)) => {
                            if block_on(stdout_sender.send(ServerEvent::Message(value))).is_err() {
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            let _ =
                                block_on(stdout_sender.send(ServerEvent::Stderr(format!(
                                    "LSP protocol error: {error}"
                                ))));
                            break;
                        }
                    }
                }
                let _ = block_on(stdout_sender.send(ServerEvent::Exited));
            })
            .map_err(|error| error.to_string())?;
        let mut stderr_sender = sender;
        std::thread::Builder::new()
            .name("virgo-lsp-stderr".to_string())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if block_on(stderr_sender.send(ServerEvent::Stderr(line))).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;

        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let root_uri = file_uri(&root);
        let mut server = Self {
            child,
            writer,
            next_request_id: 2,
            root,
            opened: HashSet::new(),
            versions: std::collections::HashMap::new(),
        };
        server
            .send(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": std::process::id(),
                    "clientInfo": { "name": "Virgo", "version": env!("CARGO_PKG_VERSION") },
                    "rootUri": root_uri,
                    "capabilities": {
                        "textDocument": {
                            "synchronization": { "didSave": true },
                            "completion": { "completionItem": { "documentationFormat": ["markdown", "plaintext"] } },
                            "publishDiagnostics": { "relatedInformation": true },
                            "definition": { "dynamicRegistration": false, "linkSupport": true },
                            "references": { "dynamicRegistration": false },
                            "hover": { "contentFormat": ["markdown", "plaintext"] }
                        }
                    }
                }
            }))
            .map_err(|error| error.to_string())?;
        Ok((
            server,
            taim::Task::run(receiver, Message::LanguageServerEvent),
        ))
    }

    pub fn initialized(&mut self) -> io::Result<()> {
        self.send(json!({"jsonrpc":"2.0", "method":"initialized", "params":{}}))
    }

    pub fn did_open(&mut self, path: &str, text: &str, language: &str) -> io::Result<()> {
        if !self.opened.insert(path.to_string()) {
            return self.did_change(path, text);
        }
        self.versions.insert(path.to_string(), 1);
        self.send(json!({
            "jsonrpc":"2.0", "method":"textDocument/didOpen", "params": {"textDocument": {
                "uri": self.document_uri(path), "languageId": language, "version": 1, "text": text
            }}
        }))
    }

    pub fn did_change(&mut self, path: &str, text: &str) -> io::Result<()> {
        if !self.opened.contains(path) {
            return self.did_open(path, text, language_id(path));
        }
        let uri = self.document_uri(path);
        let version = self.versions.entry(path.to_string()).or_insert(1);
        *version = version.saturating_add(1);
        let version = *version;
        self.send(json!({
            "jsonrpc":"2.0", "method":"textDocument/didChange", "params": {
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{"text": text}]
            }
        }))
    }

    pub fn completion(&mut self, path: &str, line: usize, column: usize) -> io::Result<u64> {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.send(json!({
            "jsonrpc":"2.0", "id": id, "method":"textDocument/completion", "params": {
                "textDocument": {"uri": self.document_uri(path)},
                "position": {"line": line, "character": column}
            }
        }))?;
        Ok(id)
    }

    pub fn definition(&mut self, path: &str, line: usize, column: usize) -> io::Result<u64> {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.send(json!({
            "jsonrpc":"2.0", "id": id, "method":"textDocument/definition", "params": {
                "textDocument": {"uri": self.document_uri(path)},
                "position": {"line": line, "character": column}
            }
        }))?;
        Ok(id)
    }

    pub fn references(&mut self, path: &str, line: usize, column: usize) -> io::Result<u64> {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.send(json!({
            "jsonrpc":"2.0", "id": id, "method":"textDocument/references", "params": {
                "textDocument": {"uri": self.document_uri(path)},
                "position": {"line": line, "character": column},
                "context": {"includeDeclaration": true}
            }
        }))?;
        Ok(id)
    }

    pub fn hover(&mut self, path: &str, line: usize, column: usize) -> io::Result<u64> {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.send(json!({
            "jsonrpc":"2.0", "id": id, "method":"textDocument/hover", "params": {
                "textDocument": {"uri": self.document_uri(path)},
                "position": {"line": line, "character": column}
            }
        }))?;
        Ok(id)
    }

    pub fn did_save(&mut self, path: &str, text: &str) -> io::Result<()> {
        if !self.opened.contains(path) {
            return Ok(());
        }
        self.send(json!({
            "jsonrpc":"2.0", "method":"textDocument/didSave", "params": {
                "textDocument": {"uri": self.document_uri(path)}, "text": text
            }
        }))
    }

    pub fn did_close(&mut self, path: &str) -> io::Result<()> {
        if !self.opened.remove(path) {
            return Ok(());
        }
        self.versions.remove(path);
        self.send(json!({
            "jsonrpc":"2.0", "method":"textDocument/didClose", "params": {
                "textDocument": {"uri": self.document_uri(path)}
            }
        }))
    }

    pub fn relative_path(&self, uri: &str) -> Option<String> {
        let decoded = decode_file_uri(uri)?;
        decoded
            .strip_prefix(&self.root)
            .ok()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
    }

    fn document_uri(&self, path: &str) -> String {
        file_uri(&self.root.join(path))
    }

    fn send(&mut self, value: Value) -> io::Result<()> {
        let bytes = encode_message(&value)?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::other("language server writer lock is poisoned"))?;
        writer.write_all(&bytes)?;
        writer.flush()
    }
}

impl Drop for LanguageServer {
    fn drop(&mut self) {
        let shutdown = json!({"jsonrpc":"2.0", "id": self.next_request_id, "method":"shutdown", "params":null});
        let _ = self.send(shutdown);
        let _ = self.send(json!({"jsonrpc":"2.0", "method":"exit", "params":null}));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Kind of an autocomplete suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Function,
    Struct,
    Enum,
    Trait,
    Variable,
    Module,
}

impl CompletionKind {
    pub fn icon(&self) -> &'static str {
        match self {
            CompletionKind::Keyword => "🔑",
            CompletionKind::Function => "ƒ",
            CompletionKind::Struct => "📦",
            CompletionKind::Enum => "⌥",
            CompletionKind::Trait => "✦",
            CompletionKind::Variable => "𝓍",
            CompletionKind::Module => "📁",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            CompletionKind::Keyword => "keyword",
            CompletionKind::Function => "function",
            CompletionKind::Struct => "struct",
            CompletionKind::Enum => "enum",
            CompletionKind::Trait => "trait",
            CompletionKind::Variable => "variable",
            CompletionKind::Module => "module",
        }
    }
}

/// A single autocomplete suggestion.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub documentation: String,
}

/// Autocomplete popover state.
#[derive(Debug, Clone)]
pub struct AutocompleteState {
    pub is_open: bool,
    pub query: String,
    pub selected_index: usize,
    pub items: Vec<CompletionItem>,
}

impl Default for AutocompleteState {
    fn default() -> Self {
        Self::new()
    }
}

impl AutocompleteState {
    pub fn new() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            selected_index: 0,
            items: vec![
                CompletionItem {
                    label: "button".to_string(),
                    kind: CompletionKind::Function,
                    detail: "pub fn button<'a, Message>(content: impl Into<Element<'a, Message>>) -> Button<'a, Message>".to_string(),
                    documentation: "Constructs a clickable push-button element with hardware GPU hover effects.".to_string(),
                },
                CompletionItem {
                    label: "row".to_string(),
                    kind: CompletionKind::Function,
                    detail: "pub fn row<'a, Message>() -> Row<'a, Message>".to_string(),
                    documentation: "Constructs a horizontal layout flex container.".to_string(),
                },
                CompletionItem {
                    label: "column".to_string(),
                    kind: CompletionKind::Function,
                    detail: "pub fn column<'a, Message>() -> Column<'a, Message>".to_string(),
                    documentation: "Constructs a vertical layout flex container.".to_string(),
                },
                CompletionItem {
                    label: "container".to_string(),
                    kind: CompletionKind::Function,
                    detail: "pub fn container<'a, Message>(content: impl Into<Element<'a, Message>>) -> Container<'a, Message>".to_string(),
                    documentation: "Wraps an element with styling, padding, and alignment constraints.".to_string(),
                },
                CompletionItem {
                    label: "Theme".to_string(),
                    kind: CompletionKind::Enum,
                    detail: "pub enum Theme { Dark, Light, HighContrast, Custom(...) }".to_string(),
                    documentation: "Global styling theme contract for widgets and canvas elements.".to_string(),
                },
                CompletionItem {
                    label: "Element".to_string(),
                    kind: CompletionKind::Struct,
                    detail: "pub struct Element<'a, Message, Theme, Renderer>".to_string(),
                    documentation: "Generic retained UI node in the Taim scene graph.".to_string(),
                },
                CompletionItem {
                    label: "pub fn".to_string(),
                    kind: CompletionKind::Keyword,
                    detail: "pub fn name(args) -> RetType".to_string(),
                    documentation: "Defines a public function.".to_string(),
                },
            ],
        }
    }

    pub fn filtered(&self) -> Vec<&CompletionItem> {
        if self.query.is_empty() {
            self.items.iter().collect()
        } else {
            let q = self.query.to_lowercase();
            self.items
                .iter()
                .filter(|i| i.label.to_lowercase().contains(&q))
                .collect()
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
    }

    pub fn replace_items(&mut self, items: Vec<CompletionItem>) {
        self.items = items;
        self.query.clear();
        self.selected_index = 0;
        self.is_open = !self.items.is_empty();
    }

    pub fn select_next(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    pub fn select_prev(&mut self) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected_index = if self.selected_index == 0 {
                count - 1
            } else {
                self.selected_index - 1
            };
        }
    }
}

#[derive(Debug, Clone)]
pub struct PublishedDiagnostic {
    pub line: usize,
    pub column: usize,
    pub severity: u64,
    pub code: String,
    pub message: String,
}

pub fn parse_diagnostics(value: &Value) -> Option<(&str, Vec<PublishedDiagnostic>)> {
    if value.get("method")?.as_str()? != "textDocument/publishDiagnostics" {
        return None;
    }
    let params = value.get("params")?;
    let uri = params.get("uri")?.as_str()?;
    let diagnostics = params
        .get("diagnostics")?
        .as_array()?
        .iter()
        .filter_map(|diagnostic| {
            let start = diagnostic.get("range")?.get("start")?;
            Some(PublishedDiagnostic {
                line: start.get("line")?.as_u64()? as usize + 1,
                column: start.get("character")?.as_u64()? as usize + 1,
                severity: diagnostic
                    .get("severity")
                    .and_then(Value::as_u64)
                    .unwrap_or(3),
                code: diagnostic
                    .get("code")
                    .map(|code| match code {
                        Value::String(code) => code.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default(),
                message: diagnostic.get("message")?.as_str()?.to_string(),
            })
        })
        .collect();
    Some((uri, diagnostics))
}

pub fn parse_completion_items(value: &Value) -> Vec<CompletionItem> {
    let Some(result) = value.get("result") else {
        return Vec::new();
    };
    let items = result
        .as_array()
        .or_else(|| result.get("items").and_then(Value::as_array));
    items
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let label = item.get("label")?.as_str()?.to_string();
            let kind = match item.get("kind").and_then(Value::as_u64).unwrap_or(6) {
                2 | 3 => CompletionKind::Function,
                7 | 22 => CompletionKind::Struct,
                13 => CompletionKind::Variable,
                9 => CompletionKind::Module,
                14 => CompletionKind::Keyword,
                10 => CompletionKind::Enum,
                8 => CompletionKind::Trait,
                _ => CompletionKind::Variable,
            };
            let detail = item
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let documentation = item
                .get("documentation")
                .and_then(|documentation| {
                    documentation
                        .as_str()
                        .or_else(|| documentation.get("value").and_then(Value::as_str))
                })
                .unwrap_or_default()
                .to_string();
            Some(CompletionItem {
                label,
                kind,
                detail,
                documentation,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspLocation {
    pub uri: String,
    pub line: usize,
    pub column: usize,
}

pub fn parse_definition_result(value: &Value) -> Option<Vec<LspLocation>> {
    let result = value.get("result")?;
    if result.is_null() {
        return None;
    }
    let parse_single = |obj: &Value| -> Option<LspLocation> {
        let uri = obj
            .get("uri")
            .or_else(|| obj.get("targetUri"))?
            .as_str()?
            .to_string();
        let range = obj
            .get("range")
            .or_else(|| obj.get("targetSelectionRange"))
            .or_else(|| obj.get("targetRange"))?;
        let start = range.get("start")?;
        let line = start.get("line")?.as_u64()? as usize + 1;
        let column = start.get("character")?.as_u64()? as usize + 1;
        Some(LspLocation { uri, line, column })
    };

    if let Some(arr) = result.as_array() {
        let list: Vec<LspLocation> = arr.iter().filter_map(parse_single).collect();
        if list.is_empty() { None } else { Some(list) }
    } else {
        parse_single(result).map(|loc| vec![loc])
    }
}

pub fn parse_references_result(value: &Value) -> Option<Vec<LspLocation>> {
    parse_definition_result(value)
}

pub fn parse_hover_result(value: &Value) -> Option<(String, String)> {
    let result = value.get("result")?;
    if result.is_null() {
        return None;
    }
    let contents = result.get("contents")?;
    if let Some(s) = contents.as_str() {
        return Some((s.to_string(), String::new()));
    }
    if let Some(markup) = contents.get("value").and_then(Value::as_str) {
        return Some((markup.to_string(), String::new()));
    }
    if let Some(arr) = contents.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                parts.push(s.to_string());
            } else if let Some(val) = item.get("value").and_then(Value::as_str) {
                parts.push(val.to_string());
            }
        }
        if !parts.is_empty() {
            let sig = parts.remove(0);
            let doc = parts.join("\n\n");
            return Some((sig, doc));
        }
    }
    None
}

pub fn language_id(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("rs") => "rust",
        Some("toml") => "toml",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("c" | "h") => "c",
        Some("cpp" | "cc" | "hpp") => "cpp",
        _ => "plaintext",
    }
}

fn encode_message(value: &Value) -> io::Result<Vec<u8>> {
    let payload = serde_json::to_vec(value).map_err(io::Error::other)?;
    let mut framed = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
    framed.extend(payload);
    Ok(framed)
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid LSP header",
            ));
        };
        if name.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(value.trim().parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid content length")
            })?);
        }
    }
    let length = content_length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing content length"))?;
    if length > 16 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LSP message exceeds 16 MB",
        ));
    }
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload)?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn file_uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let encoded: String = path
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
                vec![byte as char]
            } else {
                format!("%{byte:02X}").chars().collect()
            }
        })
        .collect();
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

fn decode_file_uri(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(decoded).ok()?;
    #[cfg(windows)]
    let decoded = decoded.strip_prefix('/').unwrap_or(&decoded).to_string();
    Some(PathBuf::from(decoded))
}

/// Symbol hover inspection info card.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub is_open: bool,
    pub symbol: String,
    pub signature: String,
    pub doc: String,
}

impl Default for HoverInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl HoverInfo {
    pub fn new() -> Self {
        Self {
            is_open: false,
            symbol: "DocumentManager".to_string(),
            signature: "pub struct DocumentManager".to_string(),
            doc: "High-performance multi-tab document state manager with split pane and live in-editor find/replace support.".to_string(),
        }
    }

    pub fn show(&mut self, symbol: &str, sig: &str, doc: &str) {
        self.symbol = symbol.to_string();
        self.signature = sig.to_string();
        self.doc = doc.to_string();
        self.is_open = true;
    }

    pub fn hide(&mut self) {
        self.is_open = false;
    }
}

/// Resolves path breadcrumbs for display at top of editor (e.g. `taim-project > src > lib.rs > run()`).
pub fn resolve_breadcrumbs(file_path: &str) -> Vec<String> {
    let mut parts = vec!["taim-project".to_string()];
    for segment in file_path.split('/') {
        if !segment.is_empty() {
            parts.push(segment.to_string());
        }
    }
    // Append current symbol context
    if file_path.ends_with(".rs") {
        parts.push("fn run()".to_string());
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn json_rpc_framing_round_trips_multiple_messages() {
        let first = json!({"jsonrpc":"2.0", "id":1, "result":{}});
        let second = json!({"jsonrpc":"2.0", "method":"ready", "params":{"ok":true}});
        let mut bytes = encode_message(&first).unwrap();
        bytes.extend(encode_message(&second).unwrap());
        let mut reader = Cursor::new(bytes);
        assert_eq!(read_message(&mut reader).unwrap(), Some(first));
        assert_eq!(read_message(&mut reader).unwrap(), Some(second));
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    #[test]
    fn malformed_and_oversized_frames_are_rejected() {
        let mut missing = Cursor::new(b"Other: 2\r\n\r\n{}".to_vec());
        assert_eq!(
            read_message(&mut missing).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let mut huge = Cursor::new(b"Content-Length: 20000000\r\n\r\n".to_vec());
        assert_eq!(
            read_message(&mut huge).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn diagnostics_and_completion_payloads_are_parsed_without_panics() {
        let diagnostics = json!({
            "jsonrpc":"2.0", "method":"textDocument/publishDiagnostics", "params": {
                "uri":"file:///tmp/main.rs", "diagnostics":[{
                    "range":{"start":{"line":2,"character":4},"end":{"line":2,"character":5}},
                    "severity":2, "code":"unused", "message":"unused value"
                }]
            }
        });
        let (uri, parsed) = parse_diagnostics(&diagnostics).unwrap();
        assert_eq!(uri, "file:///tmp/main.rs");
        assert_eq!(parsed.len(), 1);
        assert_eq!((parsed[0].line, parsed[0].column), (3, 5));
        let completion = json!({"id":2,"result":{"items":[{
            "label":"render", "kind":3, "detail":"fn render()",
            "documentation":{"kind":"markdown","value":"Renders a frame"}
        }]}});
        let parsed = parse_completion_items(&completion);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].label, "render");
        assert_eq!(parsed[0].kind, CompletionKind::Function);
        assert_eq!(parsed[0].documentation, "Renders a frame");
        assert!(parse_completion_items(&json!({"id":2,"result":null})).is_empty());
    }

    #[test]
    fn file_uri_round_trip_preserves_spaces_and_unicode() {
        let path = PathBuf::from("/tmp/Virgo project/नमस्ते.rs");
        let uri = file_uri(&path);
        assert!(uri.contains("%20"));
        assert_eq!(decode_file_uri(&uri).unwrap(), path);
    }

    #[test]
    fn definition_and_hover_payloads_are_parsed_correctly() {
        let def_resp = json!({
            "jsonrpc": "2.0",
            "id": 10,
            "result": [
                {
                    "uri": "file:///workspace/src/lib.rs",
                    "range": {
                        "start": { "line": 41, "character": 7 },
                        "end": { "line": 41, "character": 15 }
                    }
                }
            ]
        });
        let defs = parse_definition_result(&def_resp).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].uri, "file:///workspace/src/lib.rs");
        assert_eq!(defs[0].line, 42);
        assert_eq!(defs[0].column, 8);

        let link_resp = json!({
            "jsonrpc": "2.0",
            "id": 11,
            "result": {
                "targetUri": "file:///workspace/src/main.rs",
                "targetSelectionRange": {
                    "start": { "line": 9, "character": 3 },
                    "end": { "line": 9, "character": 7 }
                }
            }
        });
        let links = parse_definition_result(&link_resp).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].uri, "file:///workspace/src/main.rs");
        assert_eq!(links[0].line, 10);
        assert_eq!(links[0].column, 4);

        let hover_resp = json!({
            "jsonrpc": "2.0",
            "id": 12,
            "result": {
                "contents": [
                    "pub fn run() -> Result<()>",
                    "Starts the high-performance application."
                ]
            }
        });
        let (sig, doc) = parse_hover_result(&hover_resp).unwrap();
        assert_eq!(sig, "pub fn run() -> Result<()>");
        assert_eq!(doc, "Starts the high-performance application.");
    }
}
