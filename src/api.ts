import type { AccountDto, InstanceDto, InstancesState, VersionSummary } from "./types";
import { versionId } from "./types";
import type { InstallProgressDto } from "./install-progress";
export type { InstallProgressDto } from "./install-progress";

async function invokeSafe<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<T>(cmd, args);
  } catch (e) {
    console.warn("invoke failed", cmd, e);
    return null;
  }
}

async function invokeOrThrow<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

function mapInstance(i: InstanceDto): InstanceDto {
  return {
    ...i,
    version: versionId(i.version),
    loader: String(i.loader).toLowerCase(),
  };
}

function mapState(raw: InstancesState | InstanceDto[] | null): InstancesState {
  if (!raw) {
    return {
      instances: [
        {
          id: "default",
          name: "Survival World",
          version: "1.21.1",
          loader: "vanilla",
          description: "浏览器预览 · 请用 pnpm tauri dev 连接后端",
          version_ready: false,
        },
      ],
      active_id: "default",
    };
  }
  if (Array.isArray(raw)) {
    return {
      instances: raw.map(mapInstance),
      active_id: raw[0]?.id ?? null,
    };
  }
  return {
    instances: (raw.instances ?? []).map(mapInstance),
    active_id: raw.active_id ?? null,
  };
}

export async function listInstancesState(): Promise<InstancesState> {
  const raw = await invokeSafe<InstancesState | InstanceDto[]>("list_instances");
  return mapState(raw);
}

export async function listInstances(): Promise<InstanceDto[]> {
  return (await listInstancesState()).instances;
}

export async function createInstance(name: string, version: string): Promise<InstanceDto> {
  const raw = await invokeOrThrow<InstanceDto>("create_instance", { name, version });
  return mapInstance(raw);
}

export async function setActiveInstance(instanceId: string): Promise<InstancesState> {
  const raw = await invokeOrThrow<InstancesState>("set_active_instance_cmd", { instanceId });
  return mapState(raw);
}

export async function listAccounts(): Promise<AccountDto[]> {
  return (await invokeSafe<AccountDto[]>("list_accounts")) ?? [];
}

export async function createOffline(username: string): Promise<AccountDto | null> {
  return invokeSafe<AccountDto>("create_offline", { username });
}

export async function listVersions(includeSnapshots = false): Promise<VersionSummary[]> {
  return (
    (await invokeSafe<VersionSummary[]>("list_versions", { includeSnapshots })) ?? []
  );
}

export async function installVersion(version: string, withAssets = true): Promise<string> {
  return invokeOrThrow<string>("install_mc_version", { version, withAssets });
}

export async function launchInstance(instanceId: string): Promise<string> {
  try {
    const res = await invokeOrThrow<{ ok: boolean; message: string }>("launch_instance", {
      instanceId,
    });
    return res.message;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(msg);
  }
}

export async function stopInstance(): Promise<void> {
  await invokeSafe("stop_instance");
}

export async function getLogs(): Promise<string[]> {
  return (await invokeSafe<string[]>("get_logs")) ?? [];
}

export async function ensureJava(major = 21): Promise<string> {
  const j = await invokeOrThrow<{ path: string; major: number }>("ensure_java_cmd", { major });
  return `${j.path} (Java ${j.major})`;
}

export async function listenLogs(handler: (line: string) => void): Promise<() => void> {
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const un = await listen<string>("launch-log", (e) => handler(e.payload));
    return () => un();
  } catch {
    return () => undefined;
  }
}

export async function listenInstallProgress(
  handler: (p: InstallProgressDto) => void,
): Promise<() => void> {
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const un = await listen<InstallProgressDto>("install-progress", (e) => handler(e.payload));
    return () => un();
  } catch {
    return () => undefined;
  }
}

