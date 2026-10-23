# EasyMC design notes

**Mode:** Operate (launcher app) — scanability over spectacle.  
**Identity:** soft teal MD3 seed (青绿). Original MD3 UI — not a PCL skin; PCL is IA reference only.

## Information architecture

| Area | Role |
|------|------|
| **启动** | Launch / install the active instance |
| **商店** | Discover & install mods (Modrinth / CurseForge) into the **selected instance** |
| **库** | Instances + **per-instance mod management** (list / enable / disable / uninstall) |
| **设置** | Accounts, CurseForge key, runtime |

Installed jars belong to the instance `mods/` folder (and thus that game version), not to the store page. Empty mod list → “去商店下载” switches nav to Store.

## Tokens

- Source: `src/styles/tokens.css` + `global.css`
- Accent (`--md-sys-color-primary`) for primary actions, selection, focus only
- Cool teal-tinted neutrals for surfaces; semantic containers for success / warning / error
- Focus: `--easymc-focus-ring`; selection and scrollbars themed in `global.css`

## Motion thesis (Operate)

- **Feedback:** button press ~100–150ms (`--easymc-dur-press`), toast in/out
- **Continuity:** page change opacity + slight translate (`--easymc-dur-page`, keyed remount in `app-shell`)
- **Lists:** short row enter; uninstall leave; stagger capped (~140ms)
- **Focal (quiet):** Home launch hero entrance only
- **A11y:** `prefers-reduced-motion: reduce` disables spatial motion; opacity/state changes may remain

Easing: `--easymc-ease-out` = `cubic-bezier(0.16, 1, 0.3, 1)`. CSS only — no motion library.

## Craft floor (keep)

No eyebrow kickers, nested card soup, gradient text, decorative left borders, or expanding emoji-as-icon systems. Prefer weight/size for emphasis; one elevation system (shadow or border, not both as ghost cards).
