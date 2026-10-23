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
    install_fabric, install_quilt, install_version, is_loader_installed, is_version_installed,
    list_loaders, InstallOptions, LoaderKind, MetaProvider, PistonMetaProvider,
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
        "install-fabric" | "install-quilt" => {
            let kind = if cmd == "install-quilt" {
                LoaderKind::Quilt
            } else {
                LoaderKind::Fabric
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
            };
            let mut store = load_instances(&paths)?;
            let loader_enum = match kind {
                LoaderKind::Fabric => ModLoader::Fabric,
                LoaderKind::Quilt => ModLoader::Quilt,
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
                ModLoader::Forge | ModLoader::NeoForge => false,
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
        _ => Err(easymc_core::EasyMcError::msg(
            "loader kind must be fabric|quilt",
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
        ModLoader::Forge | ModLoader::NeoForge => false,
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
           list-loaders <fabric|quilt> [game-version]\n\
           install <version> [--no-assets]\n\
           install-fabric <game> [--loader VER] [--no-assets]\n\
           install-quilt <game> [--loader VER] [--no-assets]\n\
           instances\n\
           accounts\n\
           offline [username]\n\
           create-instance [name] [version] [--loader vanilla|fabric|quilt] [--loader-version VER]\n\
           set-active <instance-id>\n\
           launch [instance-id]\n"
    );
}
