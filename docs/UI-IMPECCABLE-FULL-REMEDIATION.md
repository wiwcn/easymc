# EasyMC 全量 UI 整改文档（对照 Impeccable · Operate）

**日期：** 2026-10-24（Asia/Shanghai）  
**触发：** William「全量 ui 对比 im 写整改文件」  
**范围：** 对照 [Impeccable](https://impeccable.style/) reference（`audit` / `critique` 结构 / `layout` / `typeset` / `colorize` / `animate` / `polish` / `clarify` / `distill` / `quieter` / `operate` / `mode-operate` / `onboard` / `harden` / `craft-floor` / `shape` / `adapt`）与现版源码、截图、mocks；**本文只出方案与验收，不直接改 UI 代码**。  
**权威顺序：** **Impeccable > 既有产品偏好**。若 im 与 `DESIGN.md` / 个人偏好冲突，以 im 为准，并在 **§8 冲突点** 写明；落地前须经 William 审阅批准，批准前不做大改 UI。  
**关联：** 仓库 `DESIGN.md`（Operate、玩家向）；窄域空态深潜 → [`UI-EMPTY-LAYOUT-REMEDIATION.md`](./UI-EMPTY-LAYOUT-REMEDIATION.md)（E1–E4 / C1–C3 / T1–T3）。

---

## 1. 执行摘要

### 1.1 总评（Operate + craft floor）

| 维度 | 粗分（0–4） | 一句话 |
|------|-------------|--------|
| Operate 任务清晰度 | **3** | 启动 CTA、侧栏 IA、下载六 Tab、账户 PCL 式列表方向正确；工具大体「消失在任务里」。 |
| Craft floor | **2** | Token / 焦点环 / reduced-motion / SVG 侧栏图标已上；仍有幽灵卡片、按钮样式漂移、字阶碎片、空态未统一。 |
| Typeset 扫读 | **2** | 单族 Roboto+系统回退正确；缺**固定角色字阶**，各页任意 `rem` 过多。 |
| Colorize / quieter | **3** | OKLCH 青绿 roles 克制；Primary 基本只用于动作/选中；局部硬编码 oklch 装饰仍在。 |
| Layout / 空态 Extremes | **1–2** | 有数据态节奏尚可；**空态二维居中**仍是最大结构失败（见空态专文）。 |
| Motion | **3** | 时长/easing token 对齐 Operate；页入 + Home focal 克制；全局 `0.01ms` reduce 需按 audit 意图化。 |
| Clarify 文案 | **3** | 主路径玩家向已大幅清理；筛选空与首用仍共用「什么都没有」；`placeholder-page` 残留环境变量腔。 |
| Onboard | **3** | 三步门禁清晰；与 im「尽量可跳过」冲突见 §8。 |
| Harden / a11y 可见项 | **2** | 部分 `role`/`aria` 已有；顶栏菜单 `tabindex="-1"`、加载无 skeleton、长文/缩放未系统验。 |

**综合：** 约 **Acceptable → Good 下限**（参考 audit 10–13 / 14–17 分界）。产品已像「启动器类别」工作屏，而非营销页；距「类别最佳 + craft floor 全绿」差在**空态结构、组件词汇统一、字阶系统、加载/错误态完备**。

### 1.2 Top 5 · P0（先看这里）

| # | ID | 一句话 |
|---|-----|--------|
| 1 | **E1–E4** | 全页/卡片空态须在所属容器内**水平+垂直居中**（`easymc-empty-state` 已存在但未铺开；设置/下载/实例仍 `.empty-soft` 贴顶）。 |
| 2 | **CMP-1** | 各页各自定义 `button.primary` / `ghost` —— 违反 operate「同一控件词表」；抽出共享按钮（及 tabs / chips）组件或共享 stylesheet。 |
| 3 | **TYP-1** | 无固定角色字阶（display / title / body / label / meta）；`font-size` 碎片约 0.7–1.55rem 散落 —— 对齐 typeset + operate「fixed rem scale · 1.125–1.2」。 |
| 4 | **CF-1** | Featured / 市场卡等 **elevation + 1px border 同用**（幽灵卡片）—— craft-floor 要求 elevation **二选一**（shadow **或** border）。 |
| 5 | **LD-1** | 加载态多为「加载中…」纯文本；operate 要求 **skeleton** 而非内容区中央 spinner/一句字；列表/启动卡首屏尤甚。 |

---

## 2. 方法与证据

### 2.1 Impeccable 透镜（已读）

| 透镜 | 用于本文哪些判断 |
|------|------------------|
| **operate.md / mode-operate.md** | 工具消失在任务；扫读；accent 仅动作/选中；组件全状态；skeleton；一致 affordance；动效 150–250ms；禁止页载编排。 |
| **craft-floor.md** | 禁 eyebrow、嵌套卡、渐变字、装饰左边框、emoji 图标、幽灵卡；浏览器表面；对比/间距/状态。 |
| **layout.md** | 阅读序 / 节奏 / 密度 / **Extremes（空态）**；间距刻度。 |
| **typeset.md** | 单族、固定 rem、角色可辨、measure。 |
| **colorize.md / quieter.md** | 角色色、对比、克制剂量、减少噪声。 |
| **animate.md** | 状态反馈 vs 装饰；timing；`prefers-reduced-motion`。 |
| **clarify.md / distill.md** | 空态类型区分、动作命名、砍冗余。 |
| **onboard.md** | 尽快到 aha；空态即引导；可跳过（与门禁冲突 → §8）。 |
| **harden.md / audit.md** | 溢出、对比、焦点、触达、完整性评分框架。 |
| **polish.md** | 整改落地顺序参考（功能态 → 缺态 → 漂移 → 视觉）。 |
| **shape.md / adapt.md** | 桌面优先；窄窗侧栏折叠已有苗头；本文不重开 IA。 |

### 2.2 证据源

| 类型 | 路径 |
|------|------|
| 源码 | `src/app-shell.ts`、`src/pages/*`、`src/components/{side-nav,empty-state,msa-login}.ts`、`src/styles/{tokens,global}.css` |
| 设计 SoT | `DESIGN.md` §1 / §6 / §7 |
| 空态专文 | `docs/UI-EMPTY-LAYOUT-REMEDIATION.md`（E1–E4） |
| 截图 | `/workspace/easymc-shots/{launch-polish,download-pcl,accounts-pcl,blank-flow,phase5,phase2,v1-align}/` |
| Mocks | `/workspace/easymc-mocks/{home-v1,store-v1,agent-v1}.jpg` |
| Impeccable raw | `pbakaus/impeccable` → `skill/reference/*.md`（2026-10-24 拉取） |

**不重开的 IA（除非 im 硬冲突）：** 启动主 CTA、下载六 Tab、设置·账户 PCL 式列表、侧栏「启动/下载/助手/联机/设置/更多」。本文只批**视觉工艺与状态完备**，不重做导航模型。

---

## 3. 分页矩阵

列含义：**现状** / **Impeccable 期望** / **差距** / **优先级** / **整改方向**。

### 3.1 启动（`home-page.ts`）

| | |
|--|--|
| **现状** | 有版本：featured 大卡 +「启动游戏」渐变 CTA + 版本网格，接近 `home-v1`；紧凑态已有。无版本：已接 `easymc-empty-state`（工作区），截图 `launch-polish/home-empty.png` 仍显偏上。加载：`加载中…`。 |
| **期望** | Operate：一眼找到 Launch；squint 测试主/次清晰。layout Extremes：空态占满主内容区并二维居中。craft-floor：卡 elevation 二选一。animate：仅一处 focal。 |
| **差距** | ① 空态高度契约未在所有构建稳定（E1）。② featured 同时 `box-shadow` + `border`（~L186–187）。③ launch 渐变与 art 装饰用裸 oklch，未进 token。④ 加载无 skeleton。⑤「管理」与溢出菜单密度尚可，但按钮样式与设置页不一致。 |
| **优先级** | **P0** E1 / CF-1 / LD-1；**P1** token 化 CTA/art；**P2** 版本卡选中态更安静（quieter）。 |
| **方向** | 验收空态多窗口高度；featured 只留 shadow **或** 细 border；抽出 `easymc-button`；首屏 skeleton 对齐 featured 几何。 |

### 3.2 下载（`download-page.ts`）

| | |
|--|--|
| **现状** | 六 Tab；原版版本卡→加载器卡（PCL IA）；市场搜索+源筛选。筛选空：`empty-soft`「咦？似乎什么都没有…」（`vanilla-empty-filter.png`）。搜索「加载中…」。卡：elevation+border。 |
| **期望** | layout：结果区空态在**结果槽**内居中，筛选栏不动。clarify：筛选无结果 ≠ 首用空。operate：一致控件；skeleton。 |
| **差距** | E3 未用 `EmptyState[panel]`；文案与首用同句（T2）；`.search` 亦幽灵卡倾向；`button.primary` 再定义一份。 |
| **优先级** | **P0** E3+T2；**P1** CMP-1 / CF-1；**P2** 市场卡与 store-v1 密度微调。 |
| **方向** | 结果区包一层可 `flex:1` 的槽 + EmptyState；文案改「没有符合筛选的版本」+「清空筛选」；搜索条 elevation 二选一。 |

### 3.3 助手（`agent-page.ts`）

| | |
|--|--|
| **现状** | 聊天主列 + 右侧 tools（220px）；徽章「本地助手已就绪」；快捷芯片；玩家向副文案已对齐 DESIGN。相对 `agent-v1`：mock 偏「服务器优化」叙事（产品已否决）；布局上 mock 工具为右侧卡片栈，现版为 tool 按钮列——可接受变体。 |
| **期望** | operate 一致按钮；clarify 禁止主界面 `EASYMC_LLM_*`（已基本做到）；空对话应教界面（onboard 空态）。 |
| **差距** | ① 又一份 `button.primary`。② 首屏无对话时缺少「下一步」结构化空态（仅欢迎句）。③ 窄宽 `tools { display: none }`（adapt）未提供等价入口（芯片在输入下，尚可，但应验收）。④ 字阶 0.7–1.35 混杂。 |
| **优先级** | **P1** CMP-1 / TYP-1 / 对话空态；**P2** 工具卡视觉向 agent-v1 卡片栈靠拢（非文案）。 |
| **方向** | 保留玩家向文案（im clarify 支持产品真相）；视觉 craft 可学 mock 的工具卡节奏；统一按钮与 chip token。 |

### 3.4 联机（`multiplayer-page.ts`）

| | |
|--|--|
| **现状** | 居中虚线框「即将推出」+ 要点列表；`margin: 48px auto`，**非整内容区垂直居中**。文案玩家向正确。 |
| **期望** | 占位仍是主内容区一种状态 → 二维居中（layout Extremes）。clarify：诚实进度 + 可选下一步。 |
| **差距** | 贴上半；无主 CTA（如「告诉我你想怎么联机」可选收集可 P2）。 |
| **优先级** | **P1** 占位二维居中（同 EmptyState page）；**P2** 反馈入口。 |
| **方向** | 复用 `easymc-empty-state variant="page"`；保留「即将推出」徽章但勿当 eyebrow 压在每节标题上。 |

### 3.5 设置 · 账户及其他（`settings-page.ts`）

| | |
|--|--|
| **现状** | 四组 Tab；账户 PCL 式行（皮肤占位+名+徽章）—— **IA 已定，不重开**。账户空：文案在上、`添加账户` 在下（`account-list-empty.png`）→ E2。模式切换用 primary/ghost 充当 segmented。 |
| **期望** | panel 空态文案+CTA 一组居中。operate：segmented control 有独立词表，勿滥用 primary 填满。 |
| **差距** | E2；CMP（模式钮视觉权重过大）；高级折叠内若再露环境变量须核对（主路径已清）。 |
| **优先级** | **P0** E2；**P1** 模式切换改为 tonal / segmented（非双 primary）；**P2** 皮肤站完成态。 |
| **方向** | 账户卡 flex 列 + EmptyState panel；模式用 `secondary-container` 选中而非双实心 primary。 |

### 3.6 更多（`more-page.ts`）

| | |
|--|--|
| **现状** | 分享/反馈/帮助 tile + 关于。简洁。 |
| **期望** | distill：少即是多；tile 一致 hover/focus。 |
| **差距** | tile 与别处卡片半径/边框不完全同一 token 用法；无 focus 显式（依赖全局 `:focus-visible`，尚可）。 |
| **优先级** | **P2** |
| **方向** | 共享 surface 样式；关于区勿再引入开发者栈堆砌。 |

### 3.7 Onboarding（`onboarding-page.ts`）

| | |
|--|--|
| **现状** | 三步：模式 → 登录 → 完成；进度段；选项卡；`blank-flow/01-onboarding-mode.png` 清晰。门禁：模式+≥1 账户（DESIGN）。 |
| **期望** | onboard：尽快 aha（第一次能启动游戏）；可跳过仪式；Show don't tell。 |
| **差距** | 与 im「尽量可跳过」冲突 → **§8.1**。步骤本身清晰；缺时间预期文案（可选）。完成后落到空启动页 → 依赖 E1 衔接。 |
| **优先级** | **P1** 与空启动衔接；**P2** 步骤旁「约 1 分钟」；冲突项按 §8 裁决后再改门禁。 |
| **方向** | 先不拆门禁；缩短文案、默认选中合理模式；出引导后空态 CTA 必须醒目（E1）。 |

### 3.8 空状态（横切 · 指向专文）

| | |
|--|--|
| **现状** | 组件 `src/components/empty-state.ts` 已实现 page/panel + 光学 `translateY(-4%)`；**仅启动页引用**。设置/下载/实例仍 `.empty-soft`。 |
| **期望** | layout Extremes + clarify 五类空态；见专文 §2–§4。 |
| **差距** | E2–E4、C1 铺开、T2 文案；截图证据仍偏上。 |
| **优先级** | **P0**（整包 E1–E4） |
| **方向** | **以 [`UI-EMPTY-LAYOUT-REMEDIATION.md`](./UI-EMPTY-LAYOUT-REMEDIATION.md) 为深潜 SoT**；本文 backlog 只挂 ID，不重复长篇机制。 |

### 3.9 版本管理（`instance-page.ts` / 库）

| | |
|--|--|
| **现状** | HMCL 式 Tab；多处 `.empty-soft`。 |
| **期望** | 与 E4 同一 EmptyState panel。 |
| **差距** | 空模组/世界/资源包未二维居中；加载「加载中…」。 |
| **优先级** | **P0** E4；**P1** LD-1 |
| **方向** | 同 C1 替换。 |

---

## 4. 横切主题

### 4.1 Layout & 空态

→ **专文** [`UI-EMPTY-LAYOUT-REMEDIATION.md`](./UI-EMPTY-LAYOUT-REMEDIATION.md)：E1–E4、C1–C3、T1–T3、高度契约、光学微调。  
**本主文档裁定：** 空态整改 = **P0 全集**；实现顺序专文 §8。

**壳层（`app-shell.ts`）：** `.content` 已 `flex:1; min-height:0; display:flex; flex-direction:column`；`.page` `min-height:100%` —— 方向正确。顶栏汉堡 `tabindex="-1"` 却可见 → a11y（§4.9）。

### 4.2 Typography / 层级（typeset · operate）

| 发现 | 证据 | 期望 |
|------|------|------|
| 单族正确 | `--easymc-font: "Roboto", system-ui…` | 保持 |
| 无角色 token | 各页 `font-size: 0.7`…`1.55rem` 任意 | `--easymc-type-display/title/body/label/meta` + 比例 ~1.125–1.2 |
| 字重跳跃 | 650 / 700 / 600 混用无文档化 | 限定 2–3 档（400/600/700） |
| 追踪 | 多处 `-0.02em`~`-0.03em` | 保持 ≤ -0.04em floor |

**整改 ID：TYP-1（P0）、TYP-2（P1 权重表）。**

### 4.3 Color & 对比 / AI-slop（colorize · quieter · craft-floor）

| 发现 | 证据 | 判定 |
|------|------|------|
| OKLCH roles 完整 | `tokens.css` primary/surface/semantic + dark 构图 | **已对齐** |
| Accent 克制 | 侧栏选中、主按钮、焦点环 | **已对齐** |
| 幽灵卡 | home featured、download `.card`/`.search`：shadow+border | **CF-1 P0** |
| 裸 oklch 装饰 | home `.art`、`button.launch` 渐变、settings 皮肤占位渐变 | **P1** 升 token 或声明为 signature |
| 渐变字 | 未发现 | 合规 |
| Emoji 侧栏 | 已换 SVG（`side-nav.ts`） | **已对齐** craft-floor |
| 装饰左边框 | 未发现系统性 >1px | 合规 |

Quieter：启动页版本卡选中「粗边+抬升」略吵 → P2 减重。

### 4.4 间距节奏 / 密度（layout · operate）

| 发现 | 证据 |
|------|------|
| 已有 4px 基距 | `--easymc-space-1`…`6` |
| 页内仍大量魔法数 | `18px 28px 32px`、`28px 28px 22px`、`36px` empty padding |
| 密度 | 下载/设置偏 Operate；联机占位过空且未居中 |

**SP-1（P1）：** 页 padding / 卡内 gap 改引用 space token；禁止新增任意 px。

### 4.5 Motion（animate · operate）

| 项 | 现状 | 判定 |
|----|------|------|
| Token | `--easymc-dur-*` / `--easymc-ease-out` | 对齐 |
| 页入 | `page-in` 6px + opacity | 可接受 continuity |
| Focal | 仅 `.featured` | 对齐「一处」 |
| List stagger | `store-page` 仍有；download 市场需确认 | 总延迟封顶 OK |
| Reduced motion | `global.css` 用 `animation-duration: 0.01ms` 全局 | audit 警告：须**有意保留**状态色/透明度反馈——现状过渡白名单尚可，但勿再扩大 `!important` 杀伤面 |
| 禁 bounce | 未发现 | 合规 |

**MOT-1（P2）：** 审计所有 `@keyframes`；删无状态意义的入场。

### 4.6 文案清晰度（clarify）— 玩家腔

| ID | 问题 | 证据 | 方向 |
|----|------|------|------|
| T2 | 筛选空 ≈ 首用空 | download `empty-soft` 同句 | 专文 T2 |
| CPY-1 | `placeholder-page.ts` 仍含 `EASYMC_*` / 环境变量 | L261, L280 | 删除死页或打进高级；主路由勿引用 |
| CPY-2 | Agent 欢迎句偏长 | phase5 截图 | distill：一句状态 + 芯片即下一步 |
| — | 账户/启动/设置主路径 | DESIGN Before→After 已落地 | **保持** |

### 4.7 组件一致性（operate · polish）

| ID | 问题 | 证据 |
|----|------|------|
| **CMP-1 P0** | `button.primary/ghost` 每页复制 | home / download / settings / agent / instance / store / onboarding |
| **CMP-2 P1** | Tabs 样式两套（设置 pill vs 下载 pill 近似但不共享） | settings / download |
| **CMP-3 P1** | Chips / badges 字号 0.7–0.78 漂移 | home / download / agent |
| **CMP-4 P1** | 模式切换滥用 primary | settings L423–424 |
| **CMP-5 P2** | 输入框圆角：full pill vs small 混用 | download inputs 999px；home create small |

**方向：** `src/components/ui-button.ts`（或 `styles/controls.css` 被各页 import）+ 文档化变体：`filled` / `tonal` / `outline` / `text` / `danger`。Launch CTA 可为 `filled-launch` 特例（signature move）。

### 4.8 Onboarding（onboard）

见 §3.7 与 **§8.1**。空态即引导：启动空态 CTA = onboarding 的延长线（E1）。

### 4.9 A11y / harden 可见项

| ID | 严重度 | 发现 | 证据 |
|----|--------|------|------|
| **A11Y-1** | P1 | 顶栏菜单按钮 `tabindex="-1"` 仍渲染可见 | `app-shell.ts` ~L202 |
| **A11Y-2** | P1 | 加载无 skeleton / 无 `aria-busy` | home / instance / download |
| **A11Y-3** | P2 | 对比未跑系统检测（Tauri 无 CDP 时源码审） | audit desktop 注记 |
| **A11Y-4** | P2 | 触达：部分 icon-btn 40px OK；更密芯片须 ≥24px + 间距 | agent chips |
| **HRD-1** | P1 | 长版本名 / 长用户名省略策略未统一 | harden overflow |
| **HRD-2** | P2 | 中文为主，德文扩长 N/A；仍须防按钮固定宽 | — |

---

## 5. 优先 backlog（含验收）

### 5.1 P0

| ID | 项 | 验收标准 | 建议序 |
|----|----|----------|--------|
| **E1** | 启动全页空态二维居中 | 多窗口高度下标题+CTA 在主内容区中部；对比 `home-empty.png` | 1 |
| **E2** | 账户卡空态二维居中 | 文案+「添加账户」一组居中；重拍 `account-list-empty` | 2 |
| **E3** | 下载结果区筛选空居中 | 筛栏固定；空态在结果槽中部；文案走 T2 | 3 |
| **E4** | 实例各列表 `.empty-soft` → EmptyState | 与 E2 同一 panel 变体 | 4 |
| **CMP-1** | 共享按钮词表 | 任意两页 primary 视觉一致（偏差 ≤ token）；Launch 可为唯一签名变体 | 5 |
| **TYP-1** | 角色字阶 token | 新 UI 禁止裸 `font-size: Xrem`（数据/特例除外）；标题/正文扫读可辨 | 6 |
| **CF-1** | 消灭幽灵卡 | featured / 市场卡 / 搜索条：shadow **或** border 二选一 | 7 |
| **LD-1** | Skeleton 加载 | 启动/下载列表首屏用几何骨架，禁止仅「加载中…」占主区 | 8 |
| **T2** | 筛选空文案 | 不得与首用同一句；含「清空筛选」动作 | 与 E3 同 PR |

### 5.2 P1

| ID | 项 | 验收 |
|----|----|------|
| **C1** | 全面替换为 `easymc-empty-state` | 无残留业务页 `.empty-soft` 主路径 |
| **C2** | 高度契约回归测试 | 专文 §5 |
| **SP-1** | 间距改 space token | 新增 padding/gap 均来自 `--easymc-space-*` |
| **CMP-2/3/4** | Tabs / chips / 模式 segmented | 设置模式钮不再双实心 primary |
| **A11Y-1** | 修复或隐藏无效菜单钮 | 可聚焦则有行为；否则不占 tab 序且 `hidden` |
| **A11Y-2** | `aria-busy` + skeleton | 读屏可感知加载 |
| **HRD-1** | 长文本 truncate/clamp | 版本名、账户名一帧验收 |
| **CPY-1** | 清理 placeholder 开发者腔 | 主包无用户可见 `EASYMC_*` |
| **TOK-1** | launch/art 色入 token | 组件无散落品牌 oklch |
| **MP-1** | 联机占位二维居中 | 同 EmptyState page |
| **ON-1** | 出引导 → 空启动 CTA 衔接 | 首启完成后 1 步内能点到「去下载」 |

### 5.3 P2

| ID | 项 |
|----|----|
| **MOT-1** | 删无意义入场动画；复核 reduce 白名单 |
| **Q-1** | 版本卡选中减重（quieter） |
| **AG-1** | 助手工具区视觉靠拢 agent-v1 卡片栈（文案仍玩家向） |
| **CMP-5** | 输入圆角语义统一（搜索 pill / 表单 small） |
| **A11Y-3/4** | 对比抽检、芯片触达 |
| **MORE-1** | 更多页 surface 共享化 |
| **ADAPT-1** | 窄窗助手 tools 隐藏时芯片等价验收 |

### 5.4 建议落地顺序（批准后）

1. **E1–E4 + C1 + T2**（结构失败优先，专文）  
2. **CMP-1 + CF-1**（词表与 craft floor）  
3. **TYP-1 + SP-1 + TOK-1**（系统层）  
4. **LD-1 + A11Y-***（状态与可访问）  
5. **P2 polish** → 再跑一轮 impeccable polish 心智清单  

**计数：** P0 = **9**（E1 E2 E3 E4 CMP-1 TYP-1 CF-1 LD-1 T2）· P1 = **12** · P2 = **8**。

---

## 6. 已对齐项（不要只抱怨）

| 项 | 说明 |
|----|------|
| Operate 定位 | `DESIGN.md` 明确 Operate；非 spectacle |
| 侧栏 IA | 启动/下载/助手/联机/设置/更多 + SVG 图标（非 emoji） |
| 启动主 CTA | 「启动游戏」大钮，路径正确 |
| 下载 / 账户 IA | PCL 向已定；视觉 MD3 青绿未抄 PCL 皮 |
| Token 色板 | OKLCH roles + 可选 dark 单独构图 |
| 浏览器表面 | 选区 / caret / 滚动条 / `:focus-visible` 环 |
| Motion token | duration/easing 与 DESIGN §6.4 一致；Home 单处 focal |
| 玩家向文案 | 主路径大幅去 `EASYMC_LLM_*` / uuid 腔 |
| EmptyState 组件 | API 已按专文 C1 方向落地（待铺开） |
| 无渐变字 / 无系统性左边框装饰 | craft-floor 多项已守住 |
| Onboarding 结构 | 三步清晰、进度可见、选项有 `radiogroup` |

---

## 7. 文档关系

```text
DESIGN.md                          ← 产品 SoT（IA / 文案铁律 / 视觉原则）
    │
    ├─ docs/UI-IMPECCABLE-FULL-REMEDIATION.md   ← 【本文】全量 master 整改计划
    │         │
    │         └─ includes/points to →
    │
    └─ docs/UI-EMPTY-LAYOUT-REMEDIATION.md      ← 空态二维居中深潜（E1–E4…）
```

- **本文 = master**：分页矩阵、横切、全量 backlog、与 im 冲突裁决。  
- **空态专文 = deep dive**：机制、高度契约、验收清单；**不要删、不要被本文替换**。  
- 本文提及空态时 **引用 E1–E4**，不复制专文长段。  
- 可选：在 `DESIGN.md` 附录 B 加一行链到本文（批准后由实现者加亦可）。

---

## 8. Impeccable vs 既有偏好 · 冲突点（im 优先）

落地时若冲突：**按 Impeccable 改**；下表供 William 审阅确认。

| # | 主题 | 既有偏好 / DESIGN | Impeccable | **裁决（本文）** |
|---|------|-------------------|------------|------------------|
| **8.1** | 首启可否跳过 | 模式+账户**不可跳过**门禁 | onboard：尽量可跳过、勿堵产品 | **保留账户就绪门禁**（否则无法启动=任务失败，harden/operate 任务优先）；**削弱仪式**：模式步可默认推荐+更短文案；皮肤站「稍后再说」已符合。若未来加「我有账户」快速路径，跟 im。 |
| **8.2** | Launch CTA 渐变填充 | DESIGN 签名：软青绿→绿渐变钮 | craft-floor 禁**渐变字**；quieter 减饱和；colorize 允许动作色 | **保留渐变填充为唯一 signature move**（mode-operate：世界只借 type/palette/density/**一个**签名）；**禁止**再扩散到标题/侧栏。若审计认饱和过高 → 降 chroma，不删签名。 |
| **8.3** | Agent mock 叙事 | 玩家排错，非开服优化 | clarify：产品真相；mock `agent-v1` 偏 server | **文案跟 DESIGN/玩家**；**布局工艺可学 mock**（工具卡栈、气泡）。不把「server optimization」拷回 UI。 |
| **8.4** | 账户 / 下载 IA | PCL 对齐已定 | operate：熟悉导航；不发明怪异 IA | **不重开 IA**；只改视觉 craft 与状态。 |
| **8.5** | 幽灵卡（边框+影） | 现版 featured「纸感」两边都要 | craft-floor **禁**同时粗边+宽软影 | **im 胜**：二选一；纸感用 shadow + 极淡分隔线（非 1px 实框叠 elevation）或只用 border 无影。 |
| **8.6** | 全局 reduce `0.01ms` | 省事关掉动画 | audit：禁止用全局近零时长毁掉有用反馈 | **im 胜**：保留色/透明度/宽度状态过渡；仅去掉位移/缩放。 |
| **8.7** | 加载文案 vs skeleton | 「加载中…」简单 | operate：skeleton，非内容区 spinner | **im 胜**：LD-1。 |
| **8.8** | 字阶随手写 | 各页调到「好看」 | typeset/operate：固定角色 rem | **im 胜**：TYP-1。 |

---

## 9. 非本次范围

- 任何 UI/CSS **实现**（待 William 批准本文后另开任务）。  
- Phase 3 皮肤站深度、生产 Azure Client ID、联机真实实现。  
- 重开侧栏信息架构或「要不要 PCL 皮」。  
- 用本文替换空态专文（禁止）。

---

## 10. 审阅清单（给 William）

- [ ] 同意 §1.2 Top 5 P0  
- [ ] 同意 §8 冲突裁决（尤其 8.1 门禁、8.2 签名渐变、8.5 幽灵卡）  
- [ ] 确认空态专文仍作 E1–E4 深潜  
- [ ] 批准后按 §5.4 顺序开工（**批准前不大改 UI**）

---

*依据：Impeccable reference（2026-10-24 raw）、EasyMC 树（含 `empty-state` 组件与主路径页）、`DESIGN.md`、`easymc-shots/**`、`easymc-mocks/**`、既有 `UI-EMPTY-LAYOUT-REMEDIATION.md`。*
