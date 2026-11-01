use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use easymc_core::{EasyMcError, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Clone, serde::Serialize)]
pub struct LaunchStatus {
    pub running: bool,
    pub message: String,
    pub pid: Option<u32>,
}

/// A game session that just ended (natural exit or stop).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FinishedSession {
    pub instance_id: String,
    pub play_secs: u64,
}

struct Running {
    child: Child,
    instance_id: String,
    started: Instant,
}

pub struct ProcessManager {
    inner: Mutex<Option<Running>>,
    logs: RwLock<Vec<String>>,
    finished: Mutex<Vec<FinishedSession>>,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            logs: RwLock::new(Vec::new()),
            finished: Mutex::new(Vec::new()),
        }
    }

    pub async fn clear_logs(&self) {
        self.logs.write().await.clear();
    }

    pub async fn push_log(&self, line: String) {
        let mut logs = self.logs.write().await;
        logs.push(line);
        if logs.len() > 5000 {
            let drain = logs.len() - 4000;
            logs.drain(0..drain);
        }
    }

    pub async fn logs_snapshot(&self) -> Vec<String> {
        self.logs.read().await.clone()
    }

    /// Drain finished sessions so the UI / store can accumulate playtime.
    pub async fn take_finished(&self) -> Vec<FinishedSession> {
        std::mem::take(&mut *self.finished.lock().await)
    }

    async fn record_finished(&self, instance_id: String, started: Instant) {
        let play_secs = started.elapsed().as_secs();
        self.finished.lock().await.push(FinishedSession {
            instance_id,
            play_secs,
        });
    }

    pub async fn is_running(&self) -> bool {
        let mut guard = self.inner.lock().await;
        if let Some(r) = guard.as_mut() {
            match r.child.try_wait() {
                Ok(Some(status)) => {
                    let msg = format!("进程已退出: {status}");
                    let id = r.instance_id.clone();
                    let started = r.started;
                    *guard = None;
                    drop(guard);
                    self.record_finished(id, started).await;
                    self.push_log(msg).await;
                    false
                }
                Ok(None) => true,
                Err(e) => {
                    let id = guard.as_ref().map(|r| r.instance_id.clone());
                    let started = guard.as_ref().map(|r| r.started);
                    *guard = None;
                    drop(guard);
                    if let (Some(id), Some(started)) = (id, started) {
                        self.record_finished(id, started).await;
                    }
                    self.push_log(format!("wait error: {e}")).await;
                    false
                }
            }
        } else {
            false
        }
    }

    pub async fn stop(&self) -> Result<()> {
        let mut guard = self.inner.lock().await;
        if let Some(mut r) = guard.take() {
            let id = r.instance_id.clone();
            let started = r.started;
            let _ = r.child.kill().await;
            drop(guard);
            self.record_finished(id.clone(), started).await;
            self.push_log(format!("已停止实例 {id}")).await;
        }
        Ok(())
    }

    pub async fn spawn(
        self: &Arc<Self>,
        instance_id: String,
        java: PathBuf,
        args: Vec<String>,
        cwd: &Path,
        env: HashMap<String, String>,
        on_line: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<LaunchStatus> {
        {
            let guard = self.inner.lock().await;
            if guard.is_some() {
                return Err(EasyMcError::msg("已有实例在运行，请先停止"));
            }
        }

        self.clear_logs().await;
        let cmdline = format!("{} {}", java.display(), args.join(" "));
        self.push_log(format!("$ {cmdline}")).await;
        on_line(format!("$ {cmdline}"));

        let mut cmd = Command::new(&java);
        cmd.args(&args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| EasyMcError::msg(format!("spawn java: {e}")))?;
        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        {
            let mut guard = self.inner.lock().await;
            *guard = Some(Running {
                child,
                instance_id: instance_id.clone(),
                started: Instant::now(),
            });
        }

        let on_out = Arc::new(on_line);
        if let Some(out) = stdout {
            let mgr = Arc::clone(self);
            let on_line = Arc::clone(&on_out);
            tokio::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    mgr.push_log(line.clone()).await;
                    on_line(line);
                }
            });
        }
        if let Some(err) = stderr {
            let mgr = Arc::clone(self);
            let on_line = Arc::clone(&on_out);
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    mgr.push_log(line.clone()).await;
                    on_line(line);
                }
            });
        }

        let mgr_reap = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if !mgr_reap.is_running().await {
                    break;
                }
            }
        });

        Ok(LaunchStatus {
            running: true,
            message: format!("已启动 {instance_id} (pid={pid:?})"),
            pid,
        })
    }
}

#[cfg(test)]
mod finished_tests {
    use super::*;

    #[tokio::test]
    async fn take_finished_drains() {
        let mgr = ProcessManager::new();
        mgr.record_finished("a".into(), Instant::now() - std::time::Duration::from_secs(5))
            .await;
        let v = mgr.take_finished().await;
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].instance_id, "a");
        assert!(v[0].play_secs >= 5);
        assert!(mgr.take_finished().await.is_empty());
    }
}
