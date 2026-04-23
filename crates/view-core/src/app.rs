use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

/// Maximum number of events to retain in the buffer.
const EVENT_LIMIT: usize = 100;
const TERMINAL_LINE_LIMIT: usize = 400;
/// Default number of terminal sessions when using `AppState::new()`.
const DEFAULT_TERMINAL_SESSION_COUNT: usize = 1;
/// Hard cap on concurrent terminal sessions (prevents unbounded memory growth).
pub const MAX_TERMINAL_SESSIONS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub instance_name: String,
    pub role: String,
    pub project: String,
    pub branch: String,
    pub status: String,
    pub capabilities: Vec<String>,
    pub port: u16,
    pub addresses: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub last_seen: chrono::DateTime<chrono::Local>,
    pub tokens: u64,
    pub activity: VecDeque<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub agent_id: String,
    pub kind: String,
    pub component: String,
    pub level: String,
    pub payload: String,
}

// Camp data structures (EventRecord, SnapshotRecord) removed.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AgentStatusSummary {
    pub total: usize,
    pub online: usize,
    pub busy: usize,
    pub offline: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EventLevelSummary {
    pub info: usize,
    pub warn: usize,
    pub error: usize,
    pub success: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentFilterMode {
    #[default]
    All,
    Busy,
    Active,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Grid,
    Focus,
}

#[derive(Clone)]
pub struct TerminalState {
    pub title: String,
    pub cwd: String,
    pub status: String,
    pub lines: VecDeque<String>,
    pub history: VecDeque<String>,
    pub pending_context_line: Option<String>,
}

/// Serializable projection of a terminal session for web clients.
#[derive(Debug, Clone, Serialize)]
pub struct TerminalSnapshot {
    pub id: usize,
    pub title: String,
    pub cwd: String,
    pub status: String,
    /// Last N lines of output (newest at the end).
    pub recent_lines: Vec<String>,
}

/// Full serializable snapshot of VIEW state — broadcast over WebSocket.
#[derive(Debug, Clone, Serialize)]
pub struct WebSnapshot {
    pub agents: Vec<Agent>,
    pub events: Vec<Event>,
    pub terminals: Vec<TerminalSnapshot>,
    pub total_events_received: u64,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self {
            title: "shell-1".to_string(),
            cwd: String::new(),
            status: "starting".to_string(),
            lines: VecDeque::new(),
            history: VecDeque::new(),
            pending_context_line: None,
        }
    }
}

pub struct AppState {
    pub agents: BTreeMap<String, Agent>,
    pub events: VecDeque<Event>,
    pub selected_agent_idx: usize,
    pub should_quit: bool,
    pub total_events_received: u64,
    pub filter_mode: AgentFilterMode,
    pub search_query: String,
    pub search_mode: bool,
    pub view_mode: ViewMode,
    pub terminal_sessions: Vec<TerminalState>,
    pub selected_terminal_idx: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create a new `AppState` with a single terminal session (default).
    pub fn new() -> Self {
        Self::new_with_sessions(DEFAULT_TERMINAL_SESSION_COUNT)
    }

    /// Create a new `AppState` with `count` terminal sessions pre-initialized.
    /// `count` is clamped to `[1, MAX_TERMINAL_SESSIONS]`.
    pub fn new_with_sessions(count: usize) -> Self {
        let count = count.max(1).min(MAX_TERMINAL_SESSIONS);
        Self {
            agents: BTreeMap::new(),
            events: VecDeque::with_capacity(EVENT_LIMIT),
            selected_agent_idx: 0,
            should_quit: false,
            total_events_received: 0,
            filter_mode: AgentFilterMode::All,
            search_query: String::new(),
            search_mode: false,
            view_mode: ViewMode::Grid,
            terminal_sessions: (0..count)
                .map(|index| TerminalState {
                    title: format!("shell-{}", index + 1),
                    ..TerminalState::default()
                })
                .collect(),
            selected_terminal_idx: 0,
        }
    }

    /// Append a new terminal session with the given title.
    /// Returns the new session's index, or `None` if the cap is reached.
    pub fn add_terminal_session(&mut self, title: impl Into<String>) -> Option<usize> {
        if self.terminal_sessions.len() >= MAX_TERMINAL_SESSIONS {
            return None;
        }
        let index = self.terminal_sessions.len();
        self.terminal_sessions.push(TerminalState {
            title: title.into(),
            ..TerminalState::default()
        });
        Some(index)
    }

    /// Remove the terminal session at `index`, selecting the nearest remaining
    /// session afterwards. Returns `false` if the index is out of range or
    /// it is the last session (minimum 1 must always exist).
    pub fn remove_terminal_session(&mut self, index: usize) -> bool {
        if self.terminal_sessions.len() <= 1 || index >= self.terminal_sessions.len() {
            return false;
        }
        self.terminal_sessions.remove(index);
        if self.selected_terminal_idx >= self.terminal_sessions.len() {
            self.selected_terminal_idx = self.terminal_sessions.len() - 1;
        }
        true
    }

    /// Build a fully serializable snapshot of current state for web clients.
    /// Terminal lines are capped at 50 most recent; events at 20 most recent.
    pub fn web_snapshot(&self) -> WebSnapshot {
        WebSnapshot {
            agents: self.agents.values().cloned().collect(),
            events: self.events.iter().take(20).cloned().collect(),
            terminals: self
                .terminal_sessions
                .iter()
                .enumerate()
                .map(|(id, session)| {
                    let len = session.lines.len();
                    let recent_lines = session
                        .lines
                        .iter()
                        .skip(len.saturating_sub(50))
                        .cloned()
                        .collect();
                    TerminalSnapshot {
                        id,
                        title: session.title.clone(),
                        cwd: session.cwd.clone(),
                        status: session.status.clone(),
                        recent_lines,
                    }
                })
                .collect(),
            total_events_received: self.total_events_received,
            timestamp: chrono::Local::now(),
        }
    }

    pub fn add_event(&mut self, event: Event) {
        self.total_events_received += 1;
        if let Some(agent) = self.agents.get_mut(&event.agent_id) {
            if let Some(last) = agent.activity.back_mut() {
                *last += 1;
            }
        }
        if self.events.len() >= EVENT_LIMIT {
            self.events.pop_back();
        }
        self.events.push_front(event);
    }

    pub fn update_agent(&mut self, mut agent: Agent) {
        // Extract and parse tokens from metadata if present
        if let Some(tokens_str) = agent.metadata.get("tokens") {
            if let Ok(tokens) = tokens_str.replace(",", "").parse::<u64>() {
                agent.tokens = tokens;
            }
        }

        if let Some(existing) = self.agents.get(&agent.id) {
            agent.activity = existing.activity.clone();
            // Preserve tokens if the incoming one is zero but we had one before
            if agent.tokens == 0 && existing.tokens > 0 {
                agent.tokens = existing.tokens;
            }
        } else if agent.activity.is_empty() {
            // Initialize empty activity buffer for new agent (50 points)
            agent.activity = VecDeque::from(vec![0; 50]);
        }
        agent.last_seen = chrono::Local::now();
        self.agents.insert(agent.id.clone(), agent);
        self.clamp_selection();
    }

    pub fn get_recent_events(&self, agent_id: Option<&str>, limit: usize) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|event| agent_id.is_none_or(|id| event.agent_id == id))
            .take(limit)
            .collect()
    }

    pub fn get_selected_agent(&self) -> Option<&Agent> {
        self.get_selected_agent_id()
            .and_then(|id| self.agents.get(&id))
    }

    pub fn visible_agent_ids(&self) -> Vec<String> {
        self.agents
            .iter()
            .filter(|(_, agent)| self.matches_filter(agent) && self.matches_search(agent))
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn visible_agent_count(&self) -> usize {
        self.visible_agent_ids().len()
    }

    pub fn visible_agents_page(&self, page_size: usize) -> Vec<String> {
        if page_size == 0 {
            return Vec::new();
        }

        let ids = self.visible_agent_ids();
        let page = self.current_grid_page(page_size);
        let start = page * page_size;
        ids.into_iter().skip(start).take(page_size).collect()
    }

    pub fn current_grid_page(&self, page_size: usize) -> usize {
        if page_size == 0 {
            0
        } else {
            self.selected_agent_idx / page_size
        }
    }

    pub fn grid_page_count(&self, page_size: usize) -> usize {
        let total = self.visible_agent_count();
        if total == 0 || page_size == 0 {
            1
        } else {
            total.div_ceil(page_size)
        }
    }

    pub fn filter_label(&self) -> &'static str {
        match self.filter_mode {
            AgentFilterMode::All => "all",
            AgentFilterMode::Busy => "busy",
            AgentFilterMode::Active => "active",
            AgentFilterMode::Offline => "offline",
        }
    }

    pub fn cycle_filter_mode(&mut self) {
        self.filter_mode = match self.filter_mode {
            AgentFilterMode::All => AgentFilterMode::Busy,
            AgentFilterMode::Busy => AgentFilterMode::Active,
            AgentFilterMode::Active => AgentFilterMode::Offline,
            AgentFilterMode::Offline => AgentFilterMode::All,
        };
        self.clamp_selection();
    }

    pub fn begin_search(&mut self) {
        self.search_mode = true;
    }

    pub fn end_search(&mut self) {
        self.search_mode = false;
    }

    pub fn clear_search_query(&mut self) {
        self.search_query.clear();
        self.clamp_selection();
    }

    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.clamp_selection();
    }

    pub fn append_terminal_line(&mut self, session_id: usize, line: impl Into<String>) {
        let Some(session) = self.terminal_sessions.get_mut(session_id) else {
            return;
        };
        if session.lines.len() >= TERMINAL_LINE_LIMIT {
            session.lines.pop_front();
        }
        session.lines.push_back(line.into());
    }

    pub fn clear_terminal_lines(&mut self, session_id: usize) {
        if let Some(session) = self.terminal_sessions.get_mut(session_id) {
            session.lines.clear();
            session.pending_context_line = None;
        }
    }

    pub fn append_terminal_context_line(&mut self, session_id: usize, line: String) {
        self.append_terminal_line(session_id, line.clone());
        if let Some(session) = self.terminal_sessions.get_mut(session_id) {
            session.pending_context_line = Some(line);
        }
    }

    pub fn finalize_terminal_context_line(&mut self, session_id: usize, seconds: f64) {
        let Some(session) = self.terminal_sessions.get_mut(session_id) else {
            return;
        };
        let Some(pending_line) = session.pending_context_line.take() else {
            return;
        };

        if let Some(line) = session
            .lines
            .iter_mut()
            .rev()
            .find(|line| **line == pending_line)
        {
            *line = format!("{pending_line} ({seconds:.4}s)");
        }
    }

    pub fn set_terminal_status(&mut self, session_id: usize, status: impl Into<String>) {
        if let Some(session) = self.terminal_sessions.get_mut(session_id) {
            session.status = status.into();
        }
    }

    pub fn set_terminal_cwd(&mut self, session_id: usize, cwd: impl Into<String>) {
        if let Some(session) = self.terminal_sessions.get_mut(session_id) {
            session.cwd = cwd.into();
        }
    }

    pub fn recent_terminal_lines(&self, session_id: usize, limit: usize) -> Vec<&str> {
        let Some(session) = self.terminal_sessions.get(session_id) else {
            return Vec::new();
        };
        let len = session.lines.len();
        session
            .lines
            .iter()
            .skip(len.saturating_sub(limit))
            .map(String::as_str)
            .collect()
    }

    pub fn append_terminal_history(&mut self, session_id: usize, command: String) {
        let Some(session) = self.terminal_sessions.get_mut(session_id) else {
            return;
        };
        // Remove if exists to move to back (most recent)
        session.history.retain(|c| c != &command);
        if session.history.len() >= 50 {
            session.history.pop_front();
        }
        session.history.push_back(command);
    }

    pub fn get_terminal_suggestion(&self, session_id: usize, input: &str) -> Option<String> {
        if input.is_empty() {
            return None;
        }
        let session = self.terminal_sessions.get(session_id)?;
        session
            .history
            .iter()
            .rev()
            .find(|cmd| cmd.starts_with(input))
            .cloned()
    }

    pub fn terminal_sessions(&self) -> &[TerminalState] {
        &self.terminal_sessions
    }

    pub fn selected_terminal(&self) -> Option<&TerminalState> {
        self.terminal_sessions.get(self.selected_terminal_idx)
    }

    pub fn select_terminal_index(&mut self, index: usize) {
        if self.terminal_sessions.is_empty() {
            self.selected_terminal_idx = 0;
        } else {
            self.selected_terminal_idx = index.min(self.terminal_sessions.len() - 1);
            self.view_mode = ViewMode::Focus;
        }
    }

    pub fn select_visible_index(&mut self, index: usize) {
        self.selected_agent_idx = index;
        self.clamp_selection();
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Grid => ViewMode::Focus,
            ViewMode::Focus => ViewMode::Grid,
        };
    }

    pub fn append_search_char(&mut self, ch: char) {
        self.search_query.push(ch);
        self.clamp_selection();
    }

    pub fn pop_search_char(&mut self) {
        self.search_query.pop();
        self.clamp_selection();
    }

    pub fn get_agent_status_summary(&self) -> AgentStatusSummary {
        let mut summary = AgentStatusSummary {
            total: self.agents.len(),
            ..AgentStatusSummary::default()
        };

        for agent in self.agents.values() {
            match agent.status.to_ascii_lowercase().as_str() {
                "busy" => {
                    summary.busy += 1;
                    summary.online += 1;
                }
                "offline" => summary.offline += 1,
                _ => summary.online += 1,
            }
        }

        summary
    }

    pub fn get_event_level_summary(&self) -> EventLevelSummary {
        let mut summary = EventLevelSummary::default();

        for event in &self.events {
            match event.level.to_ascii_lowercase().as_str() {
                "warn" => summary.warn += 1,
                "error" => summary.error += 1,
                "success" => summary.success += 1,
                _ => summary.info += 1,
            }
        }

        summary
    }

    /// Ticks the activity buffers, shifting them to the left.
    /// Should be called on a fixed interval (e.g. 1s).
    pub fn tick_activity(&mut self) {
        for agent in self.agents.values_mut() {
            if agent.activity.len() >= 50 {
                agent.activity.pop_front();
            }
            agent.activity.push_back(0);
        }
    }

    pub fn select_next(&mut self) {
        let count = self.visible_agent_count();
        if count == 0 {
            self.selected_agent_idx = 0;
            return;
        }
        self.selected_agent_idx = (self.selected_agent_idx + 1) % count;
    }

    pub fn select_previous(&mut self) {
        let count = self.visible_agent_count();
        if count == 0 {
            self.selected_agent_idx = 0;
            return;
        }
        if self.selected_agent_idx == 0 {
            self.selected_agent_idx = count - 1;
        } else {
            self.selected_agent_idx -= 1;
        }
    }

    pub fn select_first(&mut self) {
        self.selected_agent_idx = 0;
    }

    pub fn select_last(&mut self) {
        let count = self.visible_agent_count();
        if count != 0 {
            self.selected_agent_idx = count - 1;
        }
    }

    pub fn select_next_page(&mut self) {
        let count = self.visible_agent_count();
        if count != 0 {
            self.selected_agent_idx = (self.selected_agent_idx + 5).min(count - 1);
        }
    }

    pub fn select_previous_page(&mut self) {
        if self.visible_agent_count() != 0 {
            self.selected_agent_idx = self.selected_agent_idx.saturating_sub(5);
        }
    }

    pub fn get_selected_agent_id(&self) -> Option<String> {
        self.visible_agent_ids()
            .get(self.selected_agent_idx)
            .cloned()
    }

    fn matches_filter(&self, agent: &Agent) -> bool {
        match self.filter_mode {
            AgentFilterMode::All => true,
            AgentFilterMode::Busy => agent.status.eq_ignore_ascii_case("busy"),
            AgentFilterMode::Active => !agent.status.eq_ignore_ascii_case("offline"),
            AgentFilterMode::Offline => agent.status.eq_ignore_ascii_case("offline"),
        }
    }

    fn matches_search(&self, agent: &Agent) -> bool {
        if self.search_query.trim().is_empty() {
            return true;
        }

        let query = self.search_query.to_ascii_lowercase();
        let haystacks = [
            agent.id.as_str(),
            agent.project.as_str(),
            agent.role.as_str(),
            agent.branch.as_str(),
            agent.instance_name.as_str(),
        ];

        haystacks
            .iter()
            .any(|candidate| candidate.to_ascii_lowercase().contains(&query))
    }

    fn clamp_selection(&mut self) {
        let count = self.visible_agent_count();
        if count == 0 {
            self.selected_agent_idx = 0;
        } else if self.selected_agent_idx >= count {
            self.selected_agent_idx = count - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, AppState, Event, ViewMode};
    use chrono::Local;
    use std::collections::{BTreeMap, VecDeque};

    fn test_agent(id: &str, status: &str) -> Agent {
        Agent {
            id: id.to_string(),
            instance_name: id.to_string(),
            role: "executor".to_string(),
            project: "view".to_string(),
            branch: "main".to_string(),
            status: status.to_string(),
            capabilities: vec!["observe".to_string()],
            port: 0,
            addresses: Vec::new(),
            metadata: BTreeMap::new(),
            last_seen: Local::now(),
            tokens: 0,
            activity: VecDeque::from(vec![0; 50]),
        }
    }

    fn test_event(agent_id: &str, level: &str, payload: &str) -> Event {
        Event {
            timestamp: Local::now(),
            agent_id: agent_id.to_string(),
            kind: "UPDATED".to_string(),
            component: "tick".to_string(),
            level: level.to_string(),
            payload: payload.to_string(),
        }
    }

    #[test]
    fn summaries_should_track_agent_health_and_event_levels() {
        let mut app = AppState::new();
        app.update_agent(test_agent("alpha", "idle"));
        app.update_agent(test_agent("beta", "busy"));
        app.update_agent(test_agent("gamma", "Offline"));
        app.add_event(test_event("alpha", "info", "hello"));
        app.add_event(test_event("beta", "warn", "queue"));
        app.add_event(test_event("beta", "error", "failed"));
        app.add_event(test_event("alpha", "success", "done"));

        let agent_summary = app.get_agent_status_summary();
        let event_summary = app.get_event_level_summary();

        assert_eq!(agent_summary.total, 3);
        assert_eq!(agent_summary.online, 2);
        assert_eq!(agent_summary.busy, 1);
        assert_eq!(agent_summary.offline, 1);
        assert_eq!(event_summary.info, 1);
        assert_eq!(event_summary.warn, 1);
        assert_eq!(event_summary.error, 1);
        assert_eq!(event_summary.success, 1);
    }

    #[test]
    fn get_recent_events_should_support_global_and_agent_scoped_feeds() {
        let mut app = AppState::new();
        app.add_event(test_event("alpha", "info", "oldest"));
        app.add_event(test_event("beta", "warn", "middle"));
        app.add_event(test_event("alpha", "error", "newest"));

        let global_feed = app.get_recent_events(None, 2);
        let agent_feed = app.get_recent_events(Some("alpha"), 5);

        assert_eq!(global_feed.len(), 2);
        assert_eq!(global_feed[0].payload, "newest");
        assert_eq!(global_feed[1].payload, "middle");
        assert_eq!(agent_feed.len(), 2);
        assert_eq!(agent_feed[0].payload, "newest");
        assert_eq!(agent_feed[1].payload, "oldest");
    }

    #[test]
    fn visible_agent_ids_should_follow_filter_and_search_query() {
        let mut app = AppState::new();
        let mut alpha = test_agent("alpha-tick", "busy");
        alpha.project = "tick".to_string();
        let mut beta = test_agent("beta-wasp", "idle");
        beta.project = "wasp".to_string();
        let mut gamma = test_agent("gamma-camp", "offline");
        gamma.project = "camp".to_string();
        app.update_agent(alpha);
        app.update_agent(beta);
        app.update_agent(gamma);

        app.cycle_filter_mode();
        assert_eq!(app.visible_agent_ids(), vec!["alpha-tick".to_string()]);

        app.cycle_filter_mode();
        for ch in "wasp".chars() {
            app.append_search_char(ch);
        }
        assert_eq!(app.visible_agent_ids(), vec!["beta-wasp".to_string()]);
    }

    #[test]
    fn selection_should_wrap_within_visible_agents_only() {
        let mut app = AppState::new();
        app.update_agent(test_agent("alpha", "busy"));
        app.update_agent(test_agent("beta", "offline"));
        app.update_agent(test_agent("gamma", "busy"));

        app.cycle_filter_mode();
        assert_eq!(app.get_selected_agent_id(), Some("alpha".to_string()));

        app.select_next();
        assert_eq!(app.get_selected_agent_id(), Some("gamma".to_string()));

        app.select_next();
        assert_eq!(app.get_selected_agent_id(), Some("alpha".to_string()));
    }

    #[test]
    fn set_search_query_and_select_visible_index_should_clamp_to_visible_range() {
        let mut app = AppState::new();
        app.update_agent(test_agent("alpha", "busy"));
        app.update_agent(test_agent("beta", "busy"));
        app.update_agent(test_agent("gamma", "busy"));

        app.set_search_query("beta");
        app.select_visible_index(3);

        assert_eq!(app.visible_agent_ids(), vec!["beta".to_string()]);
        assert_eq!(app.get_selected_agent_id(), Some("beta".to_string()));
    }

    #[test]
    fn terminal_state_should_keep_recent_lines_only() {
        let mut app = AppState::new();
        app.set_terminal_status(0, "ready");
        app.set_terminal_cwd(0, "/tmp/view-shell");
        for index in 0..405 {
            app.append_terminal_line(0, format!("line-{index}"));
        }

        assert_eq!(app.terminal_sessions[0].status, "ready");
        assert_eq!(app.terminal_sessions[0].cwd, "/tmp/view-shell");
        assert_eq!(app.terminal_sessions[0].lines.len(), 400);
        assert_eq!(
            app.recent_terminal_lines(0, 2),
            vec!["line-403", "line-404"]
        );
    }

    #[test]
    fn clear_terminal_lines_should_drop_existing_transcript() {
        let mut app = AppState::new();
        app.append_terminal_line(0, "$ ls");
        app.append_terminal_line(0, "Cargo.toml");

        app.clear_terminal_lines(0);

        assert!(app.recent_terminal_lines(0, 10).is_empty());
    }

    #[test]
    fn finalize_terminal_context_line_should_update_pending_context_with_timing() {
        let mut app = AppState::new();
        app.append_terminal_context_line(0, "/tmp/project git:(main)".to_string());
        app.append_terminal_line(0, "$ ls");

        app.finalize_terminal_context_line(0, 0.042);

        assert_eq!(
            app.recent_terminal_lines(0, 2),
            vec!["/tmp/project git:(main) (0.0420s)", "$ ls"]
        );
    }

    #[test]
    fn select_terminal_index_should_clamp_and_switch_focus_mode() {
        let mut app = AppState::new();
        app.select_terminal_index(99);
        assert_eq!(app.selected_terminal_idx, 0);
        assert_eq!(app.view_mode, ViewMode::Focus);
    }

    #[test]
    fn grid_paging_should_follow_selection_and_page_size() {
        let mut app = AppState::new();
        for id in ["alpha", "beta", "gamma", "delta", "epsilon"] {
            app.update_agent(test_agent(id, "busy"));
        }

        assert_eq!(
            app.visible_agents_page(4),
            vec![
                "alpha".to_string(),
                "beta".to_string(),
                "delta".to_string(),
                "epsilon".to_string()
            ]
        );
        assert_eq!(app.grid_page_count(4), 2);

        app.select_last();
        assert_eq!(app.current_grid_page(4), 1);
        assert_eq!(app.visible_agents_page(4), vec!["gamma".to_string()]);
    }

    #[test]
    fn toggle_view_mode_should_switch_between_grid_and_focus() {
        let mut app = AppState::new();

        assert_eq!(app.view_mode, ViewMode::Grid);
        app.toggle_view_mode();
        assert_eq!(app.view_mode, ViewMode::Focus);
        app.toggle_view_mode();
        assert_eq!(app.view_mode, ViewMode::Grid);
    }
}
