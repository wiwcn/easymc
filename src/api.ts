import type { InstanceDto } from "./types";
import { versionId } from "./types";

const FALLBACK: InstanceDto[] = [
  {
    id: "survival",
    name: "Survival World",
    version: "1.21.1",
    loader: "fabric",
    description: "Your go-to survival experience.",
    last_played: "Today, 4:32 PM",
  },
  {
    id: "creative",
    name: "Creative Build",
    version: "1.21.1",
    loader: "vanilla",
    last_played: null,
  },
  {
    id: "modded",
    name: "Modded Adventures",
    version: "1.21.1",
    loader: "fabric",
    last_played: null,
  },
  {
    id: "test",
    name: "Test World",
    version: "1.21.1",
    loader: "vanilla",
    last_played: null,
  },
];

async function invokeSafe<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<T>(cmd, args);
  } catch {
    return null;
  }
}

export async function listInstances(): Promise<InstanceDto[]> {
  const raw = await invokeSafe<InstanceDto[]>("list_instances");
  if (!raw) return FALLBACK;
  return raw.map((i) => ({
    ...i,
    version: versionId(i.version),
    loader: String(i.loader).toLowerCase(),
  }));
}

export async function launchInstance(instanceId: string): Promise<string> {
  const res = await invokeSafe<{ ok: boolean; message: string }>("launch_instance", {
    instanceId,
  });
  if (res) return res.message;
  return `M0 stub: 将启动「${instanceId}」（浏览器预览模式，无 Tauri 后端）`;
}
