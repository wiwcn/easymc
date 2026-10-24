# EasyMC UI 完善方案：MD3 动效体系 + 微交互（参考 ReactBits · Material 3）

**日期：** 2026-10-24
**触发：** William「我们来完善 ui，参考 reactbits.dev/c/micro 和谷歌官方 MD3 设计规范和案例，先出方案」
**范围：** 在既有 MD3 OKLCH tokens + Operate 基础上，**补齐动效 / 微交互 / 组件状态完备度**；对齐 Material 3（含 Expressive 动效物理）规范，并**有取舍地**吸收 ReactBits Micro Interactions 的原理。
**性质：** 方案与验收标准。**2026-10-24 已批准并开工**——A/B/C/D 四批实施状态见 §5。
**关联文档：** `DESIGN.md`（产品 SoT / §6.4 Motion thesis）· [`UI-IMPECCABLE-FULL-REMEDIATION.md`](./UI-IMPECCABLE-FULL-REMEDIATION.md)（全量 master，ID：E1–E4 / CMP-1 / CF-1 / TYP-1 / LD-1 / MOT-1 / TOK-1 / SP-1）· [`UI-EMPTY-LAYOUT-REMEDIATION.md`](./UI-EMPTY-LAYOUT-REMEDIATION.md)（空态深潜）

---

## 1. 结论（先看这里）

| 判定 | 说明 |
|------|------|
| **可以做** | 引入 **`motion`（motion.dev 的 **vanilla** 包，非 React 包）** 作为阻尼弹簧与编排引擎，配合 token 化的 easing/duration，把 MD3 的「motion physics / 状态层 / 转场模式」落到现有 Lit 组件里。⚠️ 此决策**与 `DESIGN.md §6.4`「仅 CSS——不加 motion 库」冲突，需同步修订该条**（见 §8-K）。 |
| **不能照搬** | ReactBits Micro 是 **React + HugeIcons + WebGL/Canvas** 组件；EasyMC 是 **Lit**。**逐组件移植（React / WebGL 演示）会破坏技术栈与 Operate 定位**——只借原理，不搬组件。 |
| **本次策略** | **吸收原理，不移植组件**：借它「让微交互有物理感、有因果」的思路；**弹簧用 `motion` 库实现**，静态 easing/duration 走 token。MD3 提供权威的 token 与结构。 |
| **主要缺口** | ① 动效只有「时长 + 一条 easing」，**没有 MD3 的转场模式与弹簧**；② 交互反馈靠改背景色/缩放，**没有统一状态层**；③ 组件状态词汇不齐（Tab 两套、模式切换滥用 primary、幽灵卡）；④ reduced-motion 白名单口径需定稿。 |
| **优先级** | **P0** 动效 token 体系 + `motion` 接入 + 状态层统一 + 转场模式对齐；**P1** 组件状态完备（segmented / tabs / switch / skeleton）+ 微交互落地 + 克制版 ripple；**P2** 数字滚动、卡片 quieter 等润色。 |
| **决策已定** | 2026-10-24 拍板：弹簧 → **引入 `motion` 库**；ripple → **上（克制版）**；页转场 → **按 MD3 去位移**；骨架屏 → **改 MD3 脉冲**。详见 §8。 |

---

## 2. 现状与冲突

### 2.1 现状资产（已对齐，别推翻）

- `src/styles/tokens.css`：OKLCH 青绿 roles（primary/surface/semantic）+ 可选 dark + shape + elevation + `--easymc-dur-*` / `--easymc-ease-out`。
- `src/styles/global.css`：选区 / caret / 滚动条 / `:focus-visible` 环 / toast in-out / reduced-motion 白名单。
- `src/styles/controls.ts`：共享控件词表（`primary/tonal/ghost/text/danger/launch`）+ `.surf-elev` / `.surf-outline`（CF-1）+ skeleton。
- `src/components/empty-state.ts`：page/panel 两变体、二维居中 + 光学微调。
- `src/app-shell.ts`：`page-in`（opacity + 6px translate）；`src/side-nav.ts`：SVG 图标（非 emoji）、active 填充图标。

### 2.2 冲突裁决（im / MD3 优先）

| # | 主题 | 既有偏好 | ReactBits / MD3 | 裁决 |
|---|------|----------|-----------------|------|
| C-1 | 动效实现 | **仅 CSS，不加库**（DESIGN §6.4） | **MD3** 是平台级弹簧物理；**ReactBits** 依赖 `motion` + WebGL | **决策变更（2026-10-24）**：引入 **`motion`（vanilla 包）** 做弹簧/编排——**须同步修订 `DESIGN.md §6.4`**（见 §8-K）；**ReactBits 组件本体仍不移植**。 |
| C-2 | 动效强度 | Operate 克制、禁编排 | ReactBits 演示偏 spectacle（Flying Button 飞 170px、WebGL 烟雾） | **以 MD3 为准**：状态反馈与转场优先；**Flying Button 类不做**（见 §3.1）。 |
| C-3 | 页面转场 | `page-in` 有 6px 位移 | MD3 **top level** 明确「淡出→淡入，不用位移/缩放」（位移属于层级导航） | **按 MD3 收窄**：顶层导航用 fade-through；层级（列表→详情）才用位移。 |
| C-4 | 交互相应 | 改背景色 + `scale(0.98)` | MD3 **state layer** 8/12/12/16% | **引入状态层**，与既有 hover 底色并存（见 §4.2）。 |
| C-5 | 骨架屏 | shimmer 扫光 | MD3 skeleton 为**脉冲**，内容快速淡入覆盖 | **改脉冲为主**，内容 crossfade 覆盖（P2，见 §4.5）。 |

---

## 3. 参考基线提炼

### 3.1 ReactBits `/c/micro` 逐项评估

页面列出的五个 Micro（另见相关：Dodge Field / Flip Card / Glide Select / Prompt Bar / Spring Check / Slide Commit / Sling Button / Voice Pill …）：

| ReactBits 组件 | 机制 | EasyMC 适配 | 借用点（原理） | 落点 |
|----------------|------|-------------|----------------|------|
| **Flying Button**（WebGL 烟雾 + 推进器 + 升空 170px + 跟随光标） | WebGL/Canvas 物理飞行 | ✗ **否决** | 无（属 spectacle，违 craft-floor「禁编排/禁营销页动效」） | 不采纳（如要彩蛋须显式立项，且默认关） |
| **Bell Toggle** | 弹簧摆动 + 标签 crossfade + 展开；**预留宽度**使相邻元素不跳；键盘只给 crossfade | ✓ 强适配 | **弹簧切换 + 宽度预留 + reduced-motion→仅 crossfade** | 设置项开关（动效减弱 / 主题）、任何 toggle 型状态 |
| **Branched Menu** | 折叠树 + 收起/展开 + 指示线描画 + 宽度贴合内容 | ✓ 中适配 | **折叠区 + 指示器 morph** | 版本管理详情分区、下载分类；用 **MD3 nav rail 指示器**，不画装饰墨线 |
| **Call Chip** | chip 就地展开为面板（morph） | ✓ 强适配 | **chip ↔ 面板容器变换（MD3 container transform·屏内）** | Agent 快捷意图芯片展开为输入；下载来源/加载器芯片 |
| **Code Slots** | 老虎机式滚动数字/字形 | ○ 克用 | **就地数字滚轮（odometer）** | 下载进度 %、版本号变更、内存滑块数值；Operate 下限量使用 |

> 结论：**借 4 个原理，否决 1 个组件**。组件本体一律**用 Lit + CSS/WAAPI 复刻**，不引入 React / HugeIcons / WebGL；弹簧物理由 **`motion` vanilla 入口**驱动（见 §4.8），不用 `motion/react`。

### 3.2 Material 3 权威规格（本次对齐依据）

**Easing token（官方 CSS 值，legacy transitions 仍在用）**

| Token | CSS |
|-------|-----|
| `easing.standard` | `cubic-bezier(0.2, 0, 0, 1)` |
| `easing.standard.decelerate` | `cubic-bezier(0, 0, 0, 1)` |
| `easing.standard.accelerate` | `cubic-bezier(0.3, 0, 1, 1)` |
| `easing.emphasized` | CSS 无对应（官方建议回退 standard） |
| `easing.emphasized.decelerate` | `cubic-bezier(0.05, 0.7, 0.1, 1)`（进入屏） |
| `easing.emphasized.accelerate` | `cubic-bezier(0.3, 0, 0.8, 0.15)`（离开屏） |

**Duration token（ms）**

- short1–4：`50 / 100 / 150 / 200`（选择控件等小工具）
- medium1–4：`250 / 300 / 350 / 400`（中等面积转场）
- long1–4：`450 / 500 / 550 / 600`（大而表达性的转场）
- extra-long1–4：`700 / 800 / 900 / 1000`（罕见、环境类、无输入驱动）

**Transition patterns（六种）**：Container transform（卡↔详情、芯片↔面板）· Forward & backward（层级前进/后退，带位移+淡入）· Lateral（同级 Tab 滑动）· **Top level（顶层导航：淡出→淡入，不位移）** · Enter & exit（菜单/对话框/snackbar，**从屏幕边缘方向展开**）· Skeleton loaders（脉冲 + 内容淡入覆盖）。

**State layer（状态层）**：hover `8%` · focus `12%` · pressed `12%` · dragged `16%`（on-color 叠加在容器色上）。

**Motion physics（Expressive 更新）**：组件与动效改用**弹簧**（spatial spring 管位置/尺寸/形状，可轻微过冲；effects spring 管颜色/透明度，不过冲）。官方注明 legacy easing/duration 仍可用于 transitions，但**新组件应迁移**。

---

## 4. 落地设计（核心）

### 4.1 Motion token 升级（不破坏既有）

在 `tokens.css` 新增 **MD3 命名层**，并把现有 `--easymc-*` 作为别名映射过去（**不删旧名**，保证零回归）：

```css
/* MD3 easing */
--md-sys-motion-easing-standard: cubic-bezier(0.2, 0, 0, 1);
--md-sys-motion-easing-standard-decelerate: cubic-bezier(0, 0, 0, 1);
--md-sys-motion-easing-standard-accelerate: cubic-bezier(0.3, 0, 1, 1);
--md-sys-motion-easing-emphasized-decelerate: cubic-bezier(0.05, 0.7, 0.1, 1);
--md-sys-motion-easing-emphasized-accelerate: cubic-bezier(0.3, 0, 0.8, 0.15);

/* MD3 duration */
--md-sys-motion-duration-short2: 100ms;
--md-sys-motion-duration-short4: 200ms;
--md-sys-motion-duration-medium2: 300ms;
--md-sys-motion-duration-medium4: 400ms;
--md-sys-motion-duration-long2: 500ms;

/* 弹簧：由 `motion` 库（vanilla）在运行时生成，不落为静态 CSS 变量 */
/* 仅静态 easing / duration 落 token，供 CSS 侧 transition 使用 */

/* 既有别名 → 指向 MD3（保持向后兼容） */
--easymc-ease-out: var(--md-sys-motion-easing-emphasized-decelerate);
--easymc-dur-press: var(--md-sys-motion-duration-short2);
```

### 4.2 状态层（State layer）统一

对 `button.*` / nav item / 卡片可点区 / 列表行，统一叠一层 `::after` 状态层（`background: currentColor` + 透明度分层），替代目前「各页自己换底色」：

- hover `8%` · focus-visible `12%` · pressed（`:active`）`12%` · 拖拽 `16%`。
- 与既有 `:focus-visible` 焦点环**并存**（环在外、状态层在内）。
- 与 `CF-1`（elevation XOR border）不冲突：状态层是覆盖色，不改 elevation。

### 4.3 转场模式对齐（app-shell + 页面）

| 场景 | 现行 | MD3 目标 | 动作 |
|------|------|----------|------|
| 顶层导航切换（启动/下载/助手…） | `page-in`＝opacity + **6px 位移** | **top level**：淡出→淡入，**不用位移/缩放** | 改 fade-through（`C-3`）；去掉 translate |
| 列表 → 详情（版本详情 Tab、下载详情） | 无 | **forward & backward**：横向位移 + 淡入 | 新增层级转场（P1） |
| 同级 Tab（下载六 Tab、设置分组） | 仅内容替换 | **lateral**：滑块/内容同向滑动 | Tab 指示器 morph（P1） |
| 卡 ↔ 详情 / 芯片 ↔ 面板 | 直接出现 | **container transform** | 下载卡→详情、Agent 芯片→输入（P1，见 §4.4） |
| 菜单 / 弹层 / toast | toast 有 in-out；菜单无 | **enter & exit**：从触发边缘方向展开 | 菜单/下拉按来源方向进出（P1） |
| 加载 | home 有 skeleton，其余「加载中…」 | **skeleton**：脉冲 + 内容淡入覆盖 | 全列表骨架（P0/P1，接 LD-1） |

### 4.4 微交互落地清单（ReactBits 原理 → EasyMC 组件）

| ID | 落点 | 借鉴 | 规格（Operate 口径） |
|----|------|------|----------------------|
| **MI-1** | 侧栏 active 指示器 | Branched Menu 指示器 | active 背景 pill 在项间 **morph**（宽度/位置过渡 `medium2`，`emphasized.decelerate`）；不做装饰墨线 |
| **MI-2** | 设置开关（主题 / 动效减弱 / 联机开关） | Bell Toggle | 弹簧切换 `spring-effects`，thumb 位移；**键盘/减弱动效只做 crossfade**；标签变化**预留宽度**防抖动 |
| **MI-3** | 下载 / Agent 意图芯片 | Call Chip | 芯片 → 面板 **container transform**（同层就地展开）；收起沿原路径返回 |
| **MI-4** | 下载进度 / 版本号 / 内存数值 | Code Slots | 就地数字滚动（odometer），总时长 ≤ `medium2`；**reduced-motion 直接跳变** |
| **MI-5** | 版本文本卡 hover/选中 | — | hover 抬升 `elevation-1→2` `dur-fast`；选中＝容器色 + 边框（已 `quieter`），**不加弹簧过冲** |
| **MI-6** | 启动 CTA（`button.launch`） | — | press 保持 `scale(0.985)` + 状态层 12%；成功启动回馈用 toast 已足够，**不加闪光/飞行动画** |
| **MI-7** | 空状态出场 | — | EmptyState 首次挂载 opacity + 8px 上浮 `dur-med`（一次），**不重复编排** |
| **MI-8** | 弹层 / 下拉菜单 | — | 从触发按钮边缘方向展开（Enter & exit），`short4` 进入 / `short3` 退出 |

> 硬红线（沿用 craft-floor）：禁 WebGL/Canvas spectacle、禁 bounce/elastic 作默认曲线、禁编排式整页入场、禁同时 shadow + 粗边。

### 4.5 组件状态完备（MD3 component 对齐）

| ID | 组件 | 现状 | 目标（MD3） |
|----|------|------|-------------|
| **MC-1** | Segmented button（模式切换） | 双 `primary/ghost` 冒充（CMP-4） | 独立词表：选中 `secondary-container`，含位移指示器 |
| **MC-2** | Tabs（下载六 Tab / 设置四组） | 两套近似 pill（CMP-2） | 单一 Tab 词表 + lateral 指示器 |
| **MC-3** | Switch / Checkbox / Radio | 原生控件为主 | MD3 形状 + 状态层 + 弹簧；触达 ≥ `24px` 命中区 |
| **MC-4** | Chips | 字号 0.7–0.78 漂移（CMP-3） | `assist / filter / input / suggestion` 四类 + 统一 min-height 24/32 |
| **MC-5** | Progress（线性/圆形） | 仅线性 bar | MD3 线性 + 圆形；不定态用 MD3 波形 |
| **MC-6** | Menu / Dialog / Snackbar | menu 无进出、dialog 无、toast 有 | 统一 Enter & exit；dialog 焦点陷阱 |
| **MC-7** | Card | `surf-elev` / `surf-outline` 已有 | 明确 `elevated / filled / outlined` 三态映射（CF-1 XOR 保持） |
| **MC-8** | Nav rail（侧栏） | 已有 SVG + active | active 指示器 morph（MI-1）；禁用项状态 |

### 4.6 可访问性（harden）

- **reduced-motion**：保留颜色/透明度/尺寸状态过渡，**只去位移与缩放**（沿用并定稿 §8.6 口径）；弹簧曲线在 reduced 下退化为 `standard` 直落。
- **焦点**：所有可交互元素 `:focus-visible` 环；菜单/对话框焦点管理。
- **对比**：状态层叠加后复核正文 ≥4.5:1、控件 ≥3:1（尤其 dark 主题）。
- **触达**：icon-btn ≥40px 保持；芯片 ≥24px 且间距达标（A11Y-4）。

### 4.7 Ripple（克制版）— 已批准

- **落点**：`button.launch`、`button.primary`（filled）与侧栏 nav item；其余（`ghost` / `text` / 卡片 / 列表行）**不加**。
- **规格**：在状态层之上叠一层由指针落点扩散的 `currentColor` 涟漪——起始不透明度 ≈ **12%**，`short4`(200ms) 扩散覆盖、`short3`(150ms) 淡出；半径按元素对角线计算。
- **实现**：由 `motion` 驱动（见 §4.8），或极轻量 WAAPI（一次性涟漪无需常驻）；**reduced-motion 直接不发**。
- **约束**：同一元素同时仅一层（快速连点就地重置）；绝对定位不改布局；不与 focus ring 抢视觉。

### 4.8 `motion` 库接入与降级

- **包**：`motion`（motion.dev，**框架无关的 vanilla 入口**：`animate` / `spring` / `inView` / `stagger`）；**不改 Lit**，不使用 `motion/react`。
- **参数映射（起点值，落地按观感标定）**：
  - MD3 **spatial spring**（位置/尺寸/形状）≈ `{ type: "spring", stiffness: 380, damping: 30 }`（≈ `dampingRatio 0.8`，允许轻微过冲）；
  - MD3 **effects spring**（颜色/透明度）≈ 高刚度无过冲 `{ stiffness: 1600, damping: 80 }`。
  - 标定后的常量集中一处（如 `src/styles/motion.ts`），组件只引用，不散落魔数。
- **降级**：`prefers-reduced-motion: reduce` → 弹簧退化为 `standard` easing 直落或跳变；库加载失败 → 回退现有 CSS transition，**功能不依赖库**。
- **打包/体积（实测 2026-10-24 生产构建）**：JS `330.45 KB`（gzip `87.80 KB`）、CSS `12.05 KB`（gzip `2.86 KB`）。引入前基线 ≈ `254.9 KB / 59.7 KB` —— **实际远大于原「< 20KB」预期**。改用 `motion/mini`（`dom-mini.mjs` 仅转发 WAAPI 动画器、无弹簧能力）可大幅省体积，但**弹簧正是决策 A 的核心**；EasyMC 为 Tauri 本地桌面应用、资源随包本地分发、无网络成本，故**保留全量 vanilla 入口**，接受该体积。

---

## 5. 分批实施

| 批次 | 内容 | ID | 优先级 |
|------|------|----|--------|
| **A. 系统层** | `motion` 接入 + 弹簧常量表（§4.8）；MD3 motion token 层 + 别名映射（§4.1）；状态层统一（§4.2）；top-level 转场去位移（C-3） | A1 / §4.1 / §4.2 / C-3 | **P0** |
| **B. 状态完备** | Segmented（MC-1）、Tabs（MC-2）、Switch/Checkbox（MC-3）、Chips（MC-4）、全列表 skeleton（LD-1）；ripple 克制版落 filled 按钮/nav（§4.7） | MC-1..4 / LD-1 / §4.7 | **P0–P1** |
| **C. 微交互** | MI-1 指示器 morph、MI-3 容器变换、MI-4 数字滚动、MI-8 弹层进出、MI-7 空态出场 | MI-1/3/4/7/8 | **P1** |
| **D. 润色** | Skeleton 改 MD3 脉冲 + 内容淡入（C-5）、Menu 完整进出（MC-6）、卡片 quieter | C-5 / MC-6 / MI-5 | **P2** |

**实施状态（2026-10-24 「同意修订，开工」；`tsc --noEmit` + `vite build` 均通过）**

- **A 批次**：完成——motion token 层 + `--easymc-ease/dur-*` 别名、按钮/nav 状态层 8/12/12%、`page-in` 去位移、`src/styles/motion.ts` 弹簧常量表。
- **B 批次**：完成——`.md-tabs`/`.md-segmented`/`.md-chip`/checkbox·radio 词表 + 滑动指示器（`src/lib/indicator.ts`）+ 克制 ripple（`src/styles/ripple.ts`）；LD-1 骨架统一为 `.skel/.skel-line/.skel-block`（download / instance / home / accounts）。
- **C 批次**：完成——**MI-1** 侧栏指示器 morph（`side-nav` 复用 `syncIndicatorWithin`，`.nav-ind`）；**MI-4** 数字滚动（`rollNumber`，落点：启动页安装进度 %）；**MI-7** 空态出场（`enterUp`，作用于 `.stack`，不覆盖 `translateY(-4%)`）；**MI-8** 菜单进出（`.menu-panel.open`，进入 `short4` / 退出 `short3`，起始自触发边缘）。**MI-3 收敛**：下载卡「版本」就地展开仅做轻量 `expand` 入场（opacity + 6px），未做完整共享元素 morph（需 View Transitions，暂缓）。
- **D 批次**：完成——**C-5** 骨架改 MD3 脉冲（`easymc-skel-pulse`）+ 加载后内容淡入（host 加 `.md-loaded`）；**MC-6** 与 MI-8 同一实现；**MI-5** 卡片 quieter（home `.vcard.selected` 既有；agent `button.tool` hover 补 `dur-fast` 过渡）。
- **未落地**：`easymc-shots/motion/` 证据截录需在真机应用中人工录屏，本次未生成。

与既有 master 的关系：本方案**不重开 IA**（§8.4 已定），只做**动效 + 状态 + 组件工艺**；E1–E4 空态、CF-1 幽灵卡、TYP-1 字阶、SP-1 间距、LD-1 骨架、CMP-1 按钮词表**仍按 master 现有 backlog 执行**，本文在其上补动效维度。

---

## 6. 不做什么（红线）

- **不引入** React / `motion/react`(Framer) / HugeIcons / WebGL 依赖；不动技术栈（Lit + CSS/WAAPI）。**唯一新增依赖是 `motion` 的 vanilla 入口**（§4.8），仅取弹簧/编排，且功能不依赖它（可降级）。
- **不做** Flying Button 式 spectacle；不做整页编排入场；不把 bounce/elastic 当默认曲线。
- **不重开** 侧栏 IA、下载六 Tab、账户 PCL 式列表、启动主 CTA 结构。
- **不删** 既有 `--easymc-*` token 名（只加 MD3 层并做别名），避免回归。
- **不触碰** 本文档以外的空态 / 文案整改范围（引 E1–E4、T2 即可）。

---

## 7. 验收 / 证据

- **动效**：录屏逐条对照 §4.4 清单（hover/press 状态层 8/12%、指示器 morph、容器变换、数字滚动、弹层方向）。
- **转场**：顶层导航 fade-through（无位移）；层级导航有位移；Tab lateral。
- **组件**：MC-1..7 各一帧（默认 / hover / 选中 / 禁用 / 聚焦）。
- **A11y**：`prefers-reduced-motion: reduce` 下复查——保留状态反馈、无位移；焦点环；dark 主题对比。
- **回归**：有数据页无变化；空态仍二维居中（E1–E4 不回退）。
- **证据留存**：`easymc-shots/motion/`（新增）。

---

## 8. 决策点（2026-10-24 已拍板）

| # | 议题 | 选项 | 拍板结果 |
|---|------|------|----------|
| **A** | 弹簧实现路径 | (1) CSS `linear()` 弹簧曲线；(2) 自写 TS spring 驱动 WAAPI；(3) 引入 `motion`(motion.dev) 库 | ✅ **选 (3) 引入 `motion`（vanilla 入口）**。⚠️ 与 `DESIGN.md §6.4`「仅 CSS——不加 motion 库」冲突，**须同步修订该条**（§8-K）；仍保留降级回退 CSS。 |
| **B** | 是否上 ripple（MD3 签名） | 上（克制，仅 filled 按钮/nav） / 不上 | ✅ **上（克制版）**：仅 `button.launch` / `button.primary` / 侧栏 nav item，规格见 §4.7；reduced-motion 不发。 |
| **C** | 顶层导航转场 | 去位移按 MD3 top-level（fade-through） / 保留 6px 位移 | ✅ **按 MD3 去位移**（C-3）：顶层 fade-through，层级页才用位移。 |
| **D** | Skeleton 风格 | 改 MD3 脉冲 / 保留 shimmer | ✅ **改 MD3 脉冲**（C-5）+ 内容淡入覆盖。 |

### 8-K 依赖的 SoT 修订（待批准后执行）

因决策 A 变更，`DESIGN.md` 需同步修订一处：

- **§6.4 Motion thesis**：现为「**仅 CSS——不加 motion 库**」→ 改为「以 CSS/WAAPI 为主；**允许 `motion` 的 vanilla 入口**做弹簧与编排（不用 `motion/react`），且必须可降级（reduced-motion / 加载失败回退 CSS transition）」。
- 其余条款（Feedback / Continuity / Lists / Focal / A11y 时长口径）不变。

---

## 9. 审阅清单（给 William）

- [x] §8 四个决策点已于 **2026-10-24 拍板**（motion 库 / 克制 ripple / 顶层去位移 / MD3 脉冲）
- [x] 同意 §1 策略「吸收原理、**不移植 ReactBits 组件本体**、仅引入 `motion` vanilla 入口」
- [x] 同意 §2.2 冲突裁决（C-1…C-5）
- [x] 同意 §4 落地设计（token / 状态层 / 转场 / 微交互 / 组件 / ripple / motion）
- [x] 同意 §8-K 对 `DESIGN.md §6.4` 的修订（引入 `motion` 的前置条件，已修订）
- [x] 2026-10-24「同意修订，开工」→ 已按 §5 顺序实施 A/B/C/D，见上方「实施状态」

---

*依据：ReactBits `/c/micro`（2026-10-24 拉取：Flying Button / Bell Toggle / Branched Menu / Call Chip / Code Slots 及其 props）、Material 3 官方 `motion/easing-and-duration` 与 `motion/transitions`（2026-10-24 拉取）、EasyMC 源码树（`tokens.css` / `global.css` / `controls.ts` / `app-shell.ts` / `side-nav.ts` / `empty-state.ts` / `home-page.ts`）、仓库 `DESIGN.md` 与两份既有整改文档。*
