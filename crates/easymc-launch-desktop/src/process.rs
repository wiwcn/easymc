use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

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

struct Running {
    child: Child,
    instance_id: String,
}

pub struct ProcessManager {
    inner: Mutex<Option<Running>>,
    logs: RwLock<Vec<String>>,
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

    pub async fn is_running(&self) -> bool {
        let mut guard = self.inner.lock().await;
        if let Some(r) = guard.as_mut() {
            match r.child.try_wait() {
                Ok(Some(status)) => {
                    let msg = format!("进程已退出: {status}");
                    *guard = None;
                    drop(guard);
                    self.push_log(msg).await;
                    false
                }
                Ok(None) => true,
                Err(e) => {
                    drop(guard);
                    self.push_log(format!("wait error: {e}")).await;
                    let mut guard = self.inner.lock().await;
                    *guard = None;
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
            let _ = r.child.kill().await;
            let id = r.instance_id.clone();
            drop(guard);
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
