//! Shell command helpers for VIEW Desktop.
//!
//! Handles command submission, git context lines, history lookup, and
//! directory picker logic. No egui rendering in this module — pure logic.

use std::path::Path;
use std::process::Command;
use view_core::app::AppState;
use view_core::engine::Action;
use tokio::sync::mpsc;

// ── Git helpers ────────────────────────────────────────────────────────────────

pub fn git_prompt_details(cwd: &str) -> Option<(String, Option<String>)> {
    if cwd.is_empty() {
        return None;
    }

    let branch_output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .ok()?;
    if !branch_output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    let summary_output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .arg("diff")
        .arg("--shortstat")
        .arg("HEAD")
        .output()
        .ok();
    let summary = summary_output.and_then(|output| {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!text.is_empty()).then_some(text)
        } else {
            None
        }
    });

    Some((branch, summary))
}

pub fn format_command_context_line(
    cwd: &str,
    git_branch: Option<&str>,
    change_summary: Option<&str>,
) -> String {
    let mut parts = vec![cwd.to_string()];
    if let Some(branch) = git_branch.filter(|branch| !branch.is_empty()) {
        parts.push(format!("git:({branch})"));
    }
    if let Some(summary) = change_summary.filter(|summary| !summary.is_empty()) {
        parts.push(summary.to_string());
    }
    parts.join(" ")
}

// ── Path helpers ───────────────────────────────────────────────────────────────

pub fn shell_quote_path(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

pub fn truncate_path(value: &str, _max_chars: usize) -> String {
    let parts: Vec<&str> = value.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return "/".to_string();
    }
    if parts.len() == 1 {
        return format!("/{}", parts[0]);
    }
    format!("…/{}", parts.last().unwrap_or(&""))
}

// ── History helpers ────────────────────────────────────────────────────────────

pub fn history_entry_for_offset(
    history: &std::collections::VecDeque<String>,
    history_offset: usize,
) -> Option<String> {
    if history_offset == 0 || history_offset > history.len() {
        return None;
    }
    history.iter().rev().nth(history_offset - 1).cloned()
}

// ── Directory picker ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryOption {
    pub label: String,
    pub target_path: String,
    pub is_parent: bool,
}

pub fn directory_picker_options(cwd: &str, query: &str) -> Vec<DirectoryOption> {
    use std::fs;

    let path = Path::new(cwd);
    let query = query.trim().to_lowercase();
    let mut options = Vec::new();

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        options.push(DirectoryOption {
            label: ".. (Parent Directory)".to_string(),
            target_path: parent.display().to_string(),
            is_parent: true,
        });
    }

    let mut dirs = match fs::read_dir(path) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        Err(_) => return options,
    };
    dirs.sort_unstable_by_key(|label| label.to_lowercase());

    for label in dirs {
        if query.is_empty() || label.to_lowercase().contains(&query) {
            options.push(DirectoryOption {
                target_path: path.join(&label).display().to_string(),
                label,
                is_parent: false,
            });
        }
    }

    options
}

// ── Command submission ─────────────────────────────────────────────────────────

/// Submit a shell command: records history, appends transcript lines, and
/// sends the command bytes over the terminal channel.
///
/// Returns `true` if the command was dispatched successfully.
pub fn submit_shell_command(
    state: &mut AppState,
    action_tx: mpsc::UnboundedSender<Action>,
    history_offset: &mut usize,
    command: String,
) -> bool {
    let session_id = state.selected_terminal_idx;
    if state.selected_terminal().is_none() {
        return false;
    }

    state.append_terminal_history(state.selected_terminal_idx, command.clone());

    if crate::transcript::command_clears_transcript(&command) {
        state.clear_terminal_lines(state.selected_terminal_idx);
    } else {
        let cwd = state
            .selected_terminal()
            .map(|session| session.cwd.clone())
            .unwrap_or_default();
        let git_details = git_prompt_details(&cwd);
        let context_line = format_command_context_line(
            &cwd,
            git_details.as_ref().map(|(branch, _)| branch.as_str()),
            git_details
                .as_ref()
                .and_then(|(_, summary)| summary.as_deref()),
        );
        state.append_terminal_context_line(state.selected_terminal_idx, context_line);
        state.append_terminal_line(state.selected_terminal_idx, format!("$ {command}"));
    }

    let _ = action_tx.send(Action::SubmitCommand {
        session_id,
        command,
    });
    *history_offset = 0;
    true
}

// ── Suggestion helper ──────────────────────────────────────────────────────────

pub fn terminal_suggestion_suffix(input: &str, suggestion: Option<&str>) -> Option<String> {
    let suggestion = suggestion?;
    if input.is_empty() || suggestion == input {
        return None;
    }
    suggestion.strip_prefix(input).map(ToOwned::to_owned)
}
