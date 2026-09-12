//! Run & Debug Engine for Taim Studio.
//! Provides launch configs, execution control, breakpoints, call stacks, and variable evaluation.

/// Execution status of the debugger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugStatus {
    #[default]
    Stopped,
    Running,
    Paused,
}

impl DebugStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            DebugStatus::Stopped => "STOPPED",
            DebugStatus::Running => "RUNNING",
            DebugStatus::Paused => "PAUSED (Breakpoint)",
        }
    }
}

/// A source code breakpoint.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: usize,
    pub file: String,
    pub line: usize,
    pub enabled: bool,
}

/// A stack frame in the active call stack.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub id: usize,
    pub function_name: String,
    pub file: String,
    pub line: usize,
    pub is_selected: bool,
}

/// A variable in the active frame scope.
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub var_type: String,
    pub value: String,
}

/// A user watch expression.
#[derive(Debug, Clone)]
pub struct WatchExpression {
    pub expression: String,
    pub value: String,
}

/// Debug subsystem state.
#[derive(Debug, Clone)]
pub struct DebugState {
    pub status: DebugStatus,
    pub active_config: String,
    pub configs: Vec<String>,
    pub breakpoints: Vec<Breakpoint>,
    pub call_stack: Vec<StackFrame>,
    pub variables: Vec<Variable>,
    pub watch_expressions: Vec<WatchExpression>,
    pub new_watch_input: String,
    pub next_bp_id: usize,
}

impl Default for DebugState {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugState {
    pub fn new() -> Self {
        Self {
            status: DebugStatus::Stopped,
            active_config: "Debug taim-ide (lldb native)".to_string(),
            configs: vec![
                "Debug taim-ide (lldb native)".to_string(),
                "Release Benchmarks (perf)".to_string(),
                "Run Test Suite (cargo test)".to_string(),
            ],
            breakpoints: vec![
                Breakpoint {
                    id: 1,
                    file: "src/lib.rs".to_string(),
                    line: 42,
                    enabled: true,
                },
                Breakpoint {
                    id: 2,
                    file: "src/document.rs".to_string(),
                    line: 88,
                    enabled: true,
                },
            ],
            call_stack: vec![
                StackFrame {
                    id: 0,
                    function_name: "taim_ide::view_editors_area".to_string(),
                    file: "src/lib.rs".to_string(),
                    line: 42,
                    is_selected: true,
                },
                StackFrame {
                    id: 1,
                    function_name: "taim_ide::view".to_string(),
                    file: "src/lib.rs".to_string(),
                    line: 28,
                    is_selected: false,
                },
                StackFrame {
                    id: 2,
                    function_name: "taim_runtime::window::render_loop".to_string(),
                    file: "taim_runtime/src/window.rs".to_string(),
                    line: 312,
                    is_selected: false,
                },
            ],
            variables: vec![
                Variable {
                    name: "frame_time_ms".to_string(),
                    var_type: "f32".to_string(),
                    value: "0.284".to_string(),
                },
                Variable {
                    name: "fps".to_string(),
                    var_type: "u32".to_string(),
                    value: "120".to_string(),
                },
                Variable {
                    name: "resident_mem_mb".to_string(),
                    var_type: "f32".to_string(),
                    value: "26.4".to_string(),
                },
                Variable {
                    name: "active_pane".to_string(),
                    var_type: "usize".to_string(),
                    value: "0".to_string(),
                },
            ],
            watch_expressions: vec![
                WatchExpression {
                    expression: "self.theme_mode".to_string(),
                    value: "ThemeMode::Dark".to_string(),
                },
                WatchExpression {
                    expression: "self.docs.documents.len()".to_string(),
                    value: "4".to_string(),
                },
            ],
            new_watch_input: String::new(),
            next_bp_id: 3,
        }
    }

    /// Starts debugging session.
    pub fn start(&mut self) {
        self.status = DebugStatus::Paused;
    }

    /// Pauses debugging session.
    pub fn pause(&mut self) {
        self.status = DebugStatus::Paused;
    }

    /// Resumes running.
    pub fn continue_exec(&mut self) {
        self.status = DebugStatus::Running;
    }

    /// Steps over current line.
    pub fn step_over(&mut self) {
        if self.status == DebugStatus::Paused {
            let Some(frame) = self.call_stack.first_mut() else {
                return;
            };
            frame.line += 1;
        }
    }

    /// Stops debugging session.
    pub fn stop(&mut self) {
        self.status = DebugStatus::Stopped;
    }

    /// Toggles a breakpoint at file and line.
    pub fn toggle_breakpoint(&mut self, file: &str, line: usize) {
        if let Some(bp) = self
            .breakpoints
            .iter_mut()
            .find(|b| b.file == file && b.line == line)
        {
            bp.enabled = !bp.enabled;
        } else {
            let id = self.next_bp_id;
            self.next_bp_id += 1;
            self.breakpoints.push(Breakpoint {
                id,
                file: file.to_string(),
                line,
                enabled: true,
            });
        }
    }

    /// Adds a new watch expression.
    pub fn add_watch(&mut self, expr: &str) {
        let trimmed = expr.trim();
        if !trimmed.is_empty() {
            self.watch_expressions.push(WatchExpression {
                expression: trimmed.to_string(),
                value: "evaluating...".to_string(),
            });
            self.new_watch_input.clear();
        }
    }

    /// Removes a watch expression.
    pub fn remove_watch(&mut self, index: usize) {
        if index < self.watch_expressions.len() {
            self.watch_expressions.remove(index);
        }
    }
}
