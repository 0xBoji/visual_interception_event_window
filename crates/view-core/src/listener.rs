use crate::app::{Agent, Event};
use chrono::Local;
use std::collections::{BTreeMap, VecDeque};
use tokio::sync::mpsc;
use tokio::time::{self, Duration};

// Camp logic has been removed.

pub fn demo_mode_enabled() -> bool {
    std::env::var("VIEW_DEMO")
        .map(|value| demo_mode_from_value(&value))
        .unwrap_or(false)
}

fn demo_mode_from_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "demo"
    )
}

// Camp logic has been removed.

fn demo_agents(step: usize) -> Vec<Agent> {
    let statuses = [
        (
            "agentic-coding",
            "busy",
            "orchestrator",
            "workspace",
            "feature/session-grid",
        ),
        (
            "docs-site",
            "idle",
            "worker",
            "workspace",
            "feat/reference-pass",
        ),
        (
            "api-server",
            "busy",
            "planner",
            "workspace",
            "plan/runtime-cleanup",
        ),
        ("mobile-app", "offline", "auditor", "workspace", "main"),
        (
            "infra-terraform",
            "busy",
            "reviewer",
            "workspace",
            "fix/state-drift",
        ),
        (
            "shared-lib",
            "idle",
            "builder",
            "workspace",
            "feat/desktop-preview",
        ),
    ];

    statuses
        .iter()
        .enumerate()
        .map(|(index, (id, status, role, project, branch))| {
            let mut metadata = BTreeMap::new();
            metadata.insert(
                "tokens".to_string(),
                format!("{}", 24_000 + (index as u64 * 13_500) + (step as u64 * 750)),
            );
            metadata.insert("cwd".to_string(), format!("/Users/demo/projects/{id}"));
            metadata.insert(
                "model".to_string(),
                match index {
                    0 => "gpt-5.4".to_string(),
                    1 => "gpt-5.4-mini".to_string(),
                    2 => "gpt-5.3-codex-spark".to_string(),
                    _ => "gpt-5.4-mini".to_string(),
                },
            );
            metadata.insert(
                "last_file".to_string(),
                format!("/Users/demo/projects/{id}/{}", demo_file_name(index, step)),
            );
            metadata.insert(
                "last_tool".to_string(),
                match index {
                    0 => "Edit",
                    1 => "Search",
                    2 => "Plan",
                    3 => "Idle",
                    4 => "Review",
                    _ => "Build",
                }
                .to_string(),
            );
            metadata.insert(
                "messages".to_string(),
                format!("{}", 12 + index * 4 + (step % 3)),
            );
            metadata.insert(
                "cost".to_string(),
                format!("${:.2}", 0.18 + index as f32 * 0.09 + step as f32 * 0.01),
            );

            let activity = (0..50)
                .map(|offset| {
                    let phase = (step + offset + index * 3) % 11;
                    if *status == "offline" {
                        0
                    } else if phase > 7 {
                        4 + index as u64
                    } else if phase > 3 {
                        2 + index as u64
                    } else {
                        (index % 2) as u64
                    }
                })
                .collect::<VecDeque<_>>();

            Agent {
                id: (*id).to_string(),
                instance_name: format!("{id}.rai"),
                role: (*role).to_string(),
                project: (*project).to_string(),
                branch: (*branch).to_string(),
                status: (*status).to_string(),
                capabilities: vec![
                    "observe".to_string(),
                    "stream-json".to_string(),
                    format!("tool-{}", index + 1),
                ],
                port: 4100 + index as u16,
                addresses: vec![format!("127.0.0.1:{}", 4100 + index as u16)],
                metadata,
                last_seen: Local::now(),
                tokens: 24_000 + (index as u64 * 13_500) + (step as u64 * 750),
                activity,
            }
        })
        .collect()
}

fn demo_events(step: usize) -> Vec<Event> {
    let scripts = [
        (
            "agentic-coding",
            "shell",
            [
                ("info", "$ cargo test --workspace"),
                ("success", "Tests completed with 0 failures"),
                ("error", "Retry budget exhausted for release sync"),
            ],
        ),
        (
            "docs-site",
            "shell",
            [
                ("success", "$ pnpm dev"),
                ("info", "Preview server listening on :3000"),
                ("success", "Reference page updated cleanly"),
            ],
        ),
        (
            "api-server",
            "shell",
            [
                ("warn", "$ cargo check"),
                ("warn", "Borrow checker still unhappy in auth flow"),
                ("success", "Handler plan merged into runtime lane"),
            ],
        ),
        (
            "infra-terraform",
            "shell",
            [
                ("warn", "$ terraform plan"),
                ("info", "State drift detected in staging"),
                ("success", "Review checklist completed"),
            ],
        ),
        (
            "shared-lib",
            "shell",
            [
                ("info", "$ cargo doc --open"),
                ("success", "Desktop preview rendered successfully"),
                ("info", "Public API draft ready for review"),
            ],
        ),
    ];

    scripts
        .iter()
        .enumerate()
        .map(|(index, (agent_id, component, script))| {
            let (level, payload) = script[(step + index) % script.len()];
            Event {
                timestamp: Local::now(),
                agent_id: (*agent_id).to_string(),
                kind: "UPDATED".to_string(),
                component: (*component).to_string(),
                level: level.to_string(),
                payload: payload.to_string(),
            }
        })
        .collect()
}

fn demo_file_name(index: usize, step: usize) -> &'static str {
    let files = [
        [
            "src/main.rs",
            "src/session.rs",
            "Cargo.toml",
            "README.md",
            "src/ui.rs",
        ],
        [
            "app/routes/docs.tsx",
            "content/api.md",
            "package.json",
            "README.md",
            "app/layout.tsx",
        ],
        [
            "src/auth.rs",
            "src/server.rs",
            "src/routes.rs",
            "Cargo.toml",
            "src/lib.rs",
        ],
        [
            "infra/main.tf",
            "infra/variables.tf",
            "README.md",
            "envs/staging.tfvars",
            "modules/vpc/main.tf",
        ],
        [
            "src/lib.rs",
            "src/terminal.rs",
            "README.md",
            "src/theme.rs",
            "Cargo.toml",
        ],
    ];
    files[index % files.len()][step % files[0].len()]
}

pub async fn start_demo_listener(
    tx: mpsc::Sender<Event>,
    agent_tx: mpsc::Sender<Agent>,
) -> anyhow::Result<()> {
    let mut tick = time::interval(Duration::from_millis(900));
    for warmup in 0..4 {
        emit_demo_step(&tx, &agent_tx, warmup).await?;
    }
    let mut step = 4usize;

    loop {
        tick.tick().await;
        emit_demo_step(&tx, &agent_tx, step).await?;

        step = step.wrapping_add(1);
    }
}

async fn emit_demo_step(
    tx: &mpsc::Sender<Event>,
    agent_tx: &mpsc::Sender<Agent>,
    step: usize,
) -> anyhow::Result<()> {
    for agent in demo_agents(step) {
        if agent_tx.send(agent).await.is_err() {
            return Ok(());
        }
    }

    for event in demo_events(step) {
        if tx.send(event).await.is_err() {
            return Ok(());
        }
    }

    Ok(())
}

// Camp logic has been removed.

#[cfg(test)]
mod tests {
    use super::{demo_agents, demo_events, demo_mode_from_value};
    use crate::app::Agent;
    use chrono::Local;
    use std::collections::{BTreeMap, VecDeque};

    #[test]
    fn demo_mode_from_value_should_accept_common_truthy_inputs() {
        assert!(demo_mode_from_value("1"));
        assert!(demo_mode_from_value("true"));
        assert!(demo_mode_from_value("YES"));
        assert!(demo_mode_from_value("demo"));
        assert!(!demo_mode_from_value("0"));
        assert!(!demo_mode_from_value("off"));
    }

    #[test]
    fn demo_dataset_should_cover_multiple_agents_and_levels() {
        let agents = demo_agents(2);
        let events = demo_events(3);

        assert_eq!(agents.len(), 6);
        assert!(agents
            .iter()
            .any(|agent| agent.status.eq_ignore_ascii_case("busy")));
        assert!(agents
            .iter()
            .any(|agent| agent.status.eq_ignore_ascii_case("offline")));
        assert!(agents.iter().all(|agent| !agent.activity.is_empty()));
        assert_eq!(events.len(), 5);
        assert!(events.iter().all(|event| event.component == "shell"));
        assert!(events.iter().any(|event| event.level == "success"));
    }
}
