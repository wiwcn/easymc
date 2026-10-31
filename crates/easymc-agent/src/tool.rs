//! Normalized tool-call DTO for Agent UI cards (rules + LLM host paths).

use serde::{Deserialize, Serialize};
use crate::actions::ActionResult;
use crate::diagnose::DiagnoseReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Pending,
    Running,
    Ok,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolArg {
    pub key: String,
    pub label_zh: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUiAction {
    pub id: String,
    pub label_zh: String,
}

/// One tool invocation the chat UI can render as a complete card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub title_zh: String,
    pub status: ToolStatus,
    pub args: Vec<ToolArg>,
    pub result_zh: Option<String>,
    pub path: Option<String>,
    /// Structured payload (proposal JSON, install meta, etc.).
    pub payload: Option<serde_json::Value>,
    pub ui_actions: Vec<ToolUiAction>,
    pub tag_zh: Option<String>,
}

fn arg(key: &str, label_zh: &str, value: impl Into<String>) -> ToolArg {
    ToolArg {
        key: key.into(),
        label_zh: label_zh.into(),
        value: value.into(),
    }
}

fn ui(id: &str, label_zh: &str) -> ToolUiAction {
    ToolUiAction {
        id: id.into(),
        label_zh: label_zh.into(),
    }
}

fn new_id() -> String {
    let stamp = chrono::Local::now().format("%Y%m%d%H%M%S%3f");
    format!("tool-{stamp}")
}

/// Player-facing title + tag for a known tool name.
pub fn tool_meta(name: &str) -> (&'static str, Option<&'static str>) {
    match name {
        "ensure_java" => ("准备 Java", Some("环境")),
        "setup_env" => ("一键环境", Some("环境")),
        "install_mod" => ("安装模组", Some("模组")),
        "search_mods" => ("搜索模组", Some("模组")),
        "suggest_mods" => ("推荐模组", Some("模组")),
        "open_store" | "install_mod_hint" => ("打开下载", Some("模组")),
        "bump_memory" => ("调整内存", Some("配置")),
        "propose_changes" => ("建议更改", Some("配置")),
        "apply_changes" => ("应用更改", Some("配置")),
        "undo_changes" => ("撤销更改", Some("配置")),
        "set_download_source" => ("切换下载源", Some("下载")),
        "open_instance_dir" => ("打开版本文件夹", Some("文件")),
        "show_data_dir" => ("数据目录", Some("文件")),
        "check_mods" | "list_installed_mods" => ("已装模组", Some("模组")),
        "check_mod_deps" | "mod_dep_graph" | "analyze_mod_deps" => ("模组依赖", Some("依赖")),
        "fetch_missing_mod_deps" | "install_missing_deps" => ("补全依赖", Some("依赖")),
        "toggle_mod" | "enable_mod" | "disable_mod" => ("启用/禁用模组", Some("模组")),
        "list_instances" | "list_versions" => ("版本列表", Some("版本")),
        "select_instance" | "switch_instance" | "switch_version" => ("切换版本", Some("版本")),
        "rename_instance" | "rename_version" => ("重命名版本", Some("版本")),
        "duplicate_instance" | "duplicate_version" | "copy_instance" => ("复制版本", Some("版本")),
        "delete_instance" | "delete_version" | "remove_instance" => ("删除版本", Some("版本")),
        "delete_mod" | "remove_mod" | "uninstall_mod" => ("删除模组", Some("模组")),
        "open_latest_log" | "open_log" => ("打开日志", Some("文件")),
        "list_accounts" => ("账户列表", Some("账户")),
        "select_account" | "switch_account" => ("切换账户", Some("账户")),
        "open_screenshots" | "screenshots" => ("打开截图", Some("文件")),
        "open_mods" => ("打开模组文件夹", Some("文件")),
        "open_resourcepacks" => ("打开资源包文件夹", Some("文件")),
        "open_shaderpacks" => ("打开光影文件夹", Some("文件")),
        "open_saves" => ("打开存档文件夹", Some("文件")),
        "open_config" => ("打开配置文件夹", Some("文件")),
        "open_logs" => ("打开日志文件夹", Some("文件")),
        "open_crash-reports" | "open_crash_reports" => ("打开崩溃报告文件夹", Some("文件")),
        "open_game_folder" => ("打开游戏文件夹", Some("文件")),
        "import_mrpack" | "import_modpack" => ("导入整合包", Some(".mrpack")),
        "build_pack" | "create_modpack" | "export_modpack" => ("制作整合包", Some(".mrpack")),
        "diagnostics" => ("导出问题报告", Some("诊断")),
        "diagnose" | "analyze_crash_log" | "analyze_launch_log" => ("查看崩溃", Some("诊断")),
        "suggest_memory" => ("内存建议", Some("配置")),
        "apply_game_layer" => ("应用隔离改动", Some("隔离")),
        "discard_game_layer" => ("丢弃隔离改动", Some("隔离")),
        "modify_saves" | "write_saves" => ("修改存档", Some("存档")),
        _ => ("工具", None),
    }
}

/// Build a running placeholder card (UI inserts before host finishes).
pub fn tool_running(name: &str, args: Vec<ToolArg>) -> ToolCall {
    let (title, tag) = tool_meta(name);
    ToolCall {
        id: new_id(),
        name: name.into(),
        title_zh: title.into(),
        status: ToolStatus::Running,
        args,
        result_zh: Some("正在执行…".into()),
        path: None,
        payload: None,
        ui_actions: vec![],
        tag_zh: tag.map(|s| s.into()),
    }
}

fn args_from_value(name: &str, value: Option<&serde_json::Value>, path: Option<&str>) -> Vec<ToolArg> {
    let mut out = Vec::new();
    let v = value.unwrap_or(&serde_json::Value::Null);
    match name {
        "ensure_java" | "setup_env" => {
            if let Some(m) = v.get("major").and_then(|x| x.as_u64()) {
                out.push(arg("major", "Java 版本", format!("{m}")));
            }
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(mem) = v.get("memory_mb").and_then(|x| x.as_u64()) {
                out.push(arg("memory", "内存", format!("{mem} MB")));
            }
            if let Some(s) = v
                .get("download_source_zh")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("download_source").and_then(|x| x.as_str()))
            {
                out.push(arg("source", "下载源", s));
            }
            if let Some(p) = v.get("path").and_then(|x| x.as_str()).or(path) {
                out.push(arg("path", "路径", p));
            }
            if let Some(s) = v.get("source").and_then(|x| x.as_str()) {
                if name == "ensure_java" {
                    out.push(arg("java_source", "来源", s));
                }
            }
        }
        "install_mod" | "open_store" | "install_mod_hint" | "search_mods" => {
            if let Some(q) = v.get("query").and_then(|x| x.as_str()) {
                out.push(arg("query", "关键词", q));
            }
            if let Some(n) = v.get("project_title").or(v.get("name")).and_then(|x| x.as_str()) {
                out.push(arg("mod", "模组", n));
            }
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(c) = v.get("count").and_then(|x| x.as_u64()) {
                out.push(arg("count", "结果", format!("{c}")));
            }
            if let Some(gv) = v.get("game_version").and_then(|x| x.as_str()) {
                out.push(arg("mc", "游戏版本", gv));
            }
        }
        "suggest_mods" => {
            if let Some(th) = v.get("theme").and_then(|x| x.as_str()) {
                out.push(arg("theme", "主题", th));
            }
            if let Some(c) = v.get("count").and_then(|x| x.as_u64()) {
                out.push(arg("count", "推荐", format!("{c}")));
            }
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
        }
        "bump_memory" | "suggest_memory" => {
            if let Some(m) = v.get("memory_mb").or(v.get("suggested")).and_then(|x| x.as_u64()) {
                out.push(arg("memory", "目标内存", format!("{m} MB")));
            }
            if let Some(p) = v.get("previous").or(v.get("current")).and_then(|x| x.as_u64()) {
                out.push(arg("previous", "之前", format!("{p} MB")));
            }
        }
        "propose_changes" => {
            if let Some(title) = v.get("title_zh").and_then(|x| x.as_str()) {
                out.push(arg("title", "建议", title));
            }
            if let Some(items) = v.get("items").and_then(|x| x.as_array()) {
                out.push(arg("count", "改动项", format!("{} 项", items.len())));
            }
        }
        "set_download_source" => {
            if let Some(l) = v
                .get("label_zh")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("download_source").and_then(|x| x.as_str()))
            {
                out.push(arg("source", "下载源", l));
            }
        }
        "open_instance_dir" | "show_data_dir" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(p) = path.or_else(|| v.get("game_dir").and_then(|x| x.as_str())) {
                out.push(arg("path", "路径", p));
            }
        }
        "check_mods" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(c) = v.get("count").and_then(|x| x.as_u64()) {
                out.push(arg("count", "模组数", format!("{c}")));
            }
        }
        "check_mod_deps" | "mod_dep_graph" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(c) = v.get("installed_count").and_then(|x| x.as_u64()) {
                out.push(arg("installed", "已解析", format!("{c}")));
            }
            if let Some(arr) = v.get("missing_ids").and_then(|x| x.as_array()) {
                out.push(arg("missing", "缺失", format!("{}", arr.len())));
            }
            if let Some(arr) = v.get("conflict_messages").and_then(|x| x.as_array()) {
                out.push(arg("conflicts", "冲突", format!("{}", arr.len())));
            }
        }
        "fetch_missing_mod_deps" | "install_missing_deps" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(arr) = v.get("targets").or(v.get("attempted")).and_then(|x| x.as_array()) {
                out.push(arg("targets", "尝试", format!("{}", arr.len())));
            }
            if let Some(arr) = v.get("installed").and_then(|x| x.as_array()) {
                out.push(arg("installed", "成功", format!("{}", arr.len())));
            }
            if let Some(arr) = v.get("failed").and_then(|x| x.as_array()) {
                out.push(arg("failed", "失败", format!("{}", arr.len())));
            }
        }
        "toggle_mod" | "enable_mod" | "disable_mod" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(f) = v.get("filename").and_then(|x| x.as_str()) {
                out.push(arg("mod", "模组", f));
            }
            if let Some(q) = v.get("query").and_then(|x| x.as_str()) {
                out.push(arg("query", "关键词", q));
            }
            if let Some(e) = v.get("enabled").and_then(|x| x.as_bool()) {
                out.push(arg("enabled", "状态", if e { "启用" } else { "禁用" }));
            }
        }
        "list_instances" | "list_versions" => {
            if let Some(c) = v.get("count").and_then(|x| x.as_u64()) {
                out.push(arg("count", "版本数", format!("{c}")));
            }
        }
        "select_instance" | "switch_instance" | "switch_version" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
                out.push(arg("mc", "MC", ver));
            }
            if let Some(l) = v.get("loader").and_then(|x| x.as_str()) {
                out.push(arg("loader", "加载器", l));
            }
            if let Some(q) = v.get("query").and_then(|x| x.as_str()) {
                out.push(arg("query", "关键词", q));
            }
        }
        "rename_instance" | "rename_version" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "新名称", n));
            }
            if let Some(p) = v.get("previous_name").and_then(|x| x.as_str()) {
                out.push(arg("previous", "原名称", p));
            }
        }
        "duplicate_instance" | "duplicate_version" | "copy_instance" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "新版本", n));
            }
            if let Some(s) = v.get("source_name").and_then(|x| x.as_str()) {
                out.push(arg("source", "来源", s));
            }
            if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
                out.push(arg("mc", "MC", ver));
            }
            if let Some(l) = v.get("loader").and_then(|x| x.as_str()) {
                out.push(arg("loader", "加载器", l));
            }
        }
        "delete_instance" | "delete_version" | "remove_instance" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(ver) = v.get("version").and_then(|x| x.as_str()) {
                out.push(arg("mc", "MC", ver));
            }
            if let Some(l) = v.get("loader").and_then(|x| x.as_str()) {
                out.push(arg("loader", "加载器", l));
            }
        }
        "delete_mod" | "remove_mod" | "uninstall_mod" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(f) = v.get("filename").and_then(|x| x.as_str()) {
                out.push(arg("file", "文件", f));
            }
            if let Some(q) = v.get("query").and_then(|x| x.as_str()) {
                out.push(arg("query", "关键词", q));
            }
        }
        "open_latest_log" | "open_log" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(k) = v.get("kind").and_then(|x| x.as_str()) {
                out.push(arg("kind", "类型", k));
            }
            if let Some(p) = path.or_else(|| v.get("path").and_then(|x| x.as_str())) {
                out.push(arg("path", "日志", p));
            }
        }
        "list_accounts" => {
            if let Some(c) = v.get("count").and_then(|x| x.as_u64()) {
                out.push(arg("count", "账户数", format!("{c}")));
            }
        }
        "select_account" | "switch_account" => {
            if let Some(n) = v.get("username").and_then(|x| x.as_str()) {
                out.push(arg("username", "账户", n));
            }
            if let Some(k) = v.get("kind").and_then(|x| x.as_str()) {
                out.push(arg("kind", "类型", k));
            }
            if let Some(q) = v.get("query").and_then(|x| x.as_str()) {
                out.push(arg("query", "关键词", q));
            }
        }
        "open_screenshots" | "screenshots" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(c) = v.get("count").and_then(|x| x.as_u64()) {
                out.push(arg("count", "截图数", format!("{c}")));
            }
            if let Some(p) = path.or_else(|| v.get("path").and_then(|x| x.as_str())) {
                out.push(arg("path", "文件夹", p));
            }
        }
        "open_mods" | "open_resourcepacks" | "open_shaderpacks" | "open_saves"
        | "open_config" | "open_logs" | "open_crash-reports" | "open_crash_reports"
        | "open_game_folder" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(k) = v
                .get("kind_zh")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("kind").and_then(|x| x.as_str()))
            {
                out.push(arg("kind", "类型", k));
            }
            if let Some(c) = v.get("count").and_then(|x| x.as_u64()) {
                out.push(arg("count", "数量", format!("{c}")));
            }
            if let Some(p) = path.or_else(|| v.get("path").and_then(|x| x.as_str())) {
                out.push(arg("path", "文件夹", p));
            }
        }
        "import_mrpack" | "import_modpack" => {
            if let Some(p) = path.or_else(|| v.get("path").and_then(|x| x.as_str())) {
                out.push(arg("path", ".mrpack", p));
            }
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "新版本", n));
            }
        }
        "build_pack" => {
            if let Some(n) = v.get("instance_name").and_then(|x| x.as_str()) {
                out.push(arg("instance", "版本", n));
            }
            if let Some(c) = v.get("mod_count").and_then(|x| x.as_u64()) {
                out.push(arg("mods", "模组", format!("{c}")));
            }
            if let Some(c) = v.get("resolved").and_then(|x| x.as_u64()) {
                out.push(arg("resolved", "CDN", format!("{c}")));
            }
            if let Some(c) = v.get("overrides").and_then(|x| x.as_u64()) {
                out.push(arg("overrides", "本地覆盖", format!("{c}")));
            }
            if let Some(p) = path.or_else(|| v.get("path").and_then(|x| x.as_str())) {
                out.push(arg("path", ".mrpack", p));
            }
        }
        "diagnostics" | "diagnose" | "analyze_crash_log" | "analyze_launch_log" => {
            if let Some(p) = path {
                out.push(arg("path", "报告", p));
            }
            if let Some(s) = v.get("source_label").and_then(|x| x.as_str()) {
                out.push(arg("source", "日志来源", s));
            }
            if let Some(c) = v.get("finding_count").and_then(|x| x.as_u64()) {
                out.push(arg("findings", "问题数", format!("{c}")));
            }
        }
        _ => {
            if let Some(p) = path {
                out.push(arg("path", "路径", p));
            }
        }
    }
    out
}

fn ui_actions_for(name: &str, ok: bool, value: Option<&serde_json::Value>) -> Vec<ToolUiAction> {
    let mut acts = Vec::new();
    match name {
        "propose_changes" if ok => {
            acts.push(ui("apply_proposal", "应用更改"));
            acts.push(ui("undo_proposal", "撤销"));
        }
        "open_store" | "install_mod_hint" => {
            acts.push(ui("open_download", "打开下载"));
        }
        "search_mods" if ok => {
            acts.push(ui("open_download", "去下载页"));
        }
        "suggest_mods" if ok => {
            acts.push(ui("open_download", "去下载页看看"));
        }
        "install_mod" if !ok => {
            acts.push(ui("open_download", "去下载页"));
        }
        "open_instance_dir" if ok => {
            acts.push(ui("open_folder", "在访达 / 资源管理器打开"));
        }
        "check_mods" => {
            if value
                .and_then(|v| v.get("suggest_diagnose"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
            {
                acts.push(ui("run_diagnose", "查看崩溃"));
            }
        }
        "check_mod_deps" if ok => {
            if value
                .and_then(|v| v.get("suggest_fetch_missing"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
            {
                acts.push(ui("run_fetch_missing_deps", "补全缺失依赖"));
            }
            acts.push(ui("run_check_mod_deps", "再查依赖"));
        }
        "fetch_missing_mod_deps" if ok => {
            acts.push(ui("run_check_mod_deps", "再查依赖"));
            acts.push(ui("run_check_mods", "再看模组列表"));
        }
        "build_pack" | "import_mrpack" | "diagnostics" if ok => {
            acts.push(ui(
                "reveal_path",
                if name == "build_pack" {
                    "打开整合包位置"
                } else if name == "import_mrpack" {
                    "打开版本文件夹"
                } else {
                    "打开报告位置"
                },
            ));
        }
        "toggle_mod" if ok => {
            acts.push(ui("run_check_mods", "再看模组列表"));
        }
        "list_instances" if ok => {
            acts.push(ui("open_home", "去启动页"));
        }
        "select_instance" if ok => {
            acts.push(ui("open_home", "去启动页"));
        }
        "rename_instance" if ok => {
            acts.push(ui("open_home", "去启动页"));
        }
        "duplicate_instance" if ok => {
            acts.push(ui("open_home", "去启动页"));
            acts.push(ui("reveal_path", "打开新版本文件夹"));
        }
        "delete_instance" if ok => {
            acts.push(ui("open_home", "去启动页"));
        }
        "delete_mod" if ok => {
            acts.push(ui("run_check_mods", "再看模组列表"));
            acts.push(ui("open_mods", "打开模组文件夹"));
        }
        "open_latest_log" if ok => {
            acts.push(ui("reveal_path", "打开日志位置"));
            acts.push(ui("run_diagnose", "分析这份日志"));
        }
        "list_accounts" if ok => {
            acts.push(ui("open_accounts", "去账户管理"));
        }
        "select_account" if ok => {
            acts.push(ui("open_accounts", "去账户管理"));
        }
        "open_screenshots" if ok => {
            acts.push(ui("reveal_path", "打开截图文件夹"));
        }
        "open_mods" | "open_resourcepacks" | "open_shaderpacks" | "open_saves"
        | "open_config" | "open_logs" | "open_crash-reports" | "open_crash_reports"
        | "open_game_folder"
            if ok =>
        {
            let label = match name {
                "open_mods" => "打开模组文件夹",
                "open_resourcepacks" => "打开资源包文件夹",
                "open_shaderpacks" => "打开光影文件夹",
                "open_saves" => "打开存档文件夹",
                "open_config" => "打开配置文件夹",
                "open_logs" => "打开日志文件夹",
                "open_crash-reports" | "open_crash_reports" => "打开崩溃报告文件夹",
                _ => "打开文件夹",
            };
            acts.push(ui("reveal_path", label));
        }
        _ => {}
    }
    acts
}

/// Normalize any ActionResult into a ToolCall the UI can render.
pub fn tool_call_from_action(ar: &ActionResult) -> ToolCall {
    let name = ar.action.as_str();
    let (title, tag) = tool_meta(name);
    let status = if ar.ok {
        ToolStatus::Ok
    } else {
        ToolStatus::Error
    };
    let mut title_zh = title.to_string();
    let mut tag_zh = tag.map(|s| s.to_string());
    // Prefer proposal title/tag when present.
    if name == "propose_changes" {
        if let Some(v) = &ar.value {
            if let Some(t) = v.get("title_zh").and_then(|x| x.as_str()) {
                title_zh = t.to_string();
            }
            if let Some(t) = v.get("tag_zh").and_then(|x| x.as_str()) {
                tag_zh = Some(t.to_string());
            }
        }
    }
    ToolCall {
        id: new_id(),
        name: name.into(),
        title_zh,
        status,
        args: args_from_value(name, ar.value.as_ref(), ar.path.as_deref()),
        result_zh: Some(ar.message_zh.clone()),
        path: ar.path.clone(),
        payload: ar.value.clone(),
        ui_actions: ui_actions_for(name, ar.ok, ar.value.as_ref()),
        tag_zh,
    }
}

/// Diagnose-only tool card (no ActionResult side effect).
pub fn tool_call_from_diagnose(report: &DiagnoseReport) -> ToolCall {
    let has_error = report
        .findings
        .iter()
        .any(|f| matches!(f.severity, crate::patterns::FindingSeverity::Error));
    let status = if has_error {
        ToolStatus::Error
    } else {
        ToolStatus::Ok
    };
    let mut args = vec![arg(
        "findings",
        "问题数",
        format!("{}", report.findings.len()),
    )];
    if !report.source_label.is_empty() {
        args.push(arg("source", "日志来源", &report.source_label));
    }
    ToolCall {
        id: new_id(),
        name: "diagnose".into(),
        title_zh: "查看崩溃".into(),
        status,
        args,
        result_zh: Some(report.summary_zh.clone()),
        path: None,
        payload: Some(serde_json::json!({
            "ok": report.ok,
            "summary_zh": report.summary_zh,
            "finding_count": report.findings.len(),
        })),
        ui_actions: vec![],
        tag_zh: Some("诊断".into()),
    }
}

/// Attach / refresh `tool` on an AskResponse from action or diagnose report.
pub fn attach_tool_to_ask(
    action: Option<&ActionResult>,
    report: Option<&DiagnoseReport>,
) -> Option<ToolCall> {
    if let Some(ar) = action {
        return Some(tool_call_from_action(ar));
    }
    if let Some(r) = report {
        return Some(tool_call_from_diagnose(r));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_action_ok_card() {
        let ar = ActionResult {
            ok: true,
            action: "set_download_source".into(),
            message_zh: "已切换".into(),
            path: None,
            value: Some(serde_json::json!({
                "download_source": "bmclapi",
                "label_zh": "BMCLAPI"
            })),
        };
        let t = tool_call_from_action(&ar);
        assert_eq!(t.name, "set_download_source");
        assert_eq!(t.status, ToolStatus::Ok);
        assert_eq!(t.title_zh, "切换下载源");
        assert!(!t.args.is_empty());
    }

    #[test]
    fn running_placeholder() {
        let t = tool_running("setup_env", vec![arg("x", "测试", "1")]);
        assert_eq!(t.status, ToolStatus::Running);
        assert_eq!(t.title_zh, "一键环境");
    }
}

/// After host rewrites `action` (ensure_java / install_mod / …), refresh the card.
pub fn refresh_ask_tool(
    action: Option<&ActionResult>,
    report: Option<&DiagnoseReport>,
) -> Option<ToolCall> {
    attach_tool_to_ask(action, report)
}
