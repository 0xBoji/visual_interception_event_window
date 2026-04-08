use eframe::egui::{
    self, Align, Color32, Event, Frame, Key, Layout, RichText, ScrollArea, Stroke, Vec2,
    ViewportCommand,
};
use image::{ImageBuffer, Rgba};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tokio::runtime::Builder;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use view_core::{
    app::{Agent, AppState, ViewMode},
    listener,
};

const BG_APP: Color32 = Color32::from_rgb(18, 21, 31);
const BG_PANEL: Color32 = Color32::from_rgb(24, 28, 40);
const BG_PANEL_ALT: Color32 = Color32::from_rgb(32, 37, 54);
const FG_PRIMARY: Color32 = Color32::from_rgb(234, 238, 255);
const FG_MUTED: Color32 = Color32::from_rgb(145, 154, 188);
const ACCENT: Color32 = Color32::from_rgb(108, 92, 231);
const ACCENT_ALT: Color32 = Color32::from_rgb(76, 201, 240);
const ACCENT_ALT_2: Color32 = Color32::from_rgb(255, 122, 198);
const SUCCESS: Color32 = Color32::from_rgb(109, 234, 170);
const WARNING: Color32 = Color32::from_rgb(255, 184, 76);
const OFFLINE: Color32 = Color32::from_rgb(122, 128, 158);
const DANGER: Color32 = Color32::from_rgb(255, 96, 96);

pub struct ViewDesktopApp {
    state: Arc<Mutex<AppState>>,
    search_buffer: String,
    frame_count: u64,
    screenshot_requested: bool,
    screenshot_target: Option<PathBuf>,
}

impl ViewDesktopApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        debug_log("desktop app: new()".to_string());
        configure_theme(&cc.egui_ctx);

        let state = Arc::new(Mutex::new(AppState::new()));
        spawn_core_runtime(state.clone());

        Self {
            state,
            search_buffer: String::new(),
            frame_count: 0,
            screenshot_requested: false,
            screenshot_target: screenshot_target(),
        }
    }
}

impl eframe::App for ViewDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(120));
        self.frame_count += 1;
        debug_log(format!("frame={} entering update", self.frame_count));

        let mut state = self.state.blocking_lock();
        let search_buffer = &mut self.search_buffer;
        handle_shortcuts(ctx, &mut state, search_buffer);

        egui::TopBottomPanel::top("header")
            .frame(
                Frame::new()
                    .fill(BG_PANEL_ALT)
                    .stroke(Stroke::new(1.0, ACCENT)),
            )
            .show(ctx, |ui| render_header(ui, &mut state, search_buffer));

        egui::TopBottomPanel::bottom("footer")
            .frame(Frame::new().fill(BG_PANEL_ALT))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Tab/grid/focus").color(FG_MUTED));
                    ui.separator();
                    ui.label(RichText::new("filter").color(FG_MUTED));
                    ui.separator();
                    ui.label(RichText::new("search").color(FG_MUTED));
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "mode:{} • filter:{} • visible:{}",
                            match state.view_mode {
                                ViewMode::Grid => "grid",
                                ViewMode::Focus => "focus",
                            },
                            state.filter_label(),
                            state.visible_agent_count()
                        ))
                        .color(FG_MUTED),
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG_APP))
            .show(ctx, |ui| match state.view_mode {
                ViewMode::Grid => render_grid(ui, &mut state),
                ViewMode::Focus => render_focus(ui, &mut state),
            });

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

fn render_header(ui: &mut egui::Ui, state: &mut AppState, search_buffer: &mut String) {
    let page_size = grid_page_size(ui.available_width());
    let tabs = state.visible_agents_page(page_size);
    let selected = state.get_selected_agent_id();
    let current_page = state.current_grid_page(page_size) + 1;
    let total_pages = state.grid_page_count(page_size);

    ui.horizontal_wrapped(|ui| {
        ui.add_space(4.0);
        chip(ui, "VIEW", ACCENT, true);
        ui.label(
            RichText::new(format!(
                "{} sessions • {} visible",
                state.agents.len(),
                state.visible_agent_count()
            ))
            .color(FG_MUTED),
        );
        ui.label(
            RichText::new(format!("page {current_page}/{total_pages}"))
                .monospace()
                .color(FG_MUTED),
        );

        for (index, agent_id) in tabs.iter().enumerate() {
            let is_selected = selected.as_deref() == Some(agent_id.as_str());
            if tab_chip(ui, agent_id, is_selected).clicked() {
                let base = state.current_grid_page(page_size) * page_size;
                state.select_visible_index(base + index);
            }
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let search = ui.add_sized(
                [220.0, 28.0],
                egui::TextEdit::singleline(search_buffer)
                    .id_source("desktop_search")
                    .hint_text("search sessions"),
            );
            if state.search_mode && !search.has_focus() {
                search.request_focus();
            }
            if search.changed() {
                state.set_search_query(search_buffer);
            }
            if search.lost_focus() && state.search_mode {
                state.end_search();
            }

            if ui.button("focus").clicked() {
                state.view_mode = ViewMode::Focus;
            }
            if ui.button("grid").clicked() {
                state.view_mode = ViewMode::Grid;
            }
            if ui.button("next").clicked() {
                state.select_next_page();
            }
            if ui.button("prev").clicked() {
                state.select_previous_page();
            }
            if ui.button("filter").clicked() {
                state.cycle_filter_mode();
            }

            ui.label(
                RichText::new(format!("filter:{}", state.filter_label()))
                    .monospace()
                    .color(FG_MUTED),
            );
        });
    });
}

fn render_grid(ui: &mut egui::Ui, state: &mut AppState) {
    let columns = grid_columns_for_width(ui.available_width());
    let rows = grid_rows();
    let page_size = columns * rows;
    let ids = state.visible_agents_page(page_size);
    let selected = state.get_selected_agent_id();
    let spacing = 12.0;
    let total_spacing = spacing * (columns.saturating_sub(1) as f32);
    let tile_width = ((ui.available_width() - total_spacing).max(320.0)) / columns as f32;
    let tile_size = Vec2::new(tile_width.max(320.0), 255.0);

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for row in 0..rows {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = spacing;

                    for col in 0..columns {
                        let index = row * columns + col;
                        ui.allocate_ui_with_layout(
                            tile_size,
                            Layout::top_down(Align::LEFT),
                            |ui| {
                                if let Some(id) = ids.get(index) {
                                    if let Some(agent) = state.agents.get(id).cloned() {
                                        let is_selected = selected.as_deref() == Some(id.as_str());
                                        render_tile(
                                            ui,
                                            &agent,
                                            state,
                                            is_selected,
                                            index,
                                            tile_size,
                                        );
                                    } else {
                                        ui.allocate_space(tile_size);
                                    }
                                } else {
                                    ui.allocate_space(tile_size);
                                }
                            },
                        );
                    }
                });
                ui.add_space(spacing);
            }
        });
}

fn render_focus(ui: &mut egui::Ui, state: &mut AppState) {
    ui.columns(2, |columns| {
        columns[0].heading("Sessions");
        for (index, id) in state.visible_agent_ids().iter().enumerate() {
            let selected = state.get_selected_agent_id().as_deref() == Some(id.as_str());
            if columns[0]
                .selectable_label(selected, id)
                .on_hover_text("Select session")
                .clicked()
            {
                state.select_visible_index(index);
            }
        }

        columns[1].heading("Detail");
        if let Some(agent) = state.get_selected_agent().cloned() {
            render_focus_detail(&mut columns[1], &agent, state);
        } else {
            columns[1].label("No session selected.");
        }
    });
}

fn handle_shortcuts(ctx: &egui::Context, state: &mut AppState, search_buffer: &mut String) {
    if ctx.input(|input| input.key_pressed(Key::Tab)) {
        state.toggle_view_mode();
    }

    if ctx.input(|input| input.key_pressed(Key::ArrowDown) || input.key_pressed(Key::J)) {
        state.select_next();
    }

    if ctx.input(|input| input.key_pressed(Key::ArrowUp) || input.key_pressed(Key::K)) {
        state.select_previous();
    }

    if ctx.input(|input| input.key_pressed(Key::PageDown)) {
        state.select_next_page();
    }

    if ctx.input(|input| input.key_pressed(Key::PageUp)) {
        state.select_previous_page();
    }

    if ctx.input(|input| input.key_pressed(Key::F)) {
        state.cycle_filter_mode();
    }

    if ctx.input(|input| input.key_pressed(Key::Slash)) {
        state.begin_search();
    }

    if ctx.input(|input| input.key_pressed(Key::Escape)) {
        state.clear_search_query();
        search_buffer.clear();
        state.end_search();
    }
}

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

fn chip(ui: &mut egui::Ui, label: &str, color: Color32, filled: bool) {
    let text = RichText::new(label)
        .strong()
        .color(if filled { BG_APP } else { color });
    let frame = Frame::new()
        .fill(if filled { color } else { BG_PANEL_ALT })
        .stroke(Stroke::new(1.0, color))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(8, 4));
    frame.show(ui, |ui| {
        ui.label(text);
    });
}

fn tab_chip(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let text = RichText::new(label)
        .strong()
        .color(if selected { BG_APP } else { FG_PRIMARY });
    egui::Frame::new()
        .fill(if selected { ACCENT_ALT } else { BG_PANEL_ALT })
        .stroke(Stroke::new(1.0, if selected { ACCENT_ALT } else { ACCENT }))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| ui.button(text))
        .inner
}

fn render_tile(
    ui: &mut egui::Ui,
    agent: &Agent,
    state: &mut AppState,
    selected: bool,
    visible_index: usize,
    tile_size: Vec2,
) {
    let border = if selected { ACCENT_ALT } else { ACCENT };
    let fill = if selected { BG_PANEL } else { BG_PANEL_ALT };
    let events = state.get_recent_events(Some(&agent.id), 4);
    let status = status_color(agent);
    let messages = agent
        .metadata
        .get("messages")
        .cloned()
        .unwrap_or_else(|| "-".to_string());
    let last_tool = agent
        .metadata
        .get("last_tool")
        .cloned()
        .unwrap_or_else(|| "idle".to_string());

    let response = Frame::new()
        .fill(fill)
        .stroke(Stroke::new(if selected { 2.0 } else { 1.0 }, border))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_size(tile_size);
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").color(status).size(16.0));
                ui.label(RichText::new(&agent.id).strong().size(18.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    chip(
                        ui,
                        &agent.status.to_uppercase(),
                        if selected { ACCENT_ALT } else { status },
                        false,
                    );
                });
            });
            ui.label(
                RichText::new(format!(
                    "{}/{} • {} • {}",
                    agent.project, agent.role, last_tool, agent.branch
                ))
                .color(FG_MUTED),
            );
            ui.separator();
            ui.label(
                RichText::new(format!("activity {}", trendline(agent, 32)))
                    .monospace()
                    .color(status),
            );
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("tokens {}", agent.tokens))
                        .monospace()
                        .color(FG_PRIMARY),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("msgs {}", messages))
                        .monospace()
                        .color(FG_MUTED),
                );
                ui.separator();
                ui.label(
                    RichText::new(
                        agent
                            .metadata
                            .get("cwd")
                            .map_or_else(|| "-".to_string(), |cwd| truncate_path(cwd, 24)),
                    )
                    .monospace()
                    .color(FG_MUTED),
                );
            });
            ui.separator();

            for event in events {
                ui.label(
                    RichText::new(format!(
                        "[{}] {}",
                        event.component.to_uppercase(),
                        trim_line(&event.payload, 48)
                    ))
                    .monospace()
                    .color(level_color(&event.level)),
                );
            }

            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("click").monospace().color(ACCENT_ALT_2));
                    ui.label(RichText::new("select").monospace().color(FG_MUTED));
                    ui.separator();
                    ui.label(RichText::new("focus").monospace().color(ACCENT_ALT_2));
                    ui.label(RichText::new("inspect").monospace().color(FG_MUTED));
                });
            });
        })
        .response;

    if response.clicked() {
        state.select_visible_index(visible_index);
    }
}

fn render_focus_detail(ui: &mut egui::Ui, agent: &Agent, state: &AppState) {
    Frame::new()
        .fill(BG_PANEL_ALT)
        .stroke(Stroke::new(1.0, ACCENT))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.label(RichText::new(&agent.id).strong().size(22.0));
            ui.label(RichText::new(format!("{} · {}", agent.project, agent.role)).color(FG_MUTED));
            ui.separator();
            ui.monospace(format!("status    {}", agent.status));
            ui.monospace(format!("branch    {}", agent.branch));
            ui.monospace(format!("tokens    {}", agent.tokens));
            ui.monospace(format!(
                "cwd       {}",
                agent.metadata.get("cwd").cloned().unwrap_or_default()
            ));
            ui.monospace(format!(
                "model     {}",
                agent.metadata.get("model").cloned().unwrap_or_default()
            ));
            ui.separator();

            for event in state.get_recent_events(Some(&agent.id), 8) {
                ui.label(
                    RichText::new(format!(
                        "[{}] {}",
                        event.component.to_uppercase(),
                        trim_line(&event.payload, 78)
                    ))
                    .monospace()
                    .color(level_color(&event.level)),
                );
            }
        });
}

fn status_color(agent: &Agent) -> Color32 {
    match agent.status.to_ascii_lowercase().as_str() {
        "busy" => WARNING,
        "offline" => OFFLINE,
        _ => SUCCESS,
    }
}

fn level_color(level: &str) -> Color32 {
    match level.to_ascii_lowercase().as_str() {
        "error" => DANGER,
        "warn" => Color32::YELLOW,
        "success" => SUCCESS,
        _ => ACCENT_ALT,
    }
}

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

fn truncate_path(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let suffix = value
        .chars()
        .rev()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("…{suffix}")
}

fn trendline(agent: &Agent, width: usize) -> String {
    let values = agent.activity.iter().copied().collect::<Vec<_>>();
    let start = values.len().saturating_sub(width);
    let slice = &values[start..];
    let max = slice.iter().copied().max().unwrap_or(0);
    let blocks = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    slice
        .iter()
        .map(|value| {
            if max == 0 {
                '·'
            } else {
                let index = ((*value * (blocks.len() as u64 - 1)) / max) as usize;
                blocks[index]
            }
        })
        .collect()
}

fn grid_columns_for_width(width: f32) -> usize {
    if width > 1600.0 {
        3
    } else if width > 980.0 {
        2
    } else {
        1
    }
}

fn grid_rows() -> usize {
    2
}

fn grid_page_size(width: f32) -> usize {
    grid_columns_for_width(width) * grid_rows()
}

fn spawn_core_runtime(state: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("desktop runtime");

        runtime.block_on(async move {
            let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
            let (agent_tx, mut agent_rx) = tokio::sync::mpsc::channel(64);
            let use_demo = listener::demo_mode_enabled() || std::env::var("VIEW_DEMO").is_ok();

            tokio::spawn(async move {
                let _ = if use_demo {
                    listener::start_demo_listener(event_tx, agent_tx).await
                } else {
                    listener::start_camp_listener(event_tx, agent_tx).await
                };
            });

            let mut tick = time::interval(Duration::from_secs(1));

            loop {
                tokio::select! {
                    Some(event) = event_rx.recv() => {
                        let mut app = state.lock().await;
                        app.add_event(event);
                    }
                    Some(agent) = agent_rx.recv() => {
                        let mut app = state.lock().await;
                        app.update_agent(agent);
                    }
                    _ = tick.tick() => {
                        let mut app = state.lock().await;
                        app.tick_activity();
                    }
                }
            }
        });
    });
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
    use super::{
        grid_columns_for_width, grid_page_size, screenshot_target, trim_line, truncate_path,
    };

    #[test]
    fn grid_columns_should_scale_with_available_width() {
        assert_eq!(grid_columns_for_width(800.0), 1);
        assert_eq!(grid_columns_for_width(1200.0), 2);
        assert_eq!(grid_columns_for_width(1800.0), 3);
        assert_eq!(grid_page_size(1800.0), 6);
    }

    #[test]
    fn string_helpers_should_trim_without_panicking() {
        assert_eq!(trim_line("abcdef", 4), "abc…");
        assert_eq!(truncate_path("/a/very/long/path/file.rs", 10), "…h/file.rs");
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
}
