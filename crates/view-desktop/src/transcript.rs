//! Terminal transcript rendering for VIEW Desktop.
//!
//! Handles the scrollable output area: command blocks, error highlighting,
//! context lines (cwd + git), and block separators. Kept separate from
//! input handling and shell plumbing so each can evolve independently.

// Local color constants (removed as they are no longer used by the inlined render logic)

// ── Helpers re-exported for tests ─────────────────────────────────────────────

pub fn command_clears_transcript(command: &str) -> bool {
    matches!(command.trim(), "clear" | "cls")
}

pub fn is_error_output_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("command not found")
        || lower.contains("no such file or directory")
        || lower.contains("error:")
        || lower.contains("permission denied")
        || lower.starts_with("zsh: ")
}

pub fn command_block_has_error(lines: &[&str], prompt_index: usize) -> bool {
    lines
        .iter()
        .skip(prompt_index + 1)
        .take_while(|line| !line.starts_with("$ "))
        .any(|line| is_error_output_line(line))
}

#[cfg(test)]
pub fn should_render_block_separator(
    previous_block_had_error: bool,
    current_block_has_error: bool,
) -> bool {
    !current_block_has_error && !previous_block_had_error
}

pub fn should_extend_error_block_to_bottom(has_error: bool, is_last_block: bool) -> bool {
    has_error && is_last_block
}

pub fn is_command_context_line(line: &str) -> bool {
    line.starts_with('/')
}

pub fn is_context_block_start(lines: &[&str], index: usize) -> bool {
    lines
        .get(index)
        .is_some_and(|line| is_command_context_line(line))
        && lines
            .get(index + 1)
            .is_some_and(|next| next.starts_with("$ "))
}

pub fn is_legacy_context_block_start(lines: &[&str], index: usize) -> bool {
    lines.get(index).is_some_and(|line| line.trim().is_empty())
        && lines
            .get(index + 1)
            .is_some_and(|line| is_command_context_line(line))
        && lines
            .get(index + 2)
            .is_some_and(|next| next.starts_with("$ "))
}

// Render logic was moved to desktop_app.rs and these functions are dead code.
