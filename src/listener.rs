use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use rand::RngExt;
use chrono::Local;

use crate::app::{Agent, AgentStatus, Event};

/// This simulates external agent events.
/// 
/// Later, this can be swapped with `camp watch --json` stdout piping.
pub async fn start_simulated_listener(tx: mpsc::Sender<Event>, agent_tx: mpsc::Sender<Agent>) {
    let agent_ids = vec!["agent-alpha", "agent-beta", "agent-gamma"];
    let roles = vec!["coder", "reviewer", "planner"];
    
    // Initial agent registration
    for (i, id) in agent_ids.iter().enumerate() {
        let agent = Agent {
            id: id.to_string(),
            role: roles[i % roles.len()].to_string(),
            status: AgentStatus::Idle,
            git_locked: false,
            last_seen: Local::now(),
            tokens: 0,
            branch: "main".to_string(),
            activity: std::collections::VecDeque::new(),
        };
        let _ = agent_tx.send(agent).await;
    }

    loop {
        let delay = rand::random_range(1..=4);
        sleep(Duration::from_secs(delay)).await;

        let agent_idx = rand::random_range(0..agent_ids.len());
        let agent_id = agent_ids[agent_idx];
        
        let event_type = rand::random_range(0..3);
        let branch_name = format!("feat/RAI-{}", rand::random_range(100..999));
        let (kind, payload, new_status, new_git_lock) = match event_type {
            0 => ("Joined", "Agent joined the mesh network.", AgentStatus::Idle, false),
            1 => ("Updated", format!("Working on branch '{}'.", branch_name), AgentStatus::Busy, true),
            2 => ("TaskExecuted", "Ran unit tests in wasp sandbox.", AgentStatus::Idle, false),
            _ => unreachable!(),
        };

        let event = Event {
            timestamp: Local::now(),
            agent_id: agent_id.to_string(),
            kind: kind.to_string(),
            payload: payload.to_string(),
        };

        let _ = tx.send(event).await;

        // Also update agent state periodically
        let updated_agent = Agent {
            id: agent_id.to_string(),
            role: roles[agent_idx % roles.len()].to_string(),
            status: new_status,
            git_locked: new_git_lock,
            last_seen: Local::now(),
            tokens: rand::random_range(500..5000), // Simulate token usage
            branch: branch_name,
            activity: std::collections::VecDeque::new(),
        };
        let _ = agent_tx.send(updated_agent).await;
    }
}
