/**
 * Native Tauri file pickers (plugin-dialog). Falls back to window.prompt only
 * outside the desktop shell (e.g. vite-only browser preview).
 */
import { open } from "@tauri-apps/plugin-dialog";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Pick a `.easymc-env` / `.easymc-env.zip` / `.zip` environment pack. Returns null if cancelled. */
export async function pickEnvPackFile(): Promise<string | null> {
  if (isTauri()) {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "选择 EasyMC 环境包",
      filters: [
        {
          name: "EasyMC 环境包",
          extensions: ["easymc-env", "zip"],
        },
      ],
    });
    if (selected == null) return null;
    if (Array.isArray(selected)) return selected[0] ?? null;
    return selected;
  }
  const typed = window.prompt(
    "环境包完整路径（.easymc-env 或 .easymc-env.zip）",
    "",
  );
  const t = typed?.trim();
  return t ? t : null;
}
