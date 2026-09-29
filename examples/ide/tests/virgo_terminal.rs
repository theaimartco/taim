#![cfg(unix)]

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use taim_futures::futures::{StreamExt, executor::block_on};
use taim_ide::{Message, Studio};
use taim_runtime::Action;

#[test]
fn terminal_runs_in_workspace_and_keeps_shell_directory() {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let root = std::env::temp_dir().join(format!(
        "virgo-terminal-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("nested")).unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let task = ide.update(Message::NewTerminalTab);
    assert_eq!(ide.terminal.terminals.len(), 1);
    assert!(ide.terminal.terminals[0].running);
    let mut output = taim_runtime::task::into_stream(task).unwrap();

    let _ = ide.update(Message::TerminalFocusChanged(true));
    let _ = ide.update(Message::TerminalRawInput(
        b"printf 'VIRGO_OUTPUT_81237\\n'\r".to_vec(),
    ));
    let start = Instant::now();
    while !ide.terminal.terminals[0]
        .lines
        .iter()
        .any(|line| line == "VIRGO_OUTPUT_81237")
    {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "shell did not return command output: {:?}",
            ide.terminal.terminals[0].lines
        );
        let next = block_on(smol::future::or(output.next(), async {
            smol::Timer::after(Duration::from_millis(100)).await;
            None
        }));
        if let Some(Action::Output(message)) = next {
            let _ = ide.update(message);
        }
    }

    let _ = ide.update(Message::TerminalInputChanged("cd nested".into()));
    let _ = ide.update(Message::TerminalSubmit);
    let _ = ide.update(Message::TerminalInputChanged("pwd".into()));
    let _ = ide.update(Message::TerminalSubmit);
    let expected = root
        .canonicalize()
        .unwrap()
        .join("nested")
        .to_string_lossy()
        .to_string();
    let start = Instant::now();
    while !ide.terminal.terminals[0]
        .lines
        .iter()
        .any(|line| line == &expected)
    {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "shell did not retain the changed directory: {:?}",
            ide.terminal.terminals[0].lines
        );
        let next = block_on(smol::future::or(output.next(), async {
            smol::Timer::after(Duration::from_millis(100)).await;
            None
        }));
        if let Some(Action::Output(message)) = next {
            let _ = ide.update(message);
        }
    }

    let second = ide.update(Message::NewTerminalTab);
    assert_eq!(ide.terminal.terminals.len(), 2);
    assert_eq!(ide.terminal.active_terminal_idx, 1);
    let mut second_output = taim_runtime::task::into_stream(second).unwrap();
    let _ = ide.update(Message::TerminalInputChanged("pwd".into()));
    let _ = ide.update(Message::TerminalSubmit);
    let root_path = root.canonicalize().unwrap().to_string_lossy().to_string();
    let start = Instant::now();
    while !ide.terminal.terminals[1]
        .lines
        .iter()
        .any(|line| line == &root_path)
    {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "second shell did not start in workspace root"
        );
        let next = block_on(smol::future::or(second_output.next(), async {
            smol::Timer::after(Duration::from_millis(100)).await;
            None
        }));
        if let Some(Action::Output(message)) = next {
            let _ = ide.update(message);
        }
    }

    let _ = ide.update(Message::CloseTerminalTab(0));
    assert_eq!(ide.terminal.terminals.len(), 1);
    assert!(ide.terminal.terminals[0].running);
    let _ = ide.update(Message::CloseTerminalTab(0));
    assert!(ide.terminal.terminals.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn run_shortcut_starts_cargo_program_in_a_terminal() {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let root = std::env::temp_dir().join(format!(
        "virgo-run-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"virgo_run_probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main() { println!(\"VIRGO_RUN_72931\"); }\n",
    )
    .unwrap();
    let (mut ide, _) = Studio::with_workspace(root.clone()).unwrap();
    let task = ide.update(Message::TriggerRun);
    assert_eq!(ide.terminal.terminals.len(), 1);
    let mut output = taim_runtime::task::into_stream(task).unwrap();
    let start = Instant::now();
    while !ide.terminal.terminals[0]
        .lines
        .iter()
        .any(|line| line == "VIRGO_RUN_72931")
    {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "F5 run produced no program output: {:?}",
            ide.terminal.terminals[0].lines
        );
        let next = block_on(smol::future::or(output.next(), async {
            smol::Timer::after(Duration::from_millis(100)).await;
            None
        }));
        if let Some(Action::Output(message)) = next {
            let _ = ide.update(message);
        }
    }
    drop(ide);
    fs::remove_dir_all(root).unwrap();
}
