use anyhow::Result;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Line(String),
    Status(String),
    Cwd(String),
}

pub type TerminalCommandTx = mpsc::UnboundedSender<String>;

pub fn local_shell_command_tx() -> (TerminalCommandTx, mpsc::UnboundedReceiver<String>) {
    mpsc::unbounded_channel()
}

pub async fn start_local_shell(
    cwd: PathBuf,
    event_tx: mpsc::UnboundedSender<TerminalEvent>,
    mut command_rx: mpsc::UnboundedReceiver<String>,
) -> Result<()> {
    let shell_home = PathBuf::from("/tmp/view-shell");
    let _ = tokio::fs::create_dir_all(&shell_home).await;

    let mut child = Command::new("/usr/bin/script")
        .arg("-q")
        .arg("/dev/null")
        .arg("/bin/zsh")
        .arg("-i")
        .current_dir(&cwd)
        .env("HOME", &shell_home)
        .env("ZDOTDIR", &shell_home)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture shell stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture shell stdout"))?;

    let _ = event_tx.send(TerminalEvent::Status("running".to_string()));
    let _ = event_tx.send(TerminalEvent::Cwd(cwd.display().to_string()));

    let startup_commands = [
        "printf 'VIEW shell ready\\n'",
        "pwd",
        "printf 'Type commands in desktop focus mode and press Enter.\\n'",
    ];

    for command in startup_commands {
        stdin.write_all(command.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
    }
    stdin.flush().await?;

    let reader_task = {
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let cleaned = line
                    .replace('\u{8}', "")
                    .replace('\u{1b}', "")
                    .trim()
                    .to_string();
                if !cleaned.is_empty() {
                    let _ = event_tx.send(TerminalEvent::Line(cleaned));
                }
            }
        })
    };

    while let Some(command) = command_rx.recv().await {
        stdin.write_all(command.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
    }

    let _ = reader_task.await;
    let _ = event_tx.send(TerminalEvent::Status("closed".to_string()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::local_shell_command_tx;

    #[test]
    fn local_shell_command_channel_should_send_commands() {
        let (tx, mut rx) = local_shell_command_tx();
        tx.send("echo test".to_string()).expect("send");
        assert_eq!(rx.try_recv().expect("recv"), "echo test".to_string());
    }
}
