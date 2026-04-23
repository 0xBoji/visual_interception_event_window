use crate::app::{Agent, AppState, Event};
use crate::listener;
use crate::terminal::{self, TerminalCommandTx, TerminalEvent};
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};

/// Actions that UI surfaces can send to the CoreEngine
#[derive(Debug, Clone)]
pub enum Action {
    /// Request the engine to spawn a new terminal process
    SpawnTerminal { cwd: PathBuf },
    /// Send a command string to a specific terminal session
    SubmitCommand { session_id: usize, command: String },
}

/// The centralized background engine that drives VIEW.
/// It manages the event loop, background listeners, and terminal PTYs.
pub struct CoreEngine {
    pub state: Arc<RwLock<AppState>>,
    pub action_tx: mpsc::UnboundedSender<Action>,
}

impl CoreEngine {
    /// Start the engine tasks. Must be called from inside a tokio runtime context.
    pub fn spawn_background(state: Arc<RwLock<AppState>>) -> mpsc::UnboundedSender<Action> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
        
        let (event_tx, mut event_rx) = mpsc::channel::<Event>(64);
        let (agent_tx, mut agent_rx) = mpsc::channel::<Agent>(64);
        let (terminal_event_tx, mut terminal_event_rx) = mpsc::unbounded_channel::<TerminalEvent>();

        // 1. Start the Demo Listener
        tokio::spawn(async move {
            let _ = listener::start_demo_listener(event_tx, agent_tx).await;
        });

        // 2. The Main Event God Loop
        let state_clone = state.clone();
        tokio::spawn(async move {
            let mut tick = time::interval(Duration::from_secs(1));
            // Keep track of shell transmitters internal to the engine
            let mut shell_txs: Vec<TerminalCommandTx> = Vec::new();

            loop {
                tokio::select! {
                    // --- UI ACTIONS ---
                    Some(action) = action_rx.recv() => {
                        match action {
                            Action::SpawnTerminal { cwd } => {
                                let (tx, rx) = terminal::local_shell_command_tx();
                                let session_id = shell_txs.len();
                                shell_txs.push(tx);
                                
                                let term_event_tx = terminal_event_tx.clone();
                                tokio::spawn(async move {
                                    let _ = terminal::start_local_shell(session_id, cwd, term_event_tx, rx).await;
                                });
                            }
                            Action::SubmitCommand { session_id, command } => {
                                if let Some(tx) = shell_txs.get(session_id) {
                                    let _ = tx.send(command);
                                }
                            }
                        }
                    }

                    // --- MESH EVENTS ---
                    Some(event) = event_rx.recv() => {
                        let mut app = state_clone.write();
                        app.add_event(event);
                    }
                    Some(agent) = agent_rx.recv() => {
                        let mut app = state_clone.write();
                        app.update_agent(agent);
                    }

                    // --- TERMINAL I/O ---
                    Some(terminal_event) = terminal_event_rx.recv() => {
                        let mut app = state_clone.write();
                        match terminal_event {
                            TerminalEvent::Line { session_id, line } => app.append_terminal_line(session_id, line),
                            TerminalEvent::Status { session_id, status } => app.set_terminal_status(session_id, status),
                            TerminalEvent::Cwd { session_id, cwd } => app.set_terminal_cwd(session_id, cwd),
                            TerminalEvent::Timing { session_id, seconds } => app.finalize_terminal_context_line(session_id, seconds),
                        }
                    }

                    // --- PERIODIC TICK ---
                    _ = tick.tick() => {
                        let mut app = state_clone.write();
                        app.tick_activity();
                    }
                }
            }
        });

        action_tx
    }
}
