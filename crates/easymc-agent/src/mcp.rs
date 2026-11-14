//! Minimal MCP (Model Context Protocol) client for EasyMC.
//!
//! Tools exposed by user-configured stdio MCP servers are merged into the
//! cloud model's tool list as `mcp__<server>__<tool>`. Servers are spawned
//! lazily, once per app process, and reused across agent turns.
//!
//! Config: `~/.config/easymc/mcp.json` (override with `EASYMC_MCP_CONFIG`):
//! ```json
//! {
//!   "servers": {
//!     "filesystem": {
//!       "command": "npx",
//!       "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
//!       "enabled": true
//!     }
//!   }
//! }
//! ```

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::tool::{ToolArg, ToolCall, ToolStatus};
use crate::tool_runtime::ToolExecResult;

const DEFAULT_TIMEOUT_MS: u64 = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDescriptor {
    pub full_name: String,
    pub server: String,
    pub tool: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct McpStatus {
    pub configured: bool,
    pub config_path: String,
    pub servers: Vec<McpServerStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerStatus {
    pub name: String,
    pub connected: bool,
    pub tool_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ServerConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    servers: HashMap<String, ServerConfig>,
}

/// How many stderr lines to keep per server for diagnostics.
const STDERR_TAIL_LINES: usize = 50;
/// After a failure, wait this long before trying to spawn the server again.
const RECONNECT_COOLDOWN: Duration = Duration::from_secs(30);

struct Server {
    name: String,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr_tail: Arc<std::sync::Mutex<VecDeque<String>>>,
    next_id: u64,
    timeout: Duration,
    connected: bool,
    tool_count: usize,
}

impl Server {
    fn new(name: &str, cfg: &ServerConfig) -> Result<Self, String> {
        let mut cmd = Command::new(&cfg.command);
        cmd.args(&cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in &cfg.env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("启动 {} 失败: {e}", cfg.command))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "无法写入 MCP 服务 stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "无法读取 MCP 服务 stdout".to_string())?;
        let stderr_tail = Arc::new(std::sync::Mutex::new(VecDeque::new()));
        if let Some(stderr) = child.stderr.take() {
            let tail = Arc::clone(&stderr_tail);
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut q = tail.lock().unwrap_or_else(|e| e.into_inner());
                    if q.len() >= STDERR_TAIL_LINES {
                        q.pop_front();
                    }
                    q.push_back(line);
                }
            });
        }
        Ok(Self {
            name: name.to_string(),
            child,
            stdin,
            stdout: BufReader::new(stdout),
            stderr_tail,
            next_id: 1,
            timeout: Duration::from_millis(cfg.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)),
            connected: false,
            tool_count: 0,
        })
    }

    /// Last stderr lines, for error messages shown to the player / model.
    fn stderr_hint(&self) -> String {
        let q = self.stderr_tail.lock().unwrap_or_else(|e| e.into_inner());
        let tail = q
            .iter()
            .rev()
            .take(8)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        if tail.trim().is_empty() {
            String::new()
        } else {
            format!("\n服务 stderr:\n{tail}")
        }
    }

    async fn shutdown(&mut self) {
        self.connected = false;
        let _ = self.child.kill().await;
    }

    async fn send(&mut self, payload: &Value) -> Result<(), String> {
        let mut line = payload.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("写入 MCP 失败: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("刷新 MCP 失败: {e}"))
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
        .await
    }

    async fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))
        .await?;
        let wait = self.timeout;
        let id_str = id.to_string();
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = timeout(wait, self.stdout.read_line(&mut buf)).await;
            let n = match read {
                Err(_) => return Err(format!("MCP {method} 超时{}", self.stderr_hint())),
                Ok(Err(e)) => return Err(format!("读取 MCP 响应失败: {e}")),
                Ok(Ok(n)) => n,
            };
            if n == 0 {
                return Err(format!("MCP 服务已退出{}", self.stderr_hint()));
            }
            let line = buf.trim();
            if line.is_empty() {
                continue;
            }
            // Servers sometimes log to stdout before JSON; skip non-JSON noise.
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            // JSON-RPC ids may be numbers or strings; accept both.
            let id_matches = v
                .get("id")
                .map(|x| x.as_u64() == Some(id) || x.as_str() == Some(id_str.as_str()))
                .unwrap_or(false);
            if !id_matches {
                continue;
            }
            if let Some(err) = v.get("error") {
                return Err(format!(
                    "MCP 错误: {}{}",
                    err.get("message")
                        .and_then(|x| x.as_str())
                        .unwrap_or("unknown"),
                    self.stderr_hint()
                ));
            }
            return Ok(v.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    async fn handshake(&mut self) -> Result<Vec<McpToolDescriptor>, String> {
        let _ = self
            .call(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "EasyMC", "version": env!("CARGO_PKG_VERSION") }
                }),
            )
            .await?;
        self.notify("notifications/initialized", json!({})).await?;

        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = match &cursor {
                Some(c) => json!({ "cursor": c }),
                None => json!({}),
            };
            let res = self.call("tools/list", params).await?;
            if let Some(arr) = res.get("tools").and_then(|v| v.as_array()) {
                for t in arr {
                    let Some(name) = t.get("name").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    out.push(McpToolDescriptor {
                        full_name: namespaced(&self.name, name),
                        server: self.name.clone(),
                        tool: name.to_string(),
                        description: t
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .chars()
                            .take(500)
                            .collect::<String>(),
                        input_schema: t
                            .get("inputSchema")
                            .or_else(|| t.get("input_schema"))
                            .cloned()
                            .unwrap_or_else(|| json!({ "type": "object", "properties": {} })),
                    });
                }
            }
            cursor = res
                .get("nextCursor")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        self.connected = true;
        self.tool_count = out.len();
        Ok(out)
    }
}

struct Registry {
    initialized: bool,
    config_path: String,
    config: HashMap<String, ServerConfig>,
    /// Live handles. Never hold the registry lock while awaiting a server.
    servers: HashMap<String, Arc<Mutex<Server>>>,
    server_tools: HashMap<String, Vec<McpToolDescriptor>>,
    statuses: HashMap<String, McpServerStatus>,
    cooldown: HashMap<String, Instant>,
    tools: Vec<McpToolDescriptor>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            initialized: false,
            config_path: String::new(),
            config: HashMap::new(),
            servers: HashMap::new(),
            server_tools: HashMap::new(),
            statuses: HashMap::new(),
            cooldown: HashMap::new(),
            tools: Vec::new(),
        }
    }
}

impl Registry {
    fn status(&self) -> McpStatus {
        let mut names: Vec<&String> = self.statuses.keys().collect();
        names.sort();
        McpStatus {
            configured: !self.config.is_empty(),
            config_path: self.config_path.clone(),
            servers: names
                .into_iter()
                .filter_map(|n| self.statuses.get(n).cloned())
                .collect(),
        }
    }

    fn set_status(&mut self, name: &str, connected: bool, tool_count: usize, error: Option<String>) {
        self.statuses.insert(
            name.to_string(),
            McpServerStatus {
                name: name.to_string(),
                connected,
                tool_count,
                error,
            },
        );
    }
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
static TOOL_SNAPSHOT: OnceLock<RwLock<Vec<McpToolDescriptor>>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

fn snapshot() -> &'static RwLock<Vec<McpToolDescriptor>> {
    TOOL_SNAPSHOT.get_or_init(|| RwLock::new(Vec::new()))
}

fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("EASYMC_MCP_CONFIG") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("APPDATA")
                .ok()
                .map(PathBuf::from)
                .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("easymc").join("mcp.json")
}

fn load_config() -> (String, Option<ConfigFile>) {
    let path = config_path();
    let shown = path.display().to_string();
    let cfg = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<ConfigFile>(&raw).ok());
    (shown, cfg)
}

fn sanitize(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        s.push_str("tool");
    }
    s
}

fn namespaced(server: &str, tool: &str) -> String {
    let raw = format!("mcp__{}__{}", sanitize(server), sanitize(tool));
    if raw.len() <= 64 {
        return raw;
    }
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    let suffix = format!("_{:08x}", hasher.finish() as u32);
    let keep = 64 - suffix.len();
    format!("{}{}", &raw[..keep], suffix)
}

/// Cheap status: never spawns servers. Use after an ask (or when the UI asks
/// explicitly) to see whether the cached connections are healthy.
pub async fn status_snapshot() -> McpStatus {
    let reg = registry().lock().await;
    if reg.initialized {
        return reg.status();
    }
    let (path, cfg) = load_config();
    let mut status = McpStatus {
        configured: false,
        config_path: path,
        servers: Vec::new(),
    };
    if let Some(cfg) = cfg {
        status.configured = !cfg.servers.is_empty();
        let mut names: Vec<&String> = cfg.servers.keys().collect();
        names.sort();
        for name in names {
            if cfg.servers[name].enabled {
                status.servers.push(McpServerStatus {
                    name: name.clone(),
                    connected: false,
                    tool_count: 0,
                    error: None,
                });
            }
        }
    }
    status
}

/// Connect configured servers (idempotent) and return their status.
///
/// Failed servers are retried after [`RECONNECT_COOLDOWN`] instead of staying
/// dead for the whole app session. All awaits happen without the registry lock.
pub async fn prepare() -> McpStatus {
    let (path, cfg) = load_config();
    let mut names: Vec<String>;
    {
        let mut reg = registry().lock().await;
        if !reg.initialized {
            reg.initialized = true;
            reg.config_path = path;
            reg.config = cfg.map(|c| c.servers).unwrap_or_default();
            let enabled: Vec<(String, bool)> = reg
                .config
                .iter()
                .map(|(n, c)| (n.clone(), c.enabled))
                .collect();
            for (name, enabled) in enabled {
                if enabled {
                    reg.statuses.entry(name.clone()).or_insert(McpServerStatus {
                        name,
                        connected: false,
                        tool_count: 0,
                        error: None,
                    });
                }
            }
        }
        names = reg
            .config
            .iter()
            .filter(|(_, c)| c.enabled)
            .map(|(n, _)| n.clone())
            .collect();
    }
    names.sort();

    for name in &names {
        connect_if_needed(name).await;
    }

    let (status, tools) = {
        let mut reg = registry().lock().await;
        reg.tools = reg.server_tools.values().flatten().cloned().collect();
        (reg.status(), reg.tools.clone())
    };
    if let Ok(mut snap) = snapshot().write() {
        *snap = tools;
    }
    status
}

/// Spawn + handshake one server if it is not connected and not cooling down.
async fn connect_if_needed(name: &str) {
    let (handle, cfg) = {
        let reg = registry().lock().await;
        (reg.servers.get(name).cloned(), reg.config.get(name).cloned())
    };
    if let Some(handle) = handle {
        let healthy = handle.lock().await.connected;
        if healthy {
            return;
        }
        // Dead handle from a failed call: drop it and try a fresh process.
        let old = {
            let mut reg = registry().lock().await;
            reg.server_tools.remove(name);
            reg.servers.remove(name)
        };
        if let Some(old) = old {
            old.lock().await.shutdown().await;
        }
    }
    let Some(cfg) = cfg else {
        return;
    };
    {
        let reg = registry().lock().await;
        if reg
            .cooldown
            .get(name)
            .is_some_and(|until| *until > Instant::now())
        {
            return;
        }
    }

    match Server::new(name, &cfg) {
        Ok(mut server) => match server.handshake().await {
            Ok(tools) => {
                let count = tools.len();
                let mut reg = registry().lock().await;
                reg.servers
                    .insert(name.to_string(), Arc::new(Mutex::new(server)));
                reg.server_tools.insert(name.to_string(), tools);
                reg.set_status(name, true, count, None);
                reg.cooldown.remove(name);
            }
            Err(e) => {
                server.shutdown().await;
                eprintln!("[mcp] server {name} handshake failed: {e}");
                let mut reg = registry().lock().await;
                reg.set_status(name, false, 0, Some(e));
                reg.cooldown
                    .insert(name.to_string(), Instant::now() + RECONNECT_COOLDOWN);
            }
        },
        Err(e) => {
            eprintln!("[mcp] server {name} spawn failed: {e}");
            let mut reg = registry().lock().await;
            reg.set_status(name, false, 0, Some(e));
            reg.cooldown
                .insert(name.to_string(), Instant::now() + RECONNECT_COOLDOWN);
        }
    }
}

/// Tear down every server and reconnect from the current config file.
pub async fn reload() -> McpStatus {
    let handles: Vec<Arc<Mutex<Server>>> = {
        let reg = registry().lock().await;
        reg.servers.values().cloned().collect()
    };
    for h in handles {
        h.lock().await.shutdown().await;
    }
    {
        let mut reg = registry().lock().await;
        *reg = Registry::default();
    }
    if let Ok(mut snap) = snapshot().write() {
        snap.clear();
    }
    prepare().await
}

/// Synchronous snapshot used when building the OpenAI tool list.
pub fn tool_definitions() -> Value {
    let tools = snapshot().read().map(|s| s.clone()).unwrap_or_default();
    Value::Array(
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.full_name,
                        "description": format!("[MCP:{}] {}", t.server, t.description),
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect(),
    )
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel
        .map(|c| c.load(Ordering::Relaxed))
        .unwrap_or(false)
}

async fn wait_cancelled(cancel: Option<&AtomicBool>) {
    match cancel {
        None => std::future::pending::<()>().await,
        Some(flag) => {
            while !flag.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

pub async fn execute_tool(
    full_name: &str,
    args: &Value,
    cancel: Option<&AtomicBool>,
) -> ToolExecResult {
    if is_cancelled(cancel) {
        return mcp_error(full_name, "已取消");
    }
    let desc = snapshot()
        .read()
        .ok()
        .and_then(|s| s.iter().find(|t| t.full_name == full_name).cloned());
    let Some(desc) = desc else {
        return mcp_error(full_name, "MCP 工具未注册或服务未连接");
    };

    // Clone the handle and release the registry lock before awaiting the call.
    let handle = {
        let reg = registry().lock().await;
        reg.servers.get(&desc.server).cloned()
    };
    let handle = match handle {
        Some(h) => h,
        None => {
            connect_if_needed(&desc.server).await;
            let reg = registry().lock().await;
            match reg.servers.get(&desc.server).cloned() {
                Some(h) => h,
                None => {
                    return mcp_error(
                        full_name,
                        format!("MCP 服务 {} 未连接（稍后会自动重试）", desc.server),
                    )
                }
            }
        }
    };

    let (res, tail) = {
        let mut server = handle.lock().await;
        let res = tokio::select! {
            r = server.call("tools/call", json!({ "name": desc.tool, "arguments": args })) => r,
            _ = wait_cancelled(cancel) => Err("已取消".to_string()),
        };
        let tail = server.stderr_hint();
        if res.is_err() {
            // A timeout / protocol error leaves the pipe in an unknown state.
            server.shutdown().await;
        }
        (res, tail)
    };

    let value = match res {
        Ok(v) => v,
        Err(e) => {
            let tools = {
                let mut reg = registry().lock().await;
                reg.servers.remove(&desc.server);
                reg.server_tools.remove(&desc.server);
                reg.set_status(&desc.server, false, 0, Some(e.clone()));
                reg.cooldown
                    .insert(desc.server.clone(), Instant::now() + RECONNECT_COOLDOWN);
                reg.tools = reg.server_tools.values().flatten().cloned().collect();
                reg.tools.clone()
            };
            if let Ok(mut snap) = snapshot().write() {
                *snap = tools;
            }
            return mcp_error(full_name, format!("{e}{tail}"));
        }
    };
    let is_error = value
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut text = String::new();
    if let Some(content) = value.get("content").and_then(|v| v.as_array()) {
        for item in content {
            match item.get("type").and_then(|v| v.as_str()) {
                Some("text") => {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(t);
                    }
                }
                Some(kind) => {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&format!("[{kind} 内容已省略]"));
                }
                None => {}
            }
        }
    }
    if text.trim().is_empty() {
        text = serde_json::to_string(&value).unwrap_or_else(|_| "{}".into());
    }
    let status = if is_error {
        ToolStatus::Error
    } else {
        ToolStatus::Ok
    };
    let args_out = args
        .as_object()
        .map(|m| {
            m.iter()
                .take(6)
                .map(|(k, v)| ToolArg {
                    key: k.clone(),
                    label_zh: k.clone(),
                    value: match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    },
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let card = ToolCall {
        id: format!("mcp-{}", chrono::Local::now().format("%Y%m%d%H%M%S%3f")),
        name: full_name.into(),
        title_zh: format!("MCP · {}", desc.tool),
        status,
        args: args_out,
        result_zh: Some(text.clone()),
        path: None,
        payload: Some(value.clone()),
        ui_actions: vec![],
        tag_zh: Some(format!("MCP/{}", desc.server)),
    };
    ToolExecResult {
        message_zh: text.clone(),
        model_text: if text.chars().count() > 6000 {
            text.chars().take(6000).collect::<String>() + "…"
        } else {
            text
        },
        card,
        action: None,
        report: None,
    }
}

fn mcp_error(name: &str, err: impl std::fmt::Display) -> ToolExecResult {
    let msg = format!("MCP 工具没做成：{err}");
    let card = ToolCall {
        id: format!("mcp-{}", chrono::Local::now().format("%Y%m%d%H%M%S%3f")),
        name: name.into(),
        title_zh: "MCP 工具".into(),
        status: ToolStatus::Error,
        args: vec![],
        result_zh: Some(msg.clone()),
        path: None,
        payload: None,
        ui_actions: vec![],
        tag_zh: Some("MCP".into()),
    };
    ToolExecResult {
        model_text: msg.clone(),
        message_zh: msg,
        card,
        action: None,
        report: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaced_is_openai_safe() {
        let n = namespaced("my server", "read.file");
        assert!(n.starts_with("mcp__my_server__read_file"));
        assert!(n.len() <= 64);
        assert!(n
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'));
    }

    #[test]
    fn long_names_are_capped_and_unique() {
        let long = "x".repeat(90);
        let a = namespaced("s", &long);
        let b = namespaced("s2", &long);
        assert!(a.len() <= 64 && b.len() <= 64);
        assert_ne!(a, b);
    }

    #[test]
    fn definitions_are_empty_without_servers() {
        assert_eq!(tool_definitions(), json!([]));
    }
}
