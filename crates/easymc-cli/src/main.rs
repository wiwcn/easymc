use std::sync::Arc;

use easymc_core::{
    create_named_instance_with, create_offline_account, ensure_defaults, load_accounts,
    load_instances, load_settings, save_accounts, save_instances, save_settings,
    set_active_instance, CreateInstanceOpts, DownloadSource, McVersion, ModLoader, Paths,
};
use easymc_launch_desktop::{
    detect_java, ensure_java, prepare_and_launch, LaunchRequest, ProcessManager,
};
use easymc_meta::{
    install_fabric, install_forge, install_neoforge, install_quilt, install_version,
    is_loader_installed, is_version_installed, list_loaders_with_source, InstallOptions,
    LoaderKind, MetaProvider, PistonMetaProvider,
};
use easymc_agent::{apply_proposal, ask, llm_status, parse_proposal, run_diagnose, undo_proposal, write_diagnostic_bundle, AskRequest};
use easymc_share::{export_instance, import_instance, ExportOptions, ImportOptions, ShareMode};
use easymc_mods::{
    api_key, api_key_source, install_mod, list_installed_mods, list_mod_versions, remove_mod,
    search_and_install_best, search_mods, set_mod_disabled, sync_api_key_from_settings,
    ApiKeySource, ModSource, SearchQuery, API_KEY_ENV, CONSOLE_URL,
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
    {
        let settings = load_settings(&paths)?;
        sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
    }

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
            let source = load_settings(&paths)
                .map(|s| s.download_source)
                .unwrap_or_default();
            let p = PistonMetaProvider::with_source(source);
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
            let source = load_settings(&paths)
                .map(|s| s.download_source)
                .unwrap_or_default();
            let list = list_loaders_with_source(kind, &game, source).await?;
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
            let source = load_settings(&paths)
                .map(|s| s.download_source)
                .unwrap_or_default();
            install_version(
                &paths,
                &McVersion(id.clone()),
                InstallOptions {
                    assets: !skip_assets,
                    natives: true,
                    download_source: source,
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
            let source = load_settings(&paths)
                .map(|s| s.download_source)
                .unwrap_or_default();
            let vanilla = InstallOptions {
                assets: !skip_assets,
                natives: true,
                download_source: source,
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
            let acc = create_offline_account(&name)?;
            let mut store = load_accounts(&paths)?;
            store.active_id = Some(acc.id.clone());
            store.accounts.retain(|a| a.username != acc.username);
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
                .enumerate()
                .find(|(i, a)| {
                    !a.starts_with('-')
                        && !args.get(*i).is_some_and(|p| p.starts_with("--"))
                })
                .map(|(_, a)| a.clone())
                .unwrap_or_default();
            let game = flag_value(&args, "--game");
            let loader = flag_value(&args, "--loader").and_then(|s| ModLoader::parse_loose(&s));
            let q = SearchQuery {
                project_type: flag_value(&args, "--type")
                    .and_then(|s| easymc_mods::ProjectType::parse(&s))
                    .unwrap_or_default(),
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
        "install-modpack" => {
            let source = args.first().and_then(|s| ModSource::parse(s)).ok_or_else(|| {
                easymc_core::EasyMcError::msg("usage: install-modpack <modrinth|curseforge> <project> [--version ID] [--name NAME]")
            })?;
            let project = args.get(1).cloned().ok_or_else(|| easymc_core::EasyMcError::msg("缺少 project"))?;
            let res = easymc_mods::install_modpack(
                &paths,
                source,
                &project,
                flag_value(&args, "--version").as_deref(),
                flag_value(&args, "--name").as_deref(),
                |p| eprintln!("[{}/{} {}] {}/{} {}", p.phase, p.phase_total, p.stage_label, p.current, p.total.unwrap_or(0), p.detail),
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
        }
        "install-content" => {
            let usage = "usage: install-content <modrinth|curseforge> <resourcepack|shader|datapack> <project> [--world NAME] [--instance ID]";
            let source = args.first().and_then(|s| ModSource::parse(s)).ok_or_else(|| easymc_core::EasyMcError::msg(usage))?;
            let kind = args.get(1).and_then(|s| easymc_mods::ProjectType::parse(s)).ok_or_else(|| easymc_core::EasyMcError::msg(usage))?;
            let project = args.get(2).cloned().ok_or_else(|| easymc_core::EasyMcError::msg(usage))?;
            let store = load_instances(&paths)?;
            let inst_id = flag_value(&args, "--instance")
                .or(store.active_id.clone())
                .ok_or_else(|| easymc_core::EasyMcError::msg("无活动实例"))?;
            let inst = store.instances.iter().find(|i| i.id == inst_id).cloned()
                .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {inst_id}")))?;
            let res = easymc_mods::install_content(&inst, source, kind, &project, flag_value(&args, "--version").as_deref(), flag_value(&args, "--world").as_deref()).await?;
            println!("OK {} {} → {}{}", res.project_name, res.version_number, res.path, if res.skipped { " (已存在)" } else { "" });
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
                .iter()
                .find(|a| !a.starts_with('-'))
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
        "remove-mod" | "uninstall-mod" => {
            let filename = args
                .iter()
                .find(|a| !a.starts_with('-'))
                .cloned()
                .ok_or_else(|| {
                    easymc_core::EasyMcError::msg(
                        "usage: remove-mod <filename.jar> [--instance id]",
                    )
                })?;
            let store = load_instances(&paths)?;
            let inst_id = flag_value(&args, "--instance")
                .or(store.active_id.clone())
                .ok_or_else(|| {
                    easymc_core::EasyMcError::msg("无活动实例，请 --instance 或 set-active")
                })?;
            let inst = store
                .instances
                .iter()
                .find(|i| i.id == inst_id)
                .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {inst_id}")))?;
            let removed = remove_mod(inst, &filename)?;
            println!("OK removed {removed}");
        }
        "disable-mod" | "enable-mod" => {
            let disabled = cmd == "disable-mod";
            let filename = args
                .iter()
                .find(|a| !a.starts_with('-'))
                .cloned()
                .ok_or_else(|| {
                    easymc_core::EasyMcError::msg(
                        "usage: disable-mod|enable-mod <filename.jar> [--instance id]",
                    )
                })?;
            let store = load_instances(&paths)?;
            let inst_id = flag_value(&args, "--instance")
                .or(store.active_id.clone())
                .ok_or_else(|| {
                    easymc_core::EasyMcError::msg("无活动实例，请 --instance 或 set-active")
                })?;
            let inst = store
                .instances
                .iter()
                .find(|i| i.id == inst_id)
                .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {inst_id}")))?;
            let new_name = set_mod_disabled(inst, &filename, disabled)?;
            println!(
                "OK {} → {new_name}",
                if disabled { "disabled" } else { "enabled" }
            );
        }
        "curseforge-key" => {
            let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
            match sub {
                "status" | "--status" => {
                    match api_key_source() {
                        ApiKeySource::Env => {
                            let k = api_key().unwrap();
                            println!("已配置（环境变量 {API_KEY_ENV}，长度 {}）", k.len());
                        }
                        ApiKeySource::Settings => {
                            let k = api_key().unwrap();
                            println!("已配置（settings.json，长度 {}）", k.len());
                        }
                        ApiKeySource::None => {
                            println!("未配置 CurseForge API Key");
                            println!("申请: {CONSOLE_URL}");
                            println!("设置: curseforge-key set <key>  或在「设置」页粘贴");
                            println!("也可 export {API_KEY_ENV}=...");
                            println!("Modrinth 无需此 Key。");
                        }
                    }
                }
                "set" => {
                    let key = args.get(1).cloned().ok_or_else(|| {
                        easymc_core::EasyMcError::msg("usage: curseforge-key set <api-key>")
                    })?;
                    let mut settings = load_settings(&paths)?;
                    settings.curseforge_api_key = Some(key.clone());
                    save_settings(&paths, &settings)?;
                    sync_api_key_from_settings(Some(&key));
                    println!("OK 已写入 settings.json（长度 {}）", key.trim().len());
                    if std::env::var(API_KEY_ENV).ok().filter(|s| !s.trim().is_empty()).is_some() {
                        println!("注意: 环境变量 {API_KEY_ENV} 仍优先于 settings。");
                    }
                }
                "clear" | "unset" => {
                    let mut settings = load_settings(&paths)?;
                    settings.curseforge_api_key = None;
                    save_settings(&paths, &settings)?;
                    sync_api_key_from_settings(None);
                    println!("OK 已清除 settings.json 中的 CurseForge Key");
                    if std::env::var(API_KEY_ENV).ok().filter(|s| !s.trim().is_empty()).is_some() {
                        println!("环境变量 {API_KEY_ENV} 仍在生效。");
                    }
                }
                other => {
                    return Err(easymc_core::EasyMcError::msg(format!(
                        "未知子命令: {other}（status|set|clear）"
                    )));
                }
            }
        }
        "download-source" => {
            let sub = args.first().map(|s| s.as_str()).unwrap_or("status");
            match sub {
                "status" | "get" | "show" => {
                    let s = load_settings(&paths)?;
                    println!("{}\t{}", s.download_source.as_str(), s.download_source.label_zh());
                }
                "set" => {
                    let val = args.get(1).cloned().ok_or_else(|| {
                        easymc_core::EasyMcError::msg(
                            "usage: download-source set <official|bmclapi>",
                        )
                    })?;
                    let src = DownloadSource::parse(&val).ok_or_else(|| {
                        easymc_core::EasyMcError::msg(format!(
                            "未知下载源: {val}（official / bmclapi）"
                        ))
                    })?;
                    let mut settings = load_settings(&paths)?;
                    settings.download_source = src;
                    save_settings(&paths, &settings)?;
                    println!("OK {} ({})", src.as_str(), src.label_zh());
                }
                other => {
                    return Err(easymc_core::EasyMcError::msg(format!(
                        "未知子命令: {other}（status|set）"
                    )));
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


        "export-instance" => {
            let id = args
                .first()
                .cloned()
                .ok_or_else(|| easymc_core::EasyMcError::msg(
                    "usage: export-instance <instance-id> [--mrpack|--pack|--easymc-pack|--metadata] [--out PATH] [--notes TEXT] [--no-resolve]",
                ))?;
            let store = load_instances(&paths)?;
            let inst = store
                .instances
                .iter()
                .find(|i| i.id == id)
                .ok_or_else(|| easymc_core::EasyMcError::Missing(format!("instance {id}")))?;
            let mode = if args.iter().any(|a| a == "--easymc-pack") {
                ShareMode::Pack
            } else if args.iter().any(|a| a == "--pack" || a == "--mrpack") {
                ShareMode::Mrpack
            } else {
                ShareMode::Metadata
            };
            let output = flag_value(&args, "--out").map(std::path::PathBuf::from);
            let notes = flag_value(&args, "--notes");
            let resolve_modrinth = !args.iter().any(|a| a == "--no-resolve");
            let res = export_instance(
                &paths,
                inst,
                ExportOptions {
                    mode,
                    notes,
                    output,
                    resolve_modrinth,
                },
            )
            .await?;
            println!("{}", res.message_zh);
            println!("path={}", res.path);
        }
        "import-instance" => {
            let path = args
                .first()
                .cloned()
                .ok_or_else(|| easymc_core::EasyMcError::msg(
                    "usage: import-instance <file.json|file.zip|file.mrpack> [--name NAME] [--apply-memory] [--offline]",
                ))?;
            let name = flag_value(&args, "--name");
            let apply_memory = args.iter().any(|a| a == "--apply-memory");
            let offline = args.iter().any(|a| a == "--offline");
            let res = import_instance(
                &paths,
                std::path::Path::new(&path),
                ImportOptions {
                    name,
                    apply_memory,
                    offline,
                },
            )
            .await?;
            println!("{}", res.message_zh);
            if !res.mods_missing.is_empty() {
                println!("missing:");
                for m in &res.mods_missing {
                    println!("  - {m}");
                }
            }
            println!("instance={}", res.instance_id);
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
                        // skip flags (--rules-only, --instance ID) and keep the utterance
                        let mut parts = Vec::new();
                        let mut skip_next = false;
                        for a in args[1..].iter() {
                            if skip_next {
                                skip_next = false;
                                continue;
                            }
                            if a == "--rules-only" {
                                continue;
                            }
                            if a == "--instance" {
                                skip_next = true;
                                continue;
                            }
                            if a.starts_with("--instance=") {
                                continue;
                            }
                            if a.starts_with("--") {
                                continue;
                            }
                            parts.push(a.clone());
                        }
                        parts.join(" ")
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
                            instance_id: instance.clone(),
                            process_logs: None,
                            rules_only,
                            history: vec![],
                        },
                    )
                    .await?;
                    println!("mode={} llm={}", resp.mode, resp.llm.mode);
                    println!("{}", resp.reply_zh);
                    if let Some(tool) = &resp.tool {
                        println!(
                            "→ tool={} status={:?} title={}",
                            tool.name, tool.status, tool.title_zh
                        );
                        if !tool.args.is_empty() {
                            for a in &tool.args {
                                println!("  · {}: {}", a.label_zh, a.value);
                            }
                        }
                        if let Some(r) = &tool.result_zh {
                            if r != &resp.reply_zh {
                                println!("  result: {r}");
                            }
                        }
                    }
                    if let Some(action) = &resp.action {
                        if action.action == "propose_changes" {
                            println!("→ action=propose_changes (需确认后应用)");
                            if let Some(v) = &action.value {
                                println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
                            }
                        } else if action.action == "ensure_java" {
                            let major = action
                                .value
                                .as_ref()
                                .and_then(|v| v.get("major"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(21) as u32;
                            println!("→ 正在准备 Java {major} …");
                            match ensure_java(&paths, major, |s| println!("{s}")).await {
                                Ok(j) => println!(
                                    "✓ Java {} 已就绪：{} ({})",
                                    j.major,
                                    j.path.display(),
                                    j.source
                                ),
                                Err(e) => println!("✗ 准备 Java {major} 失败：{e}"),
                            }
                        } else if action.action == "install_mod" {
                            let query = action
                                .value
                                .as_ref()
                                .and_then(|v| v.get("query"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            let store = load_instances(&paths)?;
                            let id = instance
                                .clone()
                                .filter(|s| !s.is_empty())
                                .or(store.active_id.clone());
                            let Some(id) = id else {
                                println!("✗ 还没有选版本，没法安装「{query}」。可用 --instance ID");
                                println!("→ 或在桌面端「下载」页搜索：{query}");
                                return Ok(());
                            };
                            let Some(inst) = store.instances.iter().find(|i| i.id == id).cloned()
                            else {
                                println!("✗ 找不到版本 {id}");
                                return Ok(());
                            };
                            if matches!(inst.loader, ModLoader::Vanilla) {
                                println!(
                                    "✗ 版本「{}」是原版，通常装不了模组。请换成带加载器的版本。",
                                    inst.name
                                );
                                return Ok(());
                            }
                            println!("→ 正在搜索并安装「{query}」到「{}」…", inst.name);
                            match search_and_install_best(&inst, &query, |p| {
                                if let Some(pct) = p.percent {
                                    println!("  [{}] {:.0}% {}", p.stage_label, pct, p.detail);
                                } else {
                                    println!("  [{}] {}", p.stage_label, p.detail);
                                }
                            })
                            .await
                            {
                                Ok(r) => {
                                    let skipped = if r.skipped { "（已存在）" } else { "" };
                                    println!(
                                        "✓ 已安装「{}」{} → {}",
                                        r.project_name, skipped, r.path
                                    );
                                }
                                Err(e) => {
                                    println!("✗ 安装「{query}」失败：{e}");
                                    println!("→ 可在桌面端「下载」页手动搜索：{query}");
                                }
                            }
                        } else if action.action == "open_store" {
                            if let Some(q) = action
                                .value
                                .as_ref()
                                .and_then(|v| v.get("query"))
                                .and_then(|v| v.as_str())
                            {
                                println!("→ 请在桌面端「下载」页搜索：{q}");
                            }
                        } else if action.action == "setup_env" {
                            let major = action
                                .value
                                .as_ref()
                                .and_then(|v| v.get("major"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(21) as u32;
                            println!("→ 一键环境：准备 Java {major} …");
                            match ensure_java(&paths, major, |s| println!("{s}")).await {
                                Ok(j) => println!(
                                    "✓ Java {} 已就绪：{} ({})",
                                    j.major,
                                    j.path.display(),
                                    j.source
                                ),
                                Err(e) => println!("✗ 准备 Java {major} 失败：{e}"),
                            }
                        } else if action.action == "build_pack" {
                            let id = action
                                .value
                                .as_ref()
                                .and_then(|v| v.get("instance_id"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .filter(|s| !s.is_empty())
                                .or(instance.clone())
                                .or_else(|| load_instances(&paths).ok().and_then(|s| s.active_id));
                            let Some(id) = id else {
                                println!("✗ 还没有选版本，没法生成整合包（可用 --instance ID）");
                                return Ok(());
                            };
                            let store = load_instances(&paths)?;
                            let Some(inst) = store.instances.iter().find(|i| i.id == id).cloned() else {
                                println!("✗ 找不到版本 {id}");
                                return Ok(());
                            };
                            println!("→ 正在导出 .mrpack「{}」…", inst.name);
                            match export_instance(
                                &paths,
                                &inst,
                                ExportOptions {
                                    mode: ShareMode::Mrpack,
                                    notes: Some("CLI agent 制作整合包".into()),
                                    output: None,
                                    resolve_modrinth: true,
                                },
                            )
                            .await
                            {
                                Ok(r) => println!("✓ {}\n  路径：{}", r.message_zh, r.path),
                                Err(e) => println!("✗ 导出失败：{e}"),
                            }
                        } else if action.action == "search_mods" {
                            let query = action
                                .value
                                .as_ref()
                                .and_then(|v| v.get("query"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            if query.is_empty() {
                                println!("✗ 请提供关键词，例如：agent ask \"搜模组 sodium\"");
                            } else {
                                if let Ok(settings) = load_settings(&paths) {
                                    sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
                                }
                                let store = load_instances(&paths).ok();
                                let id = instance
                                    .clone()
                                    .filter(|s| !s.is_empty())
                                    .or_else(|| store.as_ref().and_then(|s| s.active_id.clone()));
                                let inst = id.and_then(|id| {
                                    store.as_ref().and_then(|s| {
                                        s.instances.iter().find(|i| i.id == id).cloned()
                                    })
                                });
                                let game_version = inst.as_ref().map(|i| i.version.0.clone());
                                let loader = inst.as_ref().and_then(|i| {
                                    if matches!(i.loader, ModLoader::Vanilla) {
                                        None
                                    } else {
                                        Some(i.loader)
                                    }
                                });
                                let mut q = SearchQuery {
                                    project_type: Default::default(),
                                    text: query.clone(),
                                    game_version,
                                    loader,
                                    limit: 8,
                                    offset: 0,
                                };
                                println!("→ 搜索模组「{query}」…");
                                let mut hits = search_mods(ModSource::Modrinth, &q)
                                    .await
                                    .unwrap_or_default();
                                if hits.is_empty() && (q.game_version.is_some() || q.loader.is_some()) {
                                    q.game_version = None;
                                    q.loader = None;
                                    hits = search_mods(ModSource::Modrinth, &q)
                                        .await
                                        .unwrap_or_default();
                                }
                                if api_key().is_some() {
                                    if let Ok(cf) = search_mods(ModSource::CurseForge, &q).await {
                                        for p in cf {
                                            if !hits.iter().any(|h| {
                                                h.slug.eq_ignore_ascii_case(&p.slug)
                                                    || h.name.eq_ignore_ascii_case(&p.name)
                                            }) {
                                                hits.push(p);
                                            }
                                        }
                                    }
                                }
                                hits.sort_by(|a, b| b.downloads.cmp(&a.downloads));
                                if hits.is_empty() {
                                    println!("✗ 没搜到「{query}」");
                                } else {
                                    println!("✓ 找到 {} 个结果：", hits.len().min(8));
                                    for (i, p) in hits.iter().take(8).enumerate() {
                                        println!(
                                            "  {}. {} ({}) · {} 下载",
                                            i + 1,
                                            p.name,
                                            p.source.label_zh(),
                                            p.downloads
                                        );
                                    }
                                    println!("→ 安装：agent ask \"装 <名字>\"");
                                }
                            }
                        } else if action.action == "suggest_mods" {
                            let theme = action
                                .value
                                .as_ref()
                                .and_then(|v| v.get("theme"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("survival");
                            let (qtext, theme_zh) = match theme.trim().to_ascii_lowercase().as_str() {
                                "tech" | "科技" | "工业" => ("create", "科技"),
                                "magic" | "魔法" => ("botania", "魔法"),
                                "performance" | "性能" | "优化" => ("sodium", "性能"),
                                "qos" | "生活质量" | "便利" => ("jade", "生活质量"),
                                "adventure" | "冒险" => ("origins", "冒险"),
                                "building" | "建筑" => ("chisel", "建筑"),
                                "shader" | "光影" => ("iris", "光影"),
                                _ => ("farmers-delight", "生存"),
                            };
                            if let Ok(settings) = load_settings(&paths) {
                                sync_api_key_from_settings(settings.curseforge_api_key.as_deref());
                            }
                            let store = load_instances(&paths).ok();
                            let id = instance
                                .clone()
                                .filter(|s| !s.is_empty())
                                .or_else(|| store.as_ref().and_then(|s| s.active_id.clone()));
                            let inst = id.and_then(|id| {
                                store.as_ref().and_then(|s| {
                                    s.instances.iter().find(|i| i.id == id).cloned()
                                })
                            });
                            let game_version = inst.as_ref().map(|i| i.version.0.clone());
                            let loader = inst.as_ref().and_then(|i| {
                                if matches!(i.loader, ModLoader::Vanilla) {
                                    None
                                } else {
                                    Some(i.loader)
                                }
                            });
                            let q = SearchQuery {
                                project_type: Default::default(),
                                text: qtext.into(),
                                game_version,
                                loader,
                                limit: 8,
                                offset: 0,
                            };
                            println!("→ 推荐「{theme_zh}」主题模组…");
                            let mut hits = search_mods(ModSource::Modrinth, &q)
                                .await
                                .unwrap_or_default();
                            if hits.is_empty() && (q.game_version.is_some() || q.loader.is_some()) {
                                let broad = SearchQuery {
                                    project_type: q.project_type,
                                    text: q.text.clone(),
                                    game_version: None,
                                    loader: None,
                                    limit: q.limit,
                                    offset: 0,
                                };
                                hits = search_mods(ModSource::Modrinth, &broad)
                                    .await
                                    .unwrap_or_default();
                            }
                            hits.sort_by(|a, b| b.downloads.cmp(&a.downloads));
                            if hits.is_empty() {
                                println!("✗ 暂无推荐");
                            } else {
                                println!("✓ 「{theme_zh}」推荐：");
                                for (i, p) in hits.iter().take(8).enumerate() {
                                    println!(
                                        "  {}. {} ({}) · {} 下载",
                                        i + 1,
                                        p.name,
                                        p.source.label_zh(),
                                        p.downloads
                                    );
                                }
                            }
                        } else if matches!(
                            action.action.as_str(),
                            "set_download_source"
                                | "check_mods"
                                | "open_instance_dir"
                                | "bump_memory"
                                | "diagnostics"
                                | "show_data_dir"
                                | "apply_changes"
                                | "undo_changes"
                        ) {
                            println!("→ action={}", action.action);
                            if let Some(v) = &action.value {
                                println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
                            }
                        }
                    }
                }
                "apply" => {
                    // agent apply <proposal.json path or inline JSON>
                    let raw = args.get(1).cloned().unwrap_or_default();
                    let json = if raw.starts_with('{') {
                        raw
                    } else if !raw.is_empty() {
                        std::fs::read_to_string(&raw)?
                    } else {
                        return Err(easymc_core::EasyMcError::msg(
                            "usage: agent apply <proposal.json|'{...}'>",
                        ));
                    };
                    let value: serde_json::Value = serde_json::from_str(&json)?;
                    let prop = parse_proposal(&value).ok_or_else(|| {
                        easymc_core::EasyMcError::msg("invalid proposal JSON")
                    })?;
                    let ar = apply_proposal(&paths, &prop)?;
                    println!("{}", ar.message_zh);
                }
                "undo" => {
                    let raw = args.get(1).cloned().unwrap_or_default();
                    let json = if raw.starts_with('{') {
                        raw
                    } else if !raw.is_empty() {
                        std::fs::read_to_string(&raw)?
                    } else {
                        return Err(easymc_core::EasyMcError::msg(
                            "usage: agent undo <proposal.json|'{...}'>",
                        ));
                    };
                    let value: serde_json::Value = serde_json::from_str(&json)?;
                    let prop = parse_proposal(&value).ok_or_else(|| {
                        easymc_core::EasyMcError::msg("invalid proposal JSON")
                    })?;
                    let ar = undo_proposal(&paths, &prop)?;
                    println!("{}", ar.message_zh);
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
                           agent apply <proposal.json>\n\
                           agent undo <proposal.json>\n\
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
           install-modpack <modrinth|curseforge> <project> [--name N]\n\
           install-content <modrinth|curseforge> <resourcepack|shader|datapack> <project> [--world W]\n\
           search-mods <modrinth|curseforge> [query] [--type mod|modpack|resourcepack|shader|datapack] [--game VER] [--loader …]\n\
           list-mod-versions <modrinth|curseforge> <project-id> [--game VER] [--loader …]\n\
           install-mod <modrinth|curseforge> <project-id> [--version ID] [--instance ID]\n\
           list-mods [instance-id]\n\
           remove-mod <filename.jar> [--instance id]\n\
           disable-mod <filename.jar> [--instance id]\n\
           enable-mod <filename.jar> [--instance id]\n\
           curseforge-key [status|set <key>|clear]\n\
           download-source [status|set <official|bmclapi>]\n\
           launch [instance-id]\n\
           export-instance <id> [--pack|--metadata] [--out PATH] [--notes TEXT] [--no-resolve]\n\
           import-instance <file.json|zip> [--name NAME] [--apply-memory] [--offline]\n\
\n\
         下载源：设置页「下载源」，或 download-source set bmclapi（镜像失败自动回退官方）。\n\
         CurseForge：设置页粘贴 Key，或 curseforge-key set <key>，或环境变量 EASYMC_CURSEFORGE_API_KEY（https://console.curseforge.com/）。\n\
         Modrinth 无需 Key。\n\
         分享包：元数据 JSON 或含 mods/*.jar 的 zip；默认写入 $EASYMC_DATA_DIR/shares/。\n"
    );
}
