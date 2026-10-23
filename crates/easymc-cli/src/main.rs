use std::sync::Arc;

use easymc_core::{
    create_offline_account, ensure_defaults, load_accounts, load_instances, load_settings,
    save_accounts, save_instances, upsert_instance, McVersion, Paths,
};
use easymc_launch_desktop::{detect_java, ensure_java, prepare_and_launch, LaunchRequest, ProcessManager};
use easymc_meta::{install_version, is_version_installed, InstallOptions, PistonMetaProvider, MetaProvider};

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
                println!("{}  major={}  ({})  {}", j.path.display(), j.major, j.source, j.version_string);
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
            let filter = args.first().cloned();
            for v in list.into_iter().filter(|v| {
                filter.as_ref().map(|f| v.id.contains(f.as_str()) || v.version_type == *f).unwrap_or(v.version_type == "release")
            }).take(40) {
                println!("{}\t{}\t{}", v.id, v.version_type, if is_version_installed(&paths, &v.id) { "installed" } else { "-" });
            }
        }
        "install" => {
            let id = args.first().ok_or_else(|| easymc_core::EasyMcError::msg("usage: install <version>"))?;
            let skip_assets = args.iter().any(|a| a == "--no-assets");
            install_version(
                &paths,
                &McVersion(id.clone()),
                InstallOptions {
                    assets: !skip_assets,
                    natives: true,
                },
                |p| {
                    println!("[{}] {}/ {:?} — {}", p.stage, p.current, p.total, p.detail);
                },
            )
            .await?;
            // mark instance ready if matching
            let mut store = load_instances(&paths)?;
            for inst in &mut store.instances {
                if inst.version.0 == *id {
                    inst.version_ready = true;
                }
            }
            save_instances(&paths, &store)?;
            println!("installed {id}");
        }
        "instances" => {
            let store = load_instances(&paths)?;
            for i in store.instances {
                println!(
                    "{}\t{}\t{}\tready={}\t{}",
                    i.id, i.name, i.version, i.version_ready, i.game_dir
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
            let id = args.first().cloned().unwrap_or_else(|| "default".into());
            let store = load_instances(&paths)?;
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
            if !inst.version_ready && !is_version_installed(&paths, inst.version.as_str()) {
                eprintln!("version not installed — run: easymc install {}", inst.version);
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
            // wait until exit
            while procs.is_running().await {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
        "create-instance" => {
            let name = args.first().cloned().unwrap_or_else(|| "New Instance".into());
            let ver = args.get(1).cloned().unwrap_or_else(|| "1.21.1".into());
            let id = format!(
                "inst-{}",
                chrono::Utc::now().timestamp()
            );
            let game_dir = paths.instance_game_dir(&id);
            std::fs::create_dir_all(&game_dir)?;
            let mut inst = easymc_core::Instance::new(id, name, McVersion(ver), game_dir.to_string_lossy().into());
            let accounts = load_accounts(&paths)?;
            inst.account_id = accounts.active_id;
            inst.version_ready = is_version_installed(&paths, inst.version.as_str());
            upsert_instance(&paths, inst)?;
            println!("ok");
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "EasyMC CLI\n\
         \n\
         commands:\n\
           paths\n\
           java\n\
           ensure-java [major]\n\
           list-versions [filter]\n\
           install <version> [--no-assets]\n\
           instances\n\
           accounts\n\
           offline [username]\n\
           create-instance [name] [version]\n\
           launch [instance-id]\n"
    );
}
