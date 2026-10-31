//! Live Podman rootless E2E for EasyMC container mode.
//!
//! Run:
//!   EASYMC_DATA_DIR=/tmp/easymc-podman-e2e \
//!   cargo run -p easymc-container --example podman_live_e2e -- \
//!     /workspace/easymc-shots/podman-20261031
//!
//! Exit 0 = all critical steps passed; non-zero = at least one failure.
//! Writes checklist.json + step logs under the out dir.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use easymc_container::{
    apply_game_layer, apply_one_click_setup, build_env_manifest, create_container_version,
    default_volumes, delete_container_version, discard_game_layer,
    ensure_runtime_image_for_profile, export_env_pack, gui_mount_hints, layer_state_for,
    list_container_versions, plan_container_launch, plan_without_replace,
    preheat_container_runtime, probe_container_runtime, probe_registry_mirrors,
    provision_container_version, repair_container_runtime_with_profile, resolve_agent_workspace,
    EnvExportOptions, EnvPackMode, MirrorProfile, ProbeStatus, DEFAULT_BASE_IMAGE,
    DEFAULT_BASE_IMAGE_MIRROR,
};
use easymc_core::{ensure_defaults, load_instances, McVersion, ModLoader, Paths};

struct Step {
    id: String,
    title: String,
    ok: bool,
    detail: String,
    ms: u128,
}

fn log_step(out: &Path, steps: &mut Vec<Step>, id: &str, title: &str, ok: bool, detail: &str, ms: u128) {
    let line = format!(
        "[{}] {} — {} ({} ms)\n{}\n---\n",
        if ok { "PASS" } else { "FAIL" },
        id,
        title,
        ms,
        detail
    );
    print!("{line}");
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("e2e.log"))
        .and_then(|mut f| f.write_all(line.as_bytes()));
    steps.push(Step {
        id: id.into(),
        title: title.into(),
        ok,
        detail: detail.into(),
        ms,
    });
}

fn run_capture(cmd: &str, args: &[&str]) -> (bool, String) {
    match Command::new(cmd).args(args).output() {
        Ok(o) => {
            let mut s = String::new();
            s.push_str(&String::from_utf8_lossy(&o.stdout));
            if !o.stderr.is_empty() {
                if !s.is_empty() {
                    s.push('\n');
                }
                s.push_str(&String::from_utf8_lossy(&o.stderr));
            }
            (o.status.success(), s.trim().to_string())
        }
        Err(e) => (false, e.to_string()),
    }
}

#[tokio::main]
async fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/workspace/easymc-shots/podman-20261031".into()),
    );
    let _ = fs::create_dir_all(&out);
    let _ = fs::write(out.join("e2e.log"), "");

    // Isolate EasyMC data for this run.
    if std::env::var_os("EASYMC_DATA_DIR").is_none() {
        let data = out.join("easymc-data");
        let _ = fs::create_dir_all(&data);
        std::env::set_var("EASYMC_DATA_DIR", &data);
    }

    let mut steps: Vec<Step> = Vec::new();
    let t0 = Instant::now();

    // 1) Podman binary / version
    {
        let t = Instant::now();
        let (ok, detail) = run_capture("podman", &["--version"]);
        log_step(
            &out,
            &mut steps,
            "podman_version",
            "Podman rootless binary present",
            ok,
            &detail,
            t.elapsed().as_millis(),
        );
        let _ = fs::write(out.join("01-podman-version.txt"), &detail);
    }

    // 1b) podman info
    {
        let t = Instant::now();
        let (ok, detail) = run_capture("podman", &["info", "--format", "json"]);
        let short = if detail.len() > 4000 {
            format!("{}…(truncated)", &detail[..4000])
        } else {
            detail.clone()
        };
        log_step(
            &out,
            &mut steps,
            "podman_info",
            "podman info (rootless ready)",
            ok,
            &short,
            t.elapsed().as_millis(),
        );
        let _ = fs::write(out.join("02-podman-info.json"), &detail);
    }

    // 2) detect / probe via crate
    {
        let t = Instant::now();
        let probe = probe_container_runtime();
        let ready = matches!(probe.status, ProbeStatus::Ready);
        let rootless_ok = probe
            .podman
            .as_ref()
            .and_then(|p| p.rootless)
            .unwrap_or(false);
        let version_ok = probe
            .podman
            .as_ref()
            .map(|p| p.version.to_ascii_lowercase().contains("5."))
            .unwrap_or(false);
        // Player message stays brand-free; truth lives on probe.podman.
        let msg_ok = !probe.message_zh.is_empty()
            && !probe.message_zh.to_ascii_lowercase().contains("podman");
        let ok = ready && rootless_ok && version_ok && msg_ok;
        let detail = format!(
            "ready={} rootless_ok={} version_ok={} msg_ok={} message_zh={}\n{}",
            ready,
            rootless_ok,
            version_ok,
            msg_ok,
            probe.message_zh,
            serde_json::to_string_pretty(&probe).unwrap_or_else(|e| e.to_string())
        );
        log_step(
            &out,
            &mut steps,
            "detect_probe",
            "probe_container_runtime → Ready + rootless/version match reality",
            ok,
            &detail,
            t.elapsed().as_millis(),
        );
        let _ = fs::write(out.join("03-probe.json"), &detail);
    }

    // 3) one-click setup (registries)
    {
        let t = Instant::now();
        let setup = apply_one_click_setup(true);
        let ok = setup.ok && matches!(setup.probe.status, ProbeStatus::Ready);
        let detail = serde_json::to_string_pretty(&setup).unwrap_or_else(|e| e.to_string());
        log_step(
            &out,
            &mut steps,
            "one_click_setup",
            "apply_one_click_setup (China registries)",
            ok,
            &detail,
            t.elapsed().as_millis(),
        );
        let _ = fs::write(out.join("04-setup.json"), &detail);
    }

    // 4) registry HTTPS probe + auto-pick assertion (slow China A must not win)
    let mut picked_profile = MirrorProfile::Daocloud;
    let mut prefer_china = true;
    {
        let t = Instant::now();
        let probes = probe_registry_mirrors().await;
        let any_ok = probes.iter().any(|p| p.ok);
        let detail = serde_json::to_string_pretty(&probes).unwrap_or_else(|e| e.to_string());
        let _ = fs::write(out.join("05-registry-probe.json"), &detail);

        let recommended = probes.iter().find(|p| p.recommended);
        let fastest_ok = probes
            .iter()
            .filter(|p| p.ok)
            .min_by_key(|p| p.latency_ms.unwrap_or(u32::MAX));
        let pick_ok = match (recommended, fastest_ok) {
            (Some(r), Some(f)) => r.id == f.id,
            _ => false,
        };
        // Auto-pick must match true min latency (regression: slow daocloud stayed ★).
        let not_stale_china_a = match (recommended, fastest_ok) {
            (Some(r), Some(f))
                if r.id == "daocloud" && f.id != "daocloud" =>
            {
                false
            }
            _ => true,
        };
        let ok = any_ok && pick_ok && not_stale_china_a;
        let extra = format!(
            "recommended={:?} fastest_ok={:?} pick_ok={} not_stale_china_a={}\n{}",
            recommended.map(|r| (&r.id, r.latency_ms)),
            fastest_ok.map(|r| (&r.id, r.latency_ms)),
            pick_ok,
            not_stale_china_a,
            detail
        );
        log_step(
            &out,
            &mut steps,
            "registry_probe",
            "probe_registry_mirrors + auto-pick ★ == min latency",
            ok,
            &extra,
            t.elapsed().as_millis(),
        );

        if let Some(r) = recommended {
            if let Some(p) = MirrorProfile::parse(&r.id) {
                picked_profile = p;
                prefer_china = p.prefer_china();
            }
        } else if let Some(f) = fastest_ok {
            if let Some(p) = MirrorProfile::parse(&f.id) {
                picked_profile = p;
                prefer_china = p.prefer_china();
            }
        }
        let _ = fs::write(
            out.join("05b-picked-profile.txt"),
            format!("{:?} prefer_china={}\n", picked_profile, prefer_china),
        );
    }

    // 5) pull / rewrite image for the auto-picked profile
    {
        let t = Instant::now();
        let result = ensure_runtime_image_for_profile(picked_profile);
        let (ok, detail) = match &result {
            Ok(r) => (true, serde_json::to_string_pretty(r).unwrap_or_default()),
            Err(e) => {
                // Fallback: try official if China mirror pull fails on this box.
                match ensure_runtime_image_for_profile(MirrorProfile::Official) {
                    Ok(r) => {
                        picked_profile = MirrorProfile::Official;
                        prefer_china = false;
                        (true, format!("fallback official after err={e}\n{}", serde_json::to_string_pretty(&r).unwrap_or_default()))
                    }
                    Err(e2) => (false, format!("{e} / fallback: {e2}")),
                }
            }
        };
        log_step(
            &out,
            &mut steps,
            "pull_image",
            &format!(
                "ensure_runtime_image_for_profile({:?}) mirror={} official={}",
                picked_profile, DEFAULT_BASE_IMAGE_MIRROR, DEFAULT_BASE_IMAGE
            ),
            ok,
            &detail,
            t.elapsed().as_millis(),
        );
        let _ = fs::write(out.join("06-pull-image.json"), &detail);
    }

    // 6) create container version + provision
    let paths = match Paths::discover() {
        Ok(p) => {
            let _ = p.ensure();
            let _ = ensure_defaults(&p);
            p
        }
        Err(e) => {
            log_step(
                &out,
                &mut steps,
                "paths",
                "Paths::discover",
                false,
                &e.to_string(),
                0,
            );
            write_checklist(&out, &steps, t0.elapsed().as_millis());
            std::process::exit(1);
        }
    };

    let mut instance_id: Option<String> = None;
    {
        let t = Instant::now();
        let created = create_container_version(
            &paths,
            "podman-e2e-vanilla",
            McVersion("1.21.1".into()),
            ModLoader::Vanilla,
            None,
            prefer_china,
        );
        match created {
            Ok(inst) => {
                let provisioned = inst
                    .container
                    .as_ref()
                    .map(|c| c.provisioned)
                    .unwrap_or(false);
                let detail = format!(
                    "id={} name={} provisioned={} container={:?}",
                    inst.id, inst.name, provisioned, inst.container
                );
                // provision explicitly (pull + recreate)
                let prov = provision_container_version(&paths, &inst.id, prefer_china);
                let (ok2, detail2) = match prov {
                    Ok(r) => (
                        r.instance
                            .container
                            .as_ref()
                            .map(|c| c.provisioned)
                            .unwrap_or(false),
                        format!("{detail}\nprovision: {}", r.message_zh),
                    ),
                    Err(e) => (false, format!("{detail}\nprovision err: {e}")),
                };
                instance_id = Some(inst.id.clone());
                log_step(
                    &out,
                    &mut steps,
                    "create_version",
                    "create + provision container version",
                    ok2,
                    &detail2,
                    t.elapsed().as_millis(),
                );
                let _ = fs::write(out.join("07-create-version.txt"), &detail2);
            }
            Err(e) => {
                log_step(
                    &out,
                    &mut steps,
                    "create_version",
                    "create + provision container version",
                    false,
                    &e.to_string(),
                    t.elapsed().as_millis(),
                );
            }
        }
    }

    // 7) mount plan + smoke start
    if let Some(ref id) = instance_id {
        let t = Instant::now();
        let store = load_instances(&paths).expect("instances");
        let inst = store
            .instances
            .iter()
            .find(|i| &i.id == id)
            .cloned()
            .expect("instance");
        let vols = default_volumes(&paths, &inst);
        let _ = fs::write(
            out.join("08-volumes.json"),
            serde_json::to_string_pretty(&vols).unwrap_or_default(),
        );

        // Ensure host dirs exist for mounts
        for v in &vols {
            let _ = fs::create_dir_all(&v.host);
        }
        // Seed a marker file in game dir
        let marker = PathBuf::from(&inst.game_dir).join("easymc-e2e-marker.txt");
        let _ = fs::write(&marker, "hello-from-host");

        let java_args = vec!["-version".to_string()];
        let plan = plan_container_launch(&paths, &inst, &java_args, prefer_china, false);
        match plan {
            Ok(plan) => {
                let plan2 = plan_without_replace(&plan);
                let _ = fs::write(
                    out.join("09-launch-plan.json"),
                    serde_json::to_string_pretty(&plan2).unwrap_or_default(),
                );
                // Smoke: run image java -version with same mounts (not full MC)
                let mut args: Vec<String> = vec![
                    "run".into(),
                    "--rm".into(),
                    "--name".into(),
                    format!("{}-smoke", plan2.container_name),
                    "-w".into(),
                    plan2.workdir.clone(),
                ];
                for v in &vols {
                    args.push("-v".into());
                    args.push(format!("{}:{}:{}", v.host, v.container, v.mode));
                }
                args.push(plan2.image.clone());
                args.push("java".into());
                args.push("-version".into());
                let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                let (ok, detail) = run_capture(&plan2.podman, &arg_refs);
                // Also verify mount: cat marker inside container
                let mut args_m: Vec<String> = vec![
                    "run".into(),
                    "--rm".into(),
                    "-w".into(),
                    "/data/.minecraft".into(),
                ];
                for v in &vols {
                    args_m.push("-v".into());
                    args_m.push(format!("{}:{}:{}", v.host, v.container, v.mode));
                }
                args_m.push(plan2.image.clone());
                args_m.push("cat".into());
                args_m.push("/data/.minecraft/easymc-e2e-marker.txt".into());
                let arg_refs_m: Vec<&str> = args_m.iter().map(|s| s.as_str()).collect();
                let (ok_m, detail_m) = run_capture(&plan2.podman, &arg_refs_m);
                let combined = format!(
                    "java -version ok={ok}\n{detail}\n\nmarker mount ok={ok_m} body={detail_m:?}\ngui={:?}",
                    gui_mount_hints()
                );
                let pass = ok && ok_m && detail_m.contains("hello-from-host");
                log_step(
                    &out,
                    &mut steps,
                    "smoke_start",
                    "mount plan + smoke java -version + volume check",
                    pass,
                    &combined,
                    t.elapsed().as_millis(),
                );
                let _ = fs::write(out.join("10-smoke.txt"), &combined);
            }
            Err(e) => {
                log_step(
                    &out,
                    &mut steps,
                    "smoke_start",
                    "mount plan + smoke",
                    false,
                    &e,
                    t.elapsed().as_millis(),
                );
            }
        }
    } else {
        log_step(
            &out,
            &mut steps,
            "smoke_start",
            "mount plan + smoke",
            false,
            "skipped — no instance",
            0,
        );
    }

    // 8) Agent layer apply/discard
    if let Some(ref id) = instance_id {
        let t = Instant::now();
        let store = load_instances(&paths).unwrap();
        let inst = store.instances.iter().find(|i| &i.id == id).unwrap().clone();
        let ws = resolve_agent_workspace(&paths, id).map_err(|e| e.to_string());
        let game = PathBuf::from(&inst.game_dir);
        let mods = game.join("mods");
        let _ = fs::create_dir_all(&mods);
        let _ = fs::write(mods.join("e2e-dummy.jar"), b"dummy-mod");
        let apply = apply_game_layer(&paths, id);
        let _ = fs::write(mods.join("e2e-dirty.jar"), b"dirty");
        let discard = discard_game_layer(&paths, id);
        let dirty_gone = !mods.join("e2e-dirty.jar").exists();
        let baseline_kept = mods.join("e2e-dummy.jar").exists();
        let state = layer_state_for(&paths, id).ok();
        let (ok_a, a_msg) = match &apply {
            Ok(r) => (r.ok, r.message_zh.clone()),
            Err(e) => (false, e.clone()),
        };
        let (ok_d, d_msg) = match &discard {
            Ok(r) => (r.ok, r.message_zh.clone()),
            Err(e) => (false, e.clone()),
        };
        let detail = format!(
            "workspace={ws:?}\napply ok={ok_a} {a_msg}\ndiscard ok={ok_d} {d_msg}\ndirty_gone={dirty_gone} baseline_kept={baseline_kept}\nstate={state:?}"
        );
        let pass = ok_a && ok_d && dirty_gone && baseline_kept;
        log_step(
            &out,
            &mut steps,
            "agent_layer",
            "apply_game_layer / discard_game_layer",
            pass,
            &detail,
            t.elapsed().as_millis(),
        );
        let _ = fs::write(out.join("11-layer.txt"), &detail);
    }

    // 9) env export dry-run (manifest only) + real metadata export
    if let Some(ref id) = instance_id {
        let t = Instant::now();
        let dry = build_env_manifest(&paths, id, EnvPackMode::Metadata, Some("e2e dry-run".into()));
        let dry_ok = dry.is_ok();
        let dry_json = dry
            .as_ref()
            .map(|m| serde_json::to_string_pretty(m).unwrap_or_default())
            .unwrap_or_else(|e| e.to_string());
        let _ = fs::write(out.join("12-env-dry-run.json"), &dry_json);
        let exp = export_env_pack(
            &paths,
            id,
            EnvExportOptions {
                mode: EnvPackMode::Metadata,
                notes: Some("e2e live".into()),
                output: Some(out.join("13-env-export.easymc-env.json")),
            },
        );
        let (exp_ok, exp_detail) = match exp {
            Ok(r) => (true, format!("{} path={}", r.message_zh, r.path)),
            Err(e) => (false, e.to_string()),
        };
        log_step(
            &out,
            &mut steps,
            "env_export",
            "env export dry-run (manifest) + metadata write",
            dry_ok && exp_ok,
            &format!("dry_ok={dry_ok}\n{dry_json}\n---\n{exp_detail}"),
            t.elapsed().as_millis(),
        );
    }

    // 10) preheat + repair
    {
        let t = Instant::now();
        let ph = preheat_container_runtime(prefer_china, false);
        let rp = repair_container_runtime_with_profile(&paths, picked_profile, instance_id.is_some());
        let detail = format!(
            "preheat ok={} {:?}\nrepair ok={} {:?}",
            ph.ok, ph.messages_zh, rp.ok, rp.messages_zh
        );
        log_step(
            &out,
            &mut steps,
            "preheat_repair",
            "preheat + repair",
            ph.ok && rp.ok,
            &detail,
            t.elapsed().as_millis(),
        );
        let _ = fs::write(out.join("14-preheat-repair.txt"), &detail);
    }

    // list
    {
        let list = list_container_versions(&paths).unwrap_or_default();
        let _ = fs::write(
            out.join("15-list.json"),
            serde_json::to_string_pretty(&list).unwrap_or_default(),
        );
    }

    // cleanup container (keep logs)
    if let Some(ref id) = instance_id {
        let _ = delete_container_version(&paths, id, true);
    }

    write_checklist(&out, &steps, t0.elapsed().as_millis());
    let failed = steps.iter().filter(|s| !s.ok).count();
    eprintln!(
        "\nE2E done: {}/{} passed, {} failed, total {} ms",
        steps.len() - failed,
        steps.len(),
        failed,
        t0.elapsed().as_millis()
    );
    if failed > 0 {
        std::process::exit(1);
    }
}

fn write_checklist(out: &Path, steps: &[Step], total_ms: u128) {
    let mut md = String::from("# Podman live E2E checklist\n\n");
    md.push_str(&format!("total_ms: {total_ms}\n\n"));
    md.push_str("| Step | Result | ms | Detail |\n| --- | --- | --- | --- |\n");
    for s in steps {
        let res = if s.ok { "PASS" } else { "FAIL" };
        let detail = s.detail.replace('|', "/").replace('\n', " ").chars().take(120).collect::<String>();
        md.push_str(&format!("| {} | {} | {} | {} |\n", s.id, res, s.ms, detail));
    }
    let _ = fs::write(out.join("CHECKLIST.md"), &md);

    let json: Vec<serde_json::Value> = steps
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "title": s.title,
                "ok": s.ok,
                "ms": s.ms,
                "detail": s.detail,
            })
        })
        .collect();
    let _ = fs::write(
        out.join("checklist.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "total_ms": total_ms,
            "passed": steps.iter().filter(|s| s.ok).count(),
            "failed": steps.iter().filter(|s| !s.ok).count(),
            "steps": json,
        }))
        .unwrap_or_default(),
    );
}
