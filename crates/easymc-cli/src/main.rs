use std::sync::Arc;

use easymc_core::{
    create_named_instance_with, create_offline_account, ensure_defaults, load_accounts,
    load_instances, load_settings, save_accounts, save_instances, set_active_instance,
    CreateInstanceOpts, McVersion, ModLoader, Paths,
};
use easymc_launch_desktop::{
    detect_java, ensure_java, prepare_and_launch, LaunchRequest, ProcessManager,
};
use easymc_meta::{
    install_fabric, install_forge, install_neoforge, install_quilt, install_version,
    is_loader_installed, is_version_installed, list_loaders, InstallOptions, LoaderKind,
    MetaProvider, PistonMetaProvider,
};
use easymc_agent::{ask, llm_status, run_diagnose, write_diagnostic_bundle, AskRequest};
use easymc_mods::{
    install_mod, list_installed_mods, list_mod_versions, search_mods, ModSource, SearchQuery,
    API_KEY_ENV,
};

#[tokio::main]
async fn main() {
    if let Err(e) = real_main().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn real_main() -> easymc_core::Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_help();
        return Ok(());
    }
    let cmd = args.remove(0);
    let paths = Paths::discover()?;
    paths.ensure()?;
    ensure_defaults(&paths)?;

    match cmd.as_str() {
        "help" | "-h" | "--help" => print_help(),
        "paths" => {
            println!("{}", paths.root.display());
        }
        "java" => {
            for j in detect_java() {
                println!(
                    "{}  major={}  ({})  {}",
                    j.path.display(),
                    j.major,
                    j.source,
                    j.version_string
                );
            }
        }
        "ensure-java" => {
            let major = args.first().and_then(|s| s.parse().ok()).unwrap_or(21);
            let j = ensure_java(&paths, major, |s| println!("{s}")).await?;
            println!("OK {}", j.path.display());
        }
        "list-versions" => {
            let p = PistonMetaProvider::new();
            let list = p.list_versions().await?;
            let include_snapshots = args.iter().any(|a| a == "--snapshots" || a == "snapshot");
            let filter = args
                .iter()
                .find(|a| *a != "--snapshots" && *a != "snapshot")
                .cloned();
            for v in list
                .into_iter()
                .filter(|v| {
                    if let Some(ref f) = filter {
                        v.id.contains(f.as_str()) || v.version_type == *f
                    } else if include_snapshots {
                        true
                    } else {
                        v.version_type == "release"
                    }
                })
                .take(if include_snapshots { 80 } else { 40 })
            {
                println!(
                    "{}\t{}\t{}",
                    v.id,
                    v.version_type,
                    if is_version_installed(&paths, &v.id) {
                        "installed"
                    } else {
                        "-"
                    }
                );
            }
        }
        "list-loaders" => {
            let kind_s = args
                .first()
                .cloned()
                .unwrap_or_else(|| "fabric".into());
            let game = args.get(1).cloned().unwrap_or_else(|| "1.21.1".into());
            let kind = parse_loader_kind(&kind_s)?;
            let list = list_loaders(kind, &game).await?;
            for (i, l) in list.into_iter().take(20).enumerate() {
                let mark = if l.loader.stable { "stable" } else { "-" };
                let flag = if i == 0 || l.loader.stable { "*" } else { " " };
                println!(
                    "{flag}\t{}\t{}\tintermediary={}",
                    l.loader.version, mark, l.intermediary.version
                );
            }
        }
        "install" => {
            let id = args
                .first()
                .ok_or_else(|| easymc_core::EasyMcError::msg("usage: install <version>"))?;
            let skip_assets = args.iter().any(|a| a == "--no-assets");
            install_version(
                &paths,
                &McVersion(id.clone()),
                InstallOptions {
                    assets: !skip_assets,
                    natives: true,
                },
                |p| {
                    eprintln!("{}", p.format_line());
                },
            )
            .await?;
            let mut store = load_instances(&paths)?;
            for inst in &mut store.instances {
                if inst.version.0 == *id && inst.loader == ModLoader::Vanilla {
                    inst.version_ready = true;
                }
            }
            save_instances(&paths, &store)?;
            eprintln!("OK 已安装 {id}");
            println!("installed {id}");
        }
        "install-fabric" | "install-quilt" | "install-forge" | "install-neoforge" => {
            let kind = match cmd.as_str() {
                "install-quilt" => LoaderKind::Quilt,
                "install-forge" => LoaderKind::Forge,
                "install-neoforge" => LoaderKind::NeoForge,
                _ => LoaderKind::Fabric,
            };
            let game = args
                .first()
                .cloned()
                .ok_or_else(|| {
                    easymc_core::EasyMcError::msg(format!(
                        "usage: {} <game-version> [--loader VER] [--no-assets]",
                        cmd
                    ))
                })?;
            let skip_assets = args.iter().any(|a| a == "--no-assets");
            let loader_ver = flag_value(&args, "--loader");
            let vanilla = InstallOptions {
                assets: !skip_assets,
                natives: true,
            };
            let (profile_id, resolved_loader) = match kind {
                LoaderKind::Fabric => {
                    install_fabric(
                        &paths,
                        &McVersion(game.clone()),
                        loader_ver.clone(),
                        vanilla,
                        |p| eprintln!("{}", p.format_line()),
                    )
                    .await?
                }
                LoaderKind::Quilt => {
                    install_quilt(
                        &paths,
                        &McVersion(game.clone()),
                        loader_ver.clone(),
                        vanilla,
                        |p| eprintln!("{}", p.format_line()),
                    )
                    .await?
                }
                LoaderKind::Forge => {
                    let java = ensure_java(&paths, 21, |s| eprintln!("{s}")).await?;
                    install_forge(
                        &paths,
                        &McVersion(game.clone()),
                        loader_ver.clone(),
                        &java.path,
                        vanilla,
                        |p| eprintln!("{}", p.format_line()),
                    )
                    .await?
                }
                LoaderKind::NeoForge => {
                    let java = ensure_java(&paths, 21, |s| eprintln!("{s}")).await?;
                    install_neoforge(
                        &paths,
                        &McVersion(game.clone()),
                        loader_ver.clone(),
                        &java.path,
                        vanilla,
                        |p| eprintln!("{}", p.format_line()),
                    )
                    .await?
                }
            };
            let mut store = load_instances(&paths)?;
            let loader_enum = match kind {
                LoaderKind::Fabric => ModLoader::Fabric,
                LoaderKind::Quilt => ModLoader::Quilt,
                LoaderKind::Forge => ModLoader::Forge,
                LoaderKind::NeoForge => ModLoader::NeoForge,
            };
            for inst in &mut store.instances {
                if inst.version.0 == game && inst.loader == loader_enum {
                    inst.loader_version = Some(resolved_loader.clone());
                    inst.version_ready = true;
                }
            }
            save_instances(&paths, &store)?;
            eprintln!(
                "OK 已安装 {} {} @ {} → {profile_id}",
                kind.label_zh(),
                resolved_loader,
                game
            );
            println!("installed {profile_id}");
        }
        "instances" => {
            let store = load_instances(&paths)?;
            let active = store.active_id.clone().unwrap_or_default();
            for i in store.instances {
                let mark = if i.id == active { "*" } else { " " };
                let lv = i.loader_version.as_deref().unwrap_or("-");
                println!(
                    "{mark}\t{}\t{}\t{}\t{}\tloader_ver={}\tready={}\t{}",
                    i.id, i.name, i.version, i.loader, lv, i.version_ready, i.game_dir
                );
            }
        }
        "accounts" => {
            let store = load_accounts(&paths)?;
            for a in store.accounts {
                println!("{}\t{:?}\t{}\t{}", a.id, a.kind, a.username, a.uuid);
            }
        }
        "offline" => {
            let name = args.first().cloned().unwrap_or_else(|| "EasyPlayer".into());
            let acc = create_offline_account(&name);
            let mut store = load_accounts(&paths)?;
            store.active_id = Some(acc.id.clone());
            store.accounts.retain(|a| a.username != name);
            println!("created {} {}", acc.username, acc.uuid);
            store.accounts.push(acc);
            save_accounts(&paths, &store)?;
        }
        "search-mods" => {
            let source_s = args
                .first()
                .cloned()
                .ok_or_else(|| easymc_core::EasyMcError::msg(
                    "usage: search-mods <modrinth|curseforge> [query] [--game VER] [--loader fabric|quilt|vanilla|forge|neoforge]"
                ))?;
            let source = ModSource::parse(&source_s).ok_or_else(|| {
                easymc_core::EasyMcError::msg(format!("未知模组源: {source_s}（modrinth|curseforge）"))
            })?;
            let query_text = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with('-'))
                .cloned()
                .unwrap_or_default();
            let game = flag_value(&args, "--game");
            let loader = flag_value(&args, "--loader").and_then(|s| ModLoader::parse_loose(&s));
            let q = SearchQuery {
                text: query_text,
                game_version: game,
                loader,
                limit: 15,
                offset: 0,
            };
            let hits = search_mods(source, &q).await?;
            if hits.is_empty() {
                println!("(无结果)");
            }
            for h in hits {
                let author = h.author.as_deref().unwrap_or("-");
                println!(
                    "{}\t{}\t{}\tdl={}\t@{}",
                    h.id, h.slug, h.name, h.downloads, author
                );
                if !h.description.is_empty() {
                    let d = h.description.replace('\n', " ");
                    let short = if d.chars().count() > 80 {
                        format!("{}…", d.chars().take(80).collect::<String>())
                    } else {
                        d
                    };
                    println!("  {short}");
                }
            }
        }
        "list-mod-versions" => {
            let source_s = args.first().cloned().ok_or_else(|| {
                easymc_core::EasyMcError::msg(
                    "usage: list-mod-versions <modrinth|curseforge> <project-id> [--game VER] [--loader …]",
                )
            })?;
            let project = args.get(1).cloned().ok_or_else(|| {
                easymc_core::EasyMcError::msg("缺少 project-id")
            })?;
            let source = ModSource::parse(&source_s).ok_or_else(|| {
                easymc_core::EasyMcError::msg(format!("未知模组源: {source_s}"))
            })?;
            let game = flag_value(&args, "--game");
            let loader = flag_value(&args, "--loader").and_then(|s| ModLoader::parse_loose(&s));
            let list = list_mod_versions(source, &project, game.as_deref(), loader).await?;
            for v in list.into_iter().take(20) {
                let gvs = v.game_versions.join(",");
                let lds = v.loaders.join(",");
                println!(
                    "{}\t{}\t{}\tgames=[{gvs}]\tloaders=[{lds}]",
                    v.id, v.version_number, v.name
                );
            }
        }
        "install-mod" => {
            let source_s = args.first().cloned().ok_or_else(|| {
                easymc_core::EasyMcError::msg(
                    "usage: install-mod <modrinth|curseforge> <project-id> [--version ID] [--instance ID]",
                )
            })?;
            let project = args.get(1).cloned().ok_or_else(|| {
                easymc_core::EasyMcError::msg("缺少 project-id / slug")
            })?;
            let source = ModSource::parse(&source_s).ok_or_else(|| {
                easymc_core::EasyMcError::msg(format!("未知模组源: {source_s}"))
            })?;
            let version_id = flag_value(&args, "--version");
            let store = load_instances(&paths)?;
            let inst_id = flag_value(&args, "--instance")
                .or(store.active_id.clone())
                .ok_or_else(|| easymc_core::EasyMcError::msg("无活动实例，请 --instance 或 set-active"))?;
            let inst = store
                .instances
                .iter()
                .find(|i| i.id == inst_id)
                .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {inst_id}")))?
                .clone();
            let res = install_mod(
                &inst,
                source,
                &project,
                version_id.as_deref(),
                |p| {
                    let pct = p.percent.map(|x| format!("{x:.0}%")).unwrap_or_else(|| "—".into());
                    let tot = p.total.map(|t| format!("{}/{}", p.current, t)).unwrap_or_else(|| format!("{}", p.current));
                    eprintln!(
                        "[{}/{} {}] {pct}  {tot}  {}",
                        p.phase, p.phase_total, p.stage_label, p.detail
                    );
                },
            )
            .await?;
            println!(
                "OK {} {} @ {} → {}{}",
                res.source,
                res.project_name,
                res.version_number,
                res.path,
                if res.skipped { " (跳过)" } else { "" }
            );
        }
        "list-mods" => {
            let store = load_instances(&paths)?;
            let inst_id = args
                .first()
                .cloned()
                .or(flag_value(&args, "--instance"))
                .or(store.active_id.clone())
                .ok_or_else(|| easymc_core::EasyMcError::msg("无活动实例"))?;
            let inst = store
                .instances
                .iter()
                .find(|i| i.id == inst_id)
                .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {inst_id}")))?;
            let mods = list_installed_mods(inst)?;
            if mods.is_empty() {
                println!("(mods 目录为空) {}", inst.game_dir);
            } else {
                for m in mods {
                    println!("{m}");
                }
            }
        }
        "curseforge-key" => {
            match std::env::var(API_KEY_ENV) {
                Ok(k) if !k.trim().is_empty() => {
                    println!("已配置 {API_KEY_ENV}（长度 {}）", k.trim().len());
                }
                _ => {
                    println!("未配置 {API_KEY_ENV}");
                    println!("申请: https://console.curseforge.com/ → 设置环境变量后即可搜索/安装 CurseForge 模组。");
                    println!("Modrinth 无需此 Key。");
                }
            }
        }
        "launch" => {
            let store = load_instances(&paths)?;
            let id = args
                .first()
                .cloned()
                .or(store.active_id.clone())
                .unwrap_or_else(|| "default".into());
            let accounts = load_accounts(&paths)?;
            let settings = load_settings(&paths)?;
            let inst = store
                .instances
                .iter()
                .find(|i| i.id == id)
                .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {id}")))?;
            let acc_id = inst
                .account_id
                .clone()
                .or(accounts.active_id.clone())
                .ok_or_else(|| easymc_core::EasyMcError::msg("no account"))?;
            let acc = accounts
                .accounts
                .iter()
                .find(|a| a.id == acc_id)
                .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("account {acc_id}")))?;
            if !instance_launchable(&paths, inst) {
                eprintln!(
                    "not installed — run install / install-fabric for {} {}",
                    inst.loader, inst.version
                );
                return Err(easymc_core::EasyMcError::msg("not installed"));
            }
            let procs = Arc::new(ProcessManager::new());
            let status = prepare_and_launch(
                &paths,
                inst,
                acc,
                &settings,
                &LaunchRequest {
                    memory_mb: settings.memory_mb,
                    extra_jvm_args: vec![],
                },
                procs.clone(),
                |line| println!("{line}"),
            )
            .await?;
            println!("{}", status.message);
            while procs.is_running().await {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
        "create-instance" => {
            let name = positional_clean(&args, 0).unwrap_or_else(|| "New Instance".into());
            let ver = positional_clean(&args, 1).unwrap_or_else(|| "1.21.1".into());
            let loader = flag_value(&args, "--loader")
                .as_deref()
                .and_then(ModLoader::parse_loose)
                .unwrap_or(ModLoader::Vanilla);
            let loader_version = flag_value(&args, "--loader-version");
            let ready = match loader {
                ModLoader::Vanilla => is_version_installed(&paths, &ver),
                ModLoader::Fabric => loader_version
                    .as_deref()
                    .map(|lv| is_loader_installed(&paths, LoaderKind::Fabric, &ver, lv))
                    .unwrap_or(false),
                ModLoader::Quilt => loader_version
                    .as_deref()
                    .map(|lv| is_loader_installed(&paths, LoaderKind::Quilt, &ver, lv))
                    .unwrap_or(false),
                ModLoader::Forge => loader_version
                    .as_deref()
                    .map(|lv| is_loader_installed(&paths, LoaderKind::Forge, &ver, lv))
                    .unwrap_or(false),
                ModLoader::NeoForge => loader_version
                    .as_deref()
                    .map(|lv| is_loader_installed(&paths, LoaderKind::NeoForge, &ver, lv))
                    .unwrap_or(false),
            };
            let inst = create_named_instance_with(
                &paths,
                CreateInstanceOpts {
                    name,
                    version: McVersion(ver),
                    loader,
                    loader_version,
                    version_ready: ready,
                },
            )?;
            println!(
                "{}\t{}\t{}\t{}\tloader_ver={}\tready={}",
                inst.id,
                inst.name,
                inst.version,
                inst.loader,
                inst.loader_version.as_deref().unwrap_or("-"),
                inst.version_ready
            );
        }
        "set-active" => {
            let id = args
                .first()
                .cloned()
                .ok_or_else(|| easymc_core::EasyMcError::msg("usage: set-active <instance-id>"))?;
            set_active_instance(&paths, &id)?;
            println!("active={id}");
        }

        "agent" => {
            easymc_agent::llm::try_load_dotenv_llm();
            let sub = args.first().map(|s| s.as_str()).unwrap_or("help");
            match sub {
                "diagnose" | "diag" => {
                    let instance = flag_value(&args, "--instance")
                        .or_else(|| positional_clean(&args, 1));
                    let report = run_diagnose(&paths, instance, None);
                    println!("{}", report.summary_zh);
                    for f in &report.findings {
                        println!("\n[{:?}] {}", f.severity, f.title);
                        println!("  {}", f.summary);
                        for fix in &f.fixes {
                            println!("  - {fix}");
                        }
                        if let Some(ev) = &f.evidence {
                            println!("  证据: {ev}");
                        }
                    }
                    if args.iter().any(|a| a == "--bundle" || a == "--export") {
                        let ar = write_diagnostic_bundle(&paths, &report)?;
                        println!("\n{}", ar.message_zh);
                    }
                    if !report.sources.is_empty() {
                        println!("\nsources:");
                        for s in &report.sources {
                            println!("  {s}");
                        }
                    }
                }
                "ask" => {
                    // agent ask "…"  or agent ask -- 看上次报错
                    let text = if let Some(i) = args.iter().position(|a| a == "--") {
                        args[i + 1..].join(" ")
                    } else if args.len() > 1 {
                        // skip subcommand
                        args[1..]
                            .iter()
                            .filter(|a| !a.starts_with("--") || *a == "--rules-only")
                            .filter(|a| *a != "--rules-only")
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else {
                        String::new()
                    };
                    let text = text.trim().trim_matches('"').to_string();
                    if text.is_empty() {
                        return Err(easymc_core::EasyMcError::msg(
                            "usage: agent ask \"看上次报错\" | agent ask 确保 Java 21",
                        ));
                    }
                    let rules_only = args.iter().any(|a| a == "--rules-only");
                    let instance = flag_value(&args, "--instance");
                    let resp = ask(
                        &paths,
                        AskRequest {
                            text,
                            instance_id: instance,
                            process_logs: None,
                            rules_only,
                            history: vec![],
                        },
                    )
                    .await?;
                    println!("mode={} llm={}", resp.mode, resp.llm.mode);
                    println!("{}", resp.reply_zh);
                    if let Some(action) = &resp.action {
                        if action.action == "ensure_java" {
                            let major = action
                                .value
                                .as_ref()
                                .and_then(|v| v.get("major"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(21) as u32;
                            println!("→ running ensure-java {major} …");
                            let j = ensure_java(&paths, major, |s| println!("{s}")).await?;
                            println!("OK {}", j.path.display());
                        }
                    }
                }
                "status" => {
                    let st = llm_status();
                    println!(
                        "mode={} configured={} model={} host={}",
                        st.mode,
                        st.configured,
                        st.model.as_deref().unwrap_or("-"),
                        st.base_url_host.as_deref().unwrap_or("-")
                    );
                }
                "help" | _ => {
                    eprintln!(
                        "agent subcommands:\n\
                           agent diagnose [--instance ID] [--bundle]\n\
                           agent ask \"…\" [--instance ID] [--rules-only]\n\
                           agent status\n\
                         \n\
                         LLM (optional): source ~/.config/easymc/llm.env\n\
                           EASYMC_LLM_BASE_URL  EASYMC_LLM_API_KEY  EASYMC_LLM_MODEL\n\
                         Without LLM env, Agent stays fully offline (rules only)."
                    );
                }
            }
        }

        other => {
            eprintln!("unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
    }
    Ok(())
}

fn parse_loader_kind(s: &str) -> easymc_core::Result<LoaderKind> {
    match s.to_ascii_lowercase().as_str() {
        "fabric" => Ok(LoaderKind::Fabric),
        "quilt" => Ok(LoaderKind::Quilt),
        "forge" => Ok(LoaderKind::Forge),
        "neoforge" | "neo" => Ok(LoaderKind::NeoForge),
        _ => Err(easymc_core::EasyMcError::msg(
            "loader kind must be fabric|quilt|forge|neoforge",
        )),
    }
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn positional_clean(args: &[String], index: usize) -> Option<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            i += 2; // skip flag + value (best-effort)
            continue;
        }
        out.push(args[i].clone());
        i += 1;
    }
    out.get(index).cloned()
}

fn instance_launchable(paths: &Paths, inst: &easymc_core::Instance) -> bool {
    match inst.loader {
        ModLoader::Vanilla => {
            inst.version_ready || is_version_installed(paths, inst.version.as_str())
        }
        ModLoader::Fabric => {
            if let Some(lv) = inst.loader_version.as_deref() {
                is_loader_installed(paths, LoaderKind::Fabric, inst.version.as_str(), lv)
            } else {
                false
            }
        }
        ModLoader::Quilt => {
            if let Some(lv) = inst.loader_version.as_deref() {
                is_loader_installed(paths, LoaderKind::Quilt, inst.version.as_str(), lv)
            } else {
                false
            }
        }
        ModLoader::Forge => {
            if let Some(lv) = inst.loader_version.as_deref() {
                is_loader_installed(paths, LoaderKind::Forge, inst.version.as_str(), lv)
            } else {
                false
            }
        }
        ModLoader::NeoForge => {
            if let Some(lv) = inst.loader_version.as_deref() {
                is_loader_installed(paths, LoaderKind::NeoForge, inst.version.as_str(), lv)
            } else {
                false
            }
        }
    }
}

fn print_help() {
    eprintln!(
        "EasyMC CLI\n\
         \n\
         commands:\n\
           paths\n\
           java\n\
           ensure-java [major]\n\
           list-versions [filter|--snapshots]\n\
           list-loaders <fabric|quilt|forge|neoforge> [game-version]\n\
           install <version> [--no-assets]\n\
           install-fabric <game> [--loader VER] [--no-assets]\n\
           install-quilt <game> [--loader VER] [--no-assets]\n\
           install-forge <game> [--loader VER] [--no-assets]\n\
           install-neoforge <game> [--loader VER] [--no-assets]\n\
           instances\n\
           accounts\n\
           offline [username]\n\
           create-instance [name] [version] [--loader vanilla|fabric|quilt|forge|neoforge] [--loader-version VER]\n\
           set-active <instance-id>\n\
           search-mods <modrinth|curseforge> [query] [--game VER] [--loader …]\n\
           list-mod-versions <modrinth|curseforge> <project-id> [--game VER] [--loader …]\n\
           install-mod <modrinth|curseforge> <project-id> [--version ID] [--instance ID]\n\
           list-mods [instance-id]\n\
           curseforge-key\n\
           launch [instance-id]\n\
\n\
         CurseForge 需要环境变量 EASYMC_CURSEFORGE_API_KEY（https://console.curseforge.com/）。\n\
         Modrinth 无需 Key。\n"
    );
}
