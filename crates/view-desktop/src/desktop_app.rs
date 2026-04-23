use eframe::egui::{self, Align, Color32, Event, Frame, Key, Layout, RichText, ScrollArea, Stroke, ViewportCommand};
use image::{ImageBuffer, Rgba};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use parking_lot::RwLock;
use tokio::runtime::Builder;
use tokio::time::{self, Duration};
use view_core::{
    app::AppState,
    listener,
    terminal::{self, TerminalEvent},
};

use crate::{
    shell,
    shortcuts,
    transcript::{
        command_block_has_error, is_command_context_line, is_context_block_start,
        is_error_output_line, is_legacy_context_block_start, should_extend_error_block_to_bottom,
        should_render_block_separator,
    },
};

const BG_APP: Color32 = Color32::from_rgb(10, 11, 14);
const BG_PANEL: Color32 = Color32::from_rgb(7, 8, 12);
const BG_PANEL_ALT: Color32 = Color32::from_rgb(17, 19, 26);
const FG_PRIMARY: Color32 = Color32::from_rgb(234, 238, 255);
const FG_MUTED: Color32 = Color32::from_rgb(145, 154, 188);
const ACCENT: Color32 = Color32::from_rgb(108, 92, 231);
const ACCENT_ALT: Color32 = Color32::from_rgb(76, 201, 240);
const PICKER_HOVER: Color32 = Color32::from_rgb(46, 167, 208);
const ERROR_PANEL_BG: Color32 = Color32::from_rgb(96, 40, 40);
const ERROR_TEXT: Color32 = Color32::from_rgb(255, 228, 228);

pub struct ViewDesktopApp {
    state: Arc<RwLock<AppState>>,
    shell_input: String,
    history_offset: usize,
    directory_picker_open: bool,
    directory_picker_query: String,
    shell_txs: Vec<terminal::TerminalCommandTx>,
    frame_count: u64,
    screenshot_requested: bool,
    screenshot_target: Option<PathBuf>,
}

impl ViewDesktopApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        debug_log("desktop app: new()".to_string());
        configure_theme(&cc.egui_ctx);

        let state = Arc::new(RwLock::new(AppState::new()));
        let shell_txs = spawn_core_runtime(state.clone());

        Self {
            state,
            shell_input: String::new(),
            history_offset: 0,
            directory_picker_open: false,
            directory_picker_query: String::new(),
            shell_txs,
            frame_count: 0,
            screenshot_requested: false,
            screenshot_target: screenshot_target(),
        }
    }
}

// suggestion helper — delegated to shell module
#[inline]
fn terminal_suggestion_suffix(input: &str, suggestion: Option<&str>) -> Option<String> {
    shell::terminal_suggestion_suffix(input, suggestion)
}

// shell_quote_path — delegated to shell module
#[inline]
fn shell_quote_path(path: &str) -> String {
    shell::shell_quote_path(path)
}

fn directory_picker_options(cwd: &str, query: &str) -> Vec<shell::DirectoryOption> {
    shell::directory_picker_options(cwd, query)
}

fn submit_shell_command(
    state: &mut AppState,
    shell_txs: &[terminal::TerminalCommandTx],
    history_offset: &mut usize,
    command: String,
) -> bool {
    shell::submit_shell_command(state, shell_txs, history_offset, command)
}

fn history_entry_for_offset(
    history: &std::collections::VecDeque<String>,
    history_offset: usize,
) -> Option<String> {
    shell::history_entry_for_offset(history, history_offset)
}

fn draw_divider(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0, color));
}

impl eframe::App for ViewDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(120));
        self.frame_count += 1;
        debug_log(format!("frame={} entering update", self.frame_count));

        let mut state = self.state.write();
        shortcuts::handle(ctx, &mut state);

        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG_APP))
            .show(ctx, |ui| {
                render_focus(
                    ui,
                    &mut state,
                    &mut self.shell_input,
                    &mut self.history_offset,
                    &mut self.directory_picker_open,
                    &mut self.directory_picker_query,
                    &self.shell_txs,
                )
            });

        // Force disable IME to prevent macOS Vietnamese Telex from intercepting
        // terminal inputs, showing blue composing highlights, and eating spaces.
        ctx.output_mut(|o| o.ime = None);

        drop(state);
        self.maybe_capture_screenshot(ctx);
    }
}

impl ViewDesktopApp {
    fn maybe_capture_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot_target.clone() else {
            return;
        };

        if !self.screenshot_requested && self.frame_count >= 6 {
            debug_log(format!("frame={} requesting screenshot", self.frame_count));
            eprintln!(
                "desktop screenshot: requesting viewport screenshot at frame {}",
                self.frame_count
            );
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
            self.screenshot_requested = true;
            return;
        }

        let events = ctx.input(|input| input.events.clone());
        debug_log(format!(
            "frame={} input_events={}",
            self.frame_count,
            events.len()
        ));
        for event in events {
            if let Event::Screenshot { image, .. } = event {
                debug_log("received screenshot event".to_string());
                eprintln!("desktop screenshot: received screenshot event");
                if let Err(error) = save_color_image(&path, &image) {
                    eprintln!("Failed to save desktop screenshot to {:?}: {}", path, error);
                    debug_log(format!("failed saving screenshot: {error}"));
                } else {
                    eprintln!("Desktop screenshot saved to {:?}", path);
                    debug_log(format!("saved screenshot to {:?}", path));
                }
                ctx.send_viewport_cmd(ViewportCommand::Close);
                break;
            }
        }
    }
}

fn render_focus(
    ui: &mut egui::Ui,
    state: &mut AppState,
    shell_input: &mut String,
    history_offset: &mut usize,
    directory_picker_open: &mut bool,
    directory_picker_query: &mut String,
    shell_txs: &[terminal::TerminalCommandTx],
) {
    if let Some(session) = state.selected_terminal().cloned() {
        render_focus_terminal(
            ui,
            &session,
            state,
            shell_input,
            history_offset,
            directory_picker_open,
            directory_picker_query,
            shell_txs,
        );
    } else {
        ui.label("No session selected.");
    }
}

// handle_shortcuts moved to shortcuts.rs

fn configure_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(FG_PRIMARY);
    visuals.panel_fill = BG_APP;
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.inactive.bg_fill = BG_PANEL_ALT;
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.hovered.bg_fill = BG_PANEL;
    visuals.widgets.inactive.fg_stroke.color = FG_PRIMARY;
    visuals.window_fill = BG_APP;
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.5, ACCENT_ALT);
    ctx.set_visuals(visuals);
}

#[cfg(test)]
fn trim_line(value: &str, max_chars: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return value.to_string();
    }

    chars[..max_chars.saturating_sub(1)]
        .iter()
        .collect::<String>()
        + "…"
}

fn truncate_path(value: &str, _max_chars: usize) -> String {
    let parts: Vec<&str> = value.split('/').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        return "/".to_string();
    }

    if parts.len() == 1 {
        return format!("/{}", parts[0]);
    }

    format!("…/{}", parts.last().unwrap())
}

fn render_focus_terminal(
    ui: &mut egui::Ui,
    session: &view_core::app::TerminalState,
    state: &mut AppState,
    shell_input: &mut String,
    history_offset: &mut usize,
    directory_picker_open: &mut bool,
    directory_picker_query: &mut String,
    shell_txs: &[terminal::TerminalCommandTx],
) {
    ui.add_space(14.0);
    ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;

        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(truncate_path(&session.cwd, 72))
                    .monospace()
                    .color(FG_MUTED),
            );
        });

        ui.add_space(10.0);
        ui.separator();

        let transcript_height = (ui.available_height() - 110.0).max(180.0);
        let lines = state.recent_terminal_lines(state.selected_terminal_idx, 64);
        let transcript_ends_with_error = lines
            .iter()
            .rposition(|line| line.starts_with("$ "))
            .is_some_and(|index| command_block_has_error(&lines, index));
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(transcript_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                let num_lines = lines.len() as f32;
                let num_prompts = lines.iter().filter(|l| l.starts_with("$ ")).count() as f32;
                let estimated_height = (num_lines * 14.5) + (num_prompts * 18.0) + 10.0;
                let remaining_space = ui.available_height() - estimated_height;
                if remaining_space > 0.0 {
                    ui.add_space(remaining_space);
                }

                let mut index = 0usize;
                let mut previous_block_had_error = false;
                while index < lines.len() {
                    let line = lines[index];
                    let has_context_line = is_context_block_start(&lines, index);
                    if has_context_line || line.starts_with("$ ") {
                        let block_start = index;
                        let prompt_index = if has_context_line { index + 1 } else { index };
                        let mut block_end = prompt_index + 1;
                        while block_end < lines.len()
                            && !lines[block_end].starts_with("$ ")
                            && !is_context_block_start(&lines, block_end)
                            && !is_legacy_context_block_start(&lines, block_end)
                        {
                            block_end += 1;
                        }

                        let has_error = command_block_has_error(&lines, prompt_index);
                        if should_render_block_separator(previous_block_had_error, has_error) {
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);
                        }

                        let block_width = ui.available_width();
                        let extend_error_block_to_bottom = should_extend_error_block_to_bottom(
                            has_error,
                            block_end == lines.len(),
                        );
                        let block_height = if extend_error_block_to_bottom {
                            ui.available_height()
                        } else {
                            0.0
                        };
                        Frame::new()
                            .fill(if has_error {
                                ERROR_PANEL_BG
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(if has_error { 0.0 } else { 8.0 })
                            .inner_margin(egui::Margin::symmetric(
                                10,
                                if has_error { 16 } else { 8 },
                            ))
                            .show(ui, |ui| {
                                ui.set_min_width((block_width - 20.0).max(0.0));
                                if extend_error_block_to_bottom {
                                    ui.set_min_height(block_height);
                                }
                                for block_line in &lines[block_start..block_end] {
                                    let mut color = FG_PRIMARY;
                                    let mut is_bold = false;

                                    if is_command_context_line(block_line) {
                                        color = FG_MUTED;
                                    } else if block_line.starts_with("$ ") {
                                        color = if has_error {
                                            ERROR_TEXT
                                        } else {
                                            Color32::WHITE
                                        };
                                        is_bold = true;
                                    } else if block_line.starts_with("~ (") {
                                        color = FG_MUTED;
                                    } else if is_error_output_line(block_line) {
                                        color = ERROR_TEXT;
                                    }

                                    ui.label(if is_bold {
                                        RichText::new(*block_line).monospace().color(color).strong()
                                    } else {
                                        RichText::new(*block_line).monospace().color(color)
                                    });
                                }
                            });

                        previous_block_had_error = has_error;
                        index = block_end;
                        while index < lines.len() && lines[index].trim().is_empty() {
                            index += 1;
                        }
                        continue;
                    }

                    let color = if line.starts_with("~ (") {
                        FG_MUTED
                    } else if is_error_output_line(line) {
                        Color32::from_rgb(255, 205, 205)
                    } else {
                        FG_PRIMARY
                    };

                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new(line).monospace().color(color));
                    });
                    previous_block_had_error = false;
                    index += 1;
                }
            });

        if transcript_ends_with_error {
            ui.add_space(0.0);
        } else {
            ui.add_space(16.0);
            draw_divider(ui, Color32::from_gray(60));
            ui.add_space(12.0);
        }

        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let short_cwd = truncate_path(&session.cwd, 40);
            let directory_button = egui::Button::new(
                RichText::new(format!("📁 {}", short_cwd))
                    .color(FG_PRIMARY)
                    .size(13.0)
                    .monospace(),
            )
            .stroke(Stroke::new(1.0, Color32::from_gray(60)))
            .corner_radius(6.0)
            .fill(BG_PANEL_ALT)
            .min_size(egui::vec2(0.0, 28.0));
            if ui.add(directory_button).clicked() {
                *directory_picker_open = true;
                directory_picker_query.clear();
            }
        });

        if *directory_picker_open {
            let mut close_picker = false;
            if ui.ctx().input(|input| input.key_pressed(Key::Escape)) {
                close_picker = true;
            }

            egui::Window::new("directory_picker")
                .title_bar(false)
                .resizable(false)
                .fixed_size(egui::vec2(400.0, 332.0))
                .anchor(egui::Align2::LEFT_BOTTOM, [14.0, -72.0])
                .show(ui.ctx(), |ui| {
                    let search_id = ui.make_persistent_id("directory_picker_search");
                    let search = Frame::new()
                        .fill(BG_APP)
                        .stroke(Stroke::NONE)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.add_sized(
                                [ui.available_width(), 28.0],
                                egui::TextEdit::singleline(directory_picker_query)
                                    .id(search_id)
                                    .hint_text("Search directories...")
                                    .frame(false),
                            )
                        })
                        .inner;
                    if !search.has_focus() {
                        search.request_focus();
                    }

                    ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for option in directory_picker_options(&session.cwd, directory_picker_query)
                        {
                            let prefix = if option.is_parent { "↑" } else { "🗀" };
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 28.0),
                                egui::Sense::click(),
                            );
                            if response.hovered() {
                                ui.painter().rect_filled(rect, 0.0, PICKER_HOVER);
                            }

                            let text_color = if response.hovered() {
                                BG_APP
                            } else if option.is_parent {
                                FG_MUTED
                            } else {
                                FG_PRIMARY
                            };
                            ui.painter().text(
                                egui::pos2(rect.min.x + 10.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                prefix,
                                egui::FontId::monospace(13.0),
                                text_color,
                            );
                            ui.painter().text(
                                egui::pos2(rect.min.x + 34.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &option.label,
                                egui::FontId::monospace(12.0),
                                text_color,
                            );

                            if response.clicked() {
                                let command =
                                    format!("cd {}", shell_quote_path(&option.target_path));
                                if submit_shell_command(state, shell_txs, history_offset, command) {
                                    shell_input.clear();
                                    close_picker = true;
                                }
                            }
                        }
                    });
                });
            if close_picker {
                *directory_picker_open = false;
                directory_picker_query.clear();
            }
        }

        ui.add_space(12.0);

        let input_id = ui.make_persistent_id("shell_input");

        let mut rect = ui.available_rect_before_wrap();
        rect.set_height(24.0);
        rect.min.x += 14.0;

        let suggestion = state.get_terminal_suggestion(state.selected_terminal_idx, shell_input);
        let suggestion_suffix = terminal_suggestion_suffix(shell_input, suggestion.as_deref());

        if shell_input.is_empty() {
            let mut placeholder_rect = rect;
            placeholder_rect.min.x += 2.0;
            let mut placeholder_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(placeholder_rect)
                    .id_salt("placeholder_ui"),
            );
            placeholder_ui.label(
                RichText::new("Run commands")
                    .color(FG_MUTED)
                    .size(13.0)
                    .monospace(),
            );
        } else if let Some(ref suffix) = suggestion_suffix {
            let mut ghost_rect = rect;
            ghost_rect.min.x += 2.0;
            let mut ghost_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(ghost_rect)
                    .id_salt("ghost_ui"),
            );
            let font_id = egui::TextStyle::Monospace.resolve(ui.style());
            let prefix_width = ui
                .painter()
                .layout_no_wrap(shell_input.clone(), font_id, Color32::TRANSPARENT)
                .size()
                .x;
            ghost_ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(prefix_width);
                ui.label(
                    RichText::new(suffix)
                        .color(Color32::from_gray(80))
                        .size(13.0)
                        .monospace(),
                );

                ui.add_space(12.0);
                let arrow_frame = Frame::new()
                    .stroke(Stroke::new(1.0, Color32::from_gray(50)))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(4, 2));
                arrow_frame.show(ui, |ui| {
                    ui.label(RichText::new("→▾").color(FG_MUTED).size(10.0).monospace());
                });
            });
        }

        let response = ui.add_sized(
            [ui.available_width() - 28.0, 24.0],
            egui::TextEdit::singleline(shell_input)
                .id(input_id)
                .frame(false)
                .margin(egui::Margin::symmetric(14, 0))
                .font(egui::TextStyle::Monospace),
        );

        if response.has_focus() {
            let history = &session.history;
            if !history.is_empty() {
                if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                    if *history_offset < history.len() {
                        *history_offset += 1;
                    }
                    if let Some(entry) = history_entry_for_offset(history, *history_offset) {
                        *shell_input = entry;
                        if let Some(mut text_state) = egui::TextEdit::load_state(ui.ctx(), input_id)
                        {
                            let ccursor = egui::text::CCursor::new(shell_input.chars().count());
                            text_state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                            text_state.store(ui.ctx(), input_id);
                        }
                    }
                } else if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                    if *history_offset > 1 {
                        *history_offset -= 1;
                        if let Some(entry) = history_entry_for_offset(history, *history_offset) {
                            *shell_input = entry;
                        }
                    } else if *history_offset == 1 {
                        *history_offset = 0;
                        shell_input.clear();
                    }
                    if let Some(mut text_state) = egui::TextEdit::load_state(ui.ctx(), input_id) {
                        let ccursor = egui::text::CCursor::new(shell_input.chars().count());
                        text_state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                        text_state.store(ui.ctx(), input_id);
                    }
                }
            }
        }

        if let Some(sugg) = suggestion {
            if response.has_focus()
                && (ui.input(|i| i.key_pressed(Key::Tab))
                    || ui.input(|i| i.key_pressed(Key::ArrowRight)))
            {
                *shell_input = sugg;
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), input_id) {
                    let ccursor = egui::text::CCursor::new(shell_input.chars().count());
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                    state.store(ui.ctx(), input_id);
                }
            }
        }

        if !*directory_picker_open && !response.has_focus() && shell_input.is_empty() {
            ui.memory_mut(|memory| memory.request_focus(input_id));
        }
        if response.lost_focus()
            && ui.input(|input| input.key_pressed(Key::Enter))
            && !shell_input.trim().is_empty()
        {
            let command = shell_input.trim().to_string();
            if submit_shell_command(state, shell_txs, history_offset, command) {
                shell_input.clear();
                ui.memory_mut(|memory| memory.request_focus(input_id));
            }
        }
    });
}

fn spawn_core_runtime(state: Arc<RwLock<AppState>>) -> Vec<terminal::TerminalCommandTx> {
    let mut shell_txs = Vec::new();
    let mut shell_rxs = Vec::new();
    for _ in 0..1 {
        let (tx, rx) = terminal::local_shell_command_tx();
        shell_txs.push(tx);
        shell_rxs.push(rx);
    }
    thread::spawn(move || {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("desktop runtime");

        runtime.block_on(async move {
            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
            let (agent_tx, mut agent_rx) = tokio::sync::mpsc::channel(64);
            let (terminal_event_tx, mut terminal_event_rx) = tokio::sync::mpsc::unbounded_channel();

            // Spawn LAN web server (REST + WebSocket) on a background task.
            let web_state = state.clone();
            let web_port: u16 = std::env::var("VIEW_WEB_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(23779);
            tokio::spawn(async move {
                if let Err(err) = view_web::start(web_state, web_port).await {
                    eprintln!("view-web server error: {err}");
                }
            });

            tokio::spawn(async move {
                let _ = listener::start_demo_listener(event_tx, agent_tx).await;
            });

            for (session_id, shell_rx) in shell_rxs.into_iter().enumerate() {
                let terminal_event_tx = terminal_event_tx.clone();
                tokio::spawn(async move {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
                    let _ =
                        terminal::start_local_shell(session_id, cwd, terminal_event_tx, shell_rx)
                            .await;
                });
            }

            let mut tick = time::interval(Duration::from_secs(1));

            loop {
                tokio::select! {
                    Some(event) = event_rx.recv() => {
                        let mut app = state.write();
                        app.add_event(event);
                    }
                    Some(agent) = agent_rx.recv() => {
                        let mut app = state.write();
                        app.update_agent(agent);
                    }
                    Some(terminal_event) = terminal_event_rx.recv() => {
                        let mut app = state.write();
                        match terminal_event {
                            TerminalEvent::Line { session_id, line } => {
                                app.append_terminal_line(session_id, line)
                            }
                            TerminalEvent::Status { session_id, status } => {
                                app.set_terminal_status(session_id, status)
                            }
                            TerminalEvent::Cwd { session_id, cwd } => {
                                app.set_terminal_cwd(session_id, cwd)
                            }
                            TerminalEvent::Timing { session_id, seconds } => {
                                app.finalize_terminal_context_line(session_id, seconds)
                            }
                        }
                    }
                    _ = tick.tick() => {
                        let mut app = state.write();
                        app.tick_activity();
                    }
                }
            }
        });
    });
    shell_txs
}

fn screenshot_target() -> Option<PathBuf> {
    std::env::var("VIEW_DESKTOP_SCREENSHOT_TO")
        .or_else(|_| std::env::var("EFRAME_SCREENSHOT_TO"))
        .ok()
        .map(PathBuf::from)
}

fn debug_log_path() -> Option<PathBuf> {
    std::env::var("VIEW_DESKTOP_DEBUG_LOG")
        .ok()
        .map(PathBuf::from)
}

fn debug_log(message: String) {
    let Some(path) = debug_log_path() else {
        return;
    };

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

fn save_color_image(path: &PathBuf, image: &egui::ColorImage) -> anyhow::Result<()> {
    let mut rgba = Vec::with_capacity(image.pixels.len() * 4);
    for color in &image.pixels {
        let [r, g, b, a] = color.to_array();
        rgba.extend_from_slice(&[r, g, b, a]);
    }

    let Some(buffer) =
        ImageBuffer::<Rgba<u8>, _>::from_raw(image.size[0] as u32, image.size[1] as u32, rgba)
    else {
        return Err(anyhow::anyhow!("Failed to build RGBA image buffer"));
    };

    buffer.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{screenshot_target, terminal_suggestion_suffix, trim_line, truncate_path};
    use crate::shell::{
        directory_picker_options, format_command_context_line, history_entry_for_offset,
        shell_quote_path, terminal_suggestion_suffix as shell_suggestion_suffix,
    };
    use crate::transcript::{
        command_block_has_error, command_clears_transcript, is_context_block_start,
        is_legacy_context_block_start, should_extend_error_block_to_bottom,
        should_render_block_separator,
    };

    #[test]
    fn string_helpers_should_trim_without_panicking() {
        assert_eq!(trim_line("abcdef", 4), "abc…");
        assert_eq!(truncate_path("/a/very/long/path/file.rs", 10), "…/file.rs");
    }

    #[test]
    fn screenshot_target_should_prefer_view_desktop_specific_env() {
        std::env::set_var("VIEW_DESKTOP_SCREENSHOT_TO", "/tmp/a.png");
        std::env::set_var("EFRAME_SCREENSHOT_TO", "/tmp/b.png");

        assert_eq!(
            screenshot_target().as_deref(),
            Some(std::path::Path::new("/tmp/a.png"))
        );

        std::env::remove_var("VIEW_DESKTOP_SCREENSHOT_TO");
        std::env::remove_var("EFRAME_SCREENSHOT_TO");
    }

    #[test]
    fn terminal_suggestion_suffix_should_render_only_remaining_text() {
        assert_eq!(
            terminal_suggestion_suffix("cd ", Some("cd ..")),
            Some("..".to_string())
        );
    }

    #[test]
    fn terminal_suggestion_suffix_should_ignore_exact_matches() {
        assert_eq!(terminal_suggestion_suffix("cd ..", Some("cd ..")), None);
    }

    #[test]
    fn command_clears_transcript_should_match_clear_aliases() {
        assert!(command_clears_transcript("clear"));
        assert!(command_clears_transcript(" cls "));
        assert!(!command_clears_transcript("clear now"));
    }

    #[test]
    fn command_block_has_error_should_detect_failed_command_output() {
        let lines = vec![
            "$ cd vivisual_interception_event_window",
            "~ (0.0006s)",
            "cd: no such file or directory: vivisual_interception_event_window",
            "$ cd visual_interception_event_window",
        ];

        assert!(command_block_has_error(&lines, 0));
        assert!(!command_block_has_error(&lines, 3));
    }

    #[test]
    fn should_render_block_separator_should_skip_gap_below_error_blocks() {
        assert!(should_render_block_separator(false, false));
        assert!(!should_render_block_separator(true, false));
        assert!(!should_render_block_separator(false, true));
    }

    #[test]
    fn should_extend_error_block_to_bottom_should_only_apply_to_final_error_block() {
        assert!(should_extend_error_block_to_bottom(true, true));
        assert!(!should_extend_error_block_to_bottom(true, false));
        assert!(!should_extend_error_block_to_bottom(false, true));
    }

    #[test]
    fn format_command_context_line_should_include_git_details_when_present() {
        assert_eq!(
            format_command_context_line(
                "/tmp/project",
                Some("main"),
                Some("4 files changed, 10 insertions(+)")
            ),
            "/tmp/project git:(main) 4 files changed, 10 insertions(+)"
        );
    }

    #[test]
    fn context_block_detection_should_handle_current_and_legacy_layouts() {
        let current = vec!["/tmp/project git:(main)", "$ ls"];
        let legacy = vec!["", "/tmp/project git:(main)", "$ ls"];

        assert!(is_context_block_start(&current, 0));
        assert!(is_legacy_context_block_start(&legacy, 0));
    }

    #[test]
    fn history_entry_for_offset_should_return_most_recent_first() {
        let history = std::collections::VecDeque::from(vec!["ls".to_string(), "cd ..".to_string()]);

        assert_eq!(
            history_entry_for_offset(&history, 1).as_deref(),
            Some("cd ..")
        );
        assert_eq!(history_entry_for_offset(&history, 2).as_deref(), Some("ls"));
    }

    #[test]
    fn shell_quote_path_should_escape_single_quotes() {
        assert_eq!(shell_quote_path("/tmp/it's-here"), "'/tmp/it'\\''s-here'");
    }

    #[test]
    fn directory_picker_options_should_include_parent_and_filter_children() {
        let root = std::env::temp_dir().join(format!(
            "view-picker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("alpha")).expect("alpha");
        std::fs::create_dir_all(root.join("beta")).expect("beta");

        let options = directory_picker_options(root.to_str().expect("utf8"), "alp");

        assert!(options.iter().any(|option| option.is_parent));
        assert!(options.iter().any(|option| option.label == "alpha"));
        assert!(!options.iter().any(|option| option.label == "beta"));

        let _ = std::fs::remove_dir_all(root);
    }
}
