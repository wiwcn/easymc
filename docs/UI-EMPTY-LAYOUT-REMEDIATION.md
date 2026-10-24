# EasyMC UI 整改文档：空状态与布局（对照 Impeccable）

**日期：** 2026-10-24（Asia/Shanghai）  
**触发：** William 反馈「文字尽可能居中」落地后，空状态**只左右居中、未上下居中**。  
**范围：** 对照 [Impeccable](https://impeccable.style/)（`layout` / `clarify` / Operate 模式）与现版源码、截图，列出必须整改项；**本文只出方案与验收，不直接改代码**。  
**关联：** 仓库根目录 `DESIGN.md`（Operate 模式、玩家向文案）；截图 `/workspace/easymc-shots/{launch-polish,download-pcl,accounts-pcl}/`。

---

## 1. 结论（先看这里）

| 判定 | 说明 |
|------|------|
| **成立** | 启动页全页空状态、设置页账户空状态等，目前基本是「水平居中 + 顶对齐」，**没有在内容区剩余高度内垂直居中**。 |
| **根因** | 空状态节点只有 `text-align: center` / `align-items: center`，**缺少「占满父级可用高度 + `justify-content: center`」**；父级 `.content` 虽 `flex: 1`，子页空状态未吃满高度。 |
| **规范依据** | Impeccable **layout · Extremes**：empty states 不得暴露结构失败——空态应占据本应放内容的区域，而不是顶上一小段。Operate 模式要求可预测结构；全页/大容器空态是主内容区的一种状态，视觉重心应在区域中央。Impeccable **clarify**：空态要区分首用 / 无结果 / 筛选，并给出下一步（文案方向已大致对，布局未跟上）。 |
| **优先级** | **P0** 全页空态垂直居中；**P0** 卡片内大空态在卡片可用区内垂直居中；**P1** 统一 `EmptyState` 组件；**P2** 筛选无结果与首用空态区分文案/密度。 |

---

## 2. Impeccable 对照要点（与本问题相关）

来源：`pbakaus/impeccable` → `skill/reference/layout.md`、`clarify.md`；产品站 `/arrange`；本仓库 `DESIGN.md` 已声明 **Operate**。

### 2.1 layout（结构）

- **Extremes**：长内容、**empty states**、overlay、安全区等要单独验；空态若顶对齐、下方大片留白，属于**结构失败**（内容区「空」了，但空态没有接管该区域）。
- **Operate**：稳定密度、可扫读；空态不是营销页，但仍是**该任务路径上的主状态**，应落在主内容区几何中心附近，而不是贴顶。
- **Verify**：验证项明确写到 *Long text, empty states … do not break the structure*——需用渲染证据（截图）证明空态占用正确区域。

### 2.2 clarify（文案）

- 空态要区分：**首次使用 / 无结果 / 筛选过严 / 权限 / 失败**。
- 结构：说明状态 + **下一步有用动作**（标题、一句说明、主 CTA；可选次要 CTA）。
- 现版「咦？似乎什么都没有…」语气符合玩家向；**问题在布局，不在口号本身**。

### 2.3 本产品已有约定（DESIGN.md）

- 模式 **Operate**：可扫读、任务优先。
- 空状态口吻轻松、居中——**「居中」必须理解为内容区二维居中（水平 + 垂直）**，不是仅 `text-align: center`。

### 2.4 业界同构（辅助理解，非替代 Impeccable）

全页/大容器空态应对齐容器中心（水平 + 垂直）；仅水平居中、贴顶是常见反例。卡片内空态应对齐**该卡片内容区**中心，而不是整窗。

---

## 3. 现版证据

### 3.1 截图（观感）

| 截图 | 现象 |
|------|------|
| `easymc-shots/launch-polish/home-empty.png` | 「咦？似乎什么都没有…」在主内容区**偏上**；下方大片空白，**未垂直居中**。 |
| `easymc-shots/accounts-pcl/account-list-empty.png` | 「咦？还没有账户呢…」在账户卡片**上半部**；卡片下半空，**未在卡片内垂直居中**；「添加账户」贴卡片底，与文案脱节。 |
| `easymc-shots/download-pcl/vanilla-empty-filter.png` | 筛选无结果：需按同标准验收（是否贴顶 / 是否占满结果区）。 |

### 3.2 源码（机制）

**启动页全页空态** — `src/pages/home-page.ts` `.empty`：

```css
.empty {
  text-align: center;
  padding: 48px 20px 32px;
  display: flex;
  flex-direction: column;
  align-items: center;   /* 仅交叉轴居中 = 水平 */
  /* 缺失：min-height: 100% / flex: 1；justify-content: center */
}
```

**壳层内容区** — `src/app-shell.ts` `.content`：

```css
.content {
  flex: 1;
  overflow: auto;
  padding: 18px 28px 32px;
}
```

子页默认是文档流高度 = 内容高度，空态块**不会**被拉高到视口剩余高度，因此 `justify-content: center` 即使加上也无「可居中的高度」——必须先让空态（或页面根）**吃满 `.content` 可视高度**。

**设置页账户空态** — `src/pages/settings-page.ts` `.empty-soft`：

```css
.empty-soft {
  text-align: center;
  padding: 28px 16px;
  margin: 8px 0;
  /* 无 flex 铺满卡片、无 justify-content: center */
}
```

同类：`instance-page.ts` `.empty-soft`（版本列表 / 模组 / 存档 / 资源包 / 光影）。

---

## 4. 整改清单

### P0 — 必须立刻改

| ID | 位置 | 问题 | 整改要求 | 验收 |
|----|------|------|----------|------|
| E1 | 启动页无版本 `home-page` `.empty` | 仅水平居中，贴顶 | 空态容器占满主内容区可用高度（扣除顶栏后 `.content` 内），`display:flex; flex-direction:column; align-items:center; justify-content:center;`；避免靠大 `padding-top` 假装居中 | 不同窗口高度下，标题+说明+按钮组视觉落在内容区中部；截图对比 `home-empty.png` |
| E2 | 设置 · 账户列表空 | 文案贴卡片顶，CTA 贴底 | 账户卡片在空态时成为纵向 flex；中间块（文案±插画）`flex:1` + 居中；主 CTA 可紧跟文案（一组居中）或固定卡片底——**推荐文案与主 CTA 作为一组整体垂直居中**，避免「字在上、钮在底」 | `account-list-empty` 重拍 |
| E3 | 下载 · 筛选无结果 | 结果区可能贴顶 | 结果列表区域（不是整页含 Tab）内二维居中；保留筛选栏位置 | `vanilla-empty-filter` 重拍 |
| E4 | 实例/库各类 `.empty-soft` | 同 E2 | 与 E2 同一组件/同一 token | 各空列表一帧 |

### P1 — 结构统一（防复发）

| ID | 项 | 要求 |
|----|----|------|
| C1 | 抽出 `EmptyState`（Lit） | Props：`title`、`description?`、`primaryAction?`、`secondaryAction?`、`variant: 'page' \| 'panel'`。`page`：占满父级剩余高度并二维居中；`panel`：占满所在卡片/列表槽并二维居中。 |
| C2 | 高度契约 | `app-shell` 的 `.content` 保持 `flex:1; min-height:0`；页面根在空态时 `min-height: 100%` 或 `height: 100%` + flex 列，保证子级有可分配高度。禁止用魔法数字 `min-height: 60vh` 作为唯一手段（可作 fallback）。 |
| C3 | DESIGN.md 补一句 | 「空状态居中 = 在所属内容容器内水平+垂直居中；全页用 page 变体，卡片内用 panel 变体。」 |

### P2 — 文案与类型（clarify）

| ID | 场景 | 建议 |
|----|------|------|
| T1 | 首用（从未有数据） | 保留轻松口吻 + 明确主路径（添加 / 去下载） |
| T2 | 筛选无结果 | 勿用「什么都没有」与首用完全同一句；说明「没有符合筛选的…」+「清空筛选」 |
| T3 | 权限/失败 | 单独错误态，不与空列表混用 |

---

## 5. 推荐实现要点（给实现者）

```text
.content (flex:1, overflow:auto)
  └─ page root (min-height: 100%; display:flex; flex-direction:column)
       └─ EmptyState[page] (flex:1; display:flex; align-items:center; justify-content:center)
            └─ stack (title / sub / actions)  ← 这一组才是视觉中心
```

卡片内：

```text
.card (display:flex; flex-direction:column; min-height: … 足够显示空态)
  └─ EmptyState[panel] (flex:1; … center)
```

**不要：**

- 只写 `text-align: center` 或只 `align-items: center` 就宣称「居中」；
- 用 `padding-top: 20vh` 等硬编码冒充垂直居中；
- 全页空态与导航栏、侧栏抢中心（中心相对**主内容区**，不是整窗含侧栏）。

**光学微调（Impeccable：optical corrections after render）：**  
几何中心略偏上 4–8% 可读性更好时可做，但须在真居中之后微调，且用 token（如 `translateY(-4%)`）写明，禁止回到「贴顶 + 大 padding」。

---

## 6. 验收清单（改完打勾）

- [ ] `home-empty`：主文案+按钮在主内容区垂直居中（多窗口高度各一帧）
- [ ] `account-list-empty`：文案与「添加账户」作为一组在账户卡片内垂直居中
- [ ] 下载筛选空、实例模组/存档等 panel 空态同上
- [ ] 有数据时布局与现版一致，无回归
- [ ] DESIGN.md 已补「二维居中」定义
- [ ] 提交说明引用本文档章节（E1–E4 / C1）

---

## 7. 非本次范围

- 配色 / 动效 / impeccable polish 全量扫描（可在空态布局达标后另开 `/impeccable polish`）。
- Phase 3 皮肤站深度、生产 Azure 客户端 ID。
- 侧栏 IA、启动主卡视觉（已在 `4d6836e` 批次，与空态垂直居中正交）。

---

## 8. 建议落地顺序

1. **E1 + 高度契约（C2）** — 启动页最刺眼，先堵住「只左右居中」。  
2. **C1 EmptyState** — 立刻替换 E2/E3/E4，避免复制粘贴再偏。  
3. **T2 筛选文案** — 与下载空态同 PR 即可。  
4. 更新 DESIGN.md（C3）并重拍 `easymc-shots/**` 空态组。

---

*本文依据：Impeccable layout/clarify（2026-10-24 拉取的公开 reference）、EasyMC `4d6836e` 树、上述截图与 William 原话「文字尽可能的居中」「你这只左右居中了没上下居中」。*
