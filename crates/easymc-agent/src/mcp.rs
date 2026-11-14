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
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

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

struct Server {
    name: String,
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    timeout: Duration,
    connected: bool,
    error: Option<String>,
    tool_count: usize,
}

impl Server {
    fn new(name: &str, cfg: &ServerConfig) -> Result<Self, String> {
        let mut cmd = Command::new(&cfg.command);
        cmd.args(&cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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
        Ok(Self {
            name: name.to_string(),
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            timeout: Duration::from_millis(cfg.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)),
            connected: false,
            error: None,
            tool_count: 0,
        })
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
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = timeout(wait, self.stdout.read_line(&mut buf)).await;
            let n = match read {
                Err(_) => return Err(format!("MCP {method} 超时")),
                Ok(Err(e)) => return Err(format!("读取 MCP 响应失败: {e}")),
                Ok(Ok(n)) => n,
            };
            if n == 0 {
                return Err("MCP 服务已退出".into());
            }
            let line = buf.trim();
            if line.is_empty() {
                continue;
            }
            // Servers sometimes log to stdout before JSON; skip non-JSON noise.
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if v.get("id").and_then(|x| x.as_u64()) != Some(id) {
                continue;
            }
            if let Some(err) = v.get("error") {
                return Err(format!(
                    "MCP 错误: {}",
                    err.get("message")
                        .and_then(|x| x.as_str())
                        .unwrap_or("unknown")
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
    status: McpStatus,
    servers: HashMap<String, Server>,
    tools: Vec<McpToolDescriptor>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            initialized: false,
            status: McpStatus::default(),
            servers: HashMap::new(),
            tools: Vec::new(),
        }
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
        return reg.status.clone();
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
pub async fn prepare() -> McpStatus {
    let mut reg = registry().lock().await;
    if reg.initialized {
        return reg.status.clone();
    }
    reg.initialized = true;
    let (path, cfg) = load_config();
    reg.status.config_path = path;
    let Some(cfg) = cfg else {
        return reg.status.clone();
    };
    reg.status.configured = !cfg.servers.is_empty();

    let mut names: Vec<&String> = cfg.servers.keys().collect();
    names.sort();
    for name in names {
        let sc = &cfg.servers[name];
        if !sc.enabled {
            continue;
        }
        let mut status = McpServerStatus {
            name: name.clone(),
            connected: false,
            tool_count: 0,
            error: None,
        };
        match Server::new(name, sc) {
            Ok(mut server) => match server.handshake().await {
                Ok(tools) => {
                    status.connected = true;
                    status.tool_count = tools.len();
                    reg.tools.extend(tools);
                    reg.servers.insert(name.clone(), server);
                }
                Err(e) => {
                    status.error = Some(e.clone());
                    if let Some(s) = reg.servers.get_mut(name) {
                        s.error = Some(e);
                        s.connected = false;
                    }
                }
            },
            Err(e) => status.error = Some(e),
        }
        reg.status.servers.push(status);
    }
    if let Ok(mut snap) = snapshot().write() {
        *snap = reg.tools.clone();
    }
    reg.status.clone()
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

pub async fn execute_tool(full_name: &str, args: &Value) -> ToolExecResult {
    let desc = snapshot()
        .read()
        .ok()
        .and_then(|s| s.iter().find(|t| t.full_name == full_name).cloned());
    let Some(desc) = desc else {
        return mcp_error(full_name, "MCP 工具未注册或服务未连接");
    };

    let mut reg = registry().lock().await;
    let Some(server) = reg.servers.get_mut(&desc.server) else {
        return mcp_error(full_name, format!("MCP 服务 {} 未连接", desc.server));
    };
    let res = server
        .call("tools/call", json!({ "name": desc.tool, "arguments": args }))
        .await;
    let value = match res {
        Ok(v) => v,
        Err(e) => return mcp_error(full_name, e),
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
