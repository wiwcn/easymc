# EasyMC 运行实例化方案：Alpine 子系统 + 硬件直通

**日期：** 2026-10-24
**触发：** William「大活是将 Minecraft 运行彻底实例化，通过安装一个 Alpine 子系统通过硬件直通运行，可以参考 github 上的项目比如 kokodio/metallum 这种调用渲染器，这样 agents 只需要分析容器，内容隔离不会污染，并且跨平台好做」
**范围：** 把「运行 Minecraft」从「在宿主文件系统上直接 `java -jar`」升级为「每个实例一个可丢弃的隔离运行单元」，并定义宿主 App、Agent、运行单元三者的接口契约。
**性质：** 技术方案 + 分阶段落地 + 风险清单。**评审稿，尚未开工**——§9 的 D1–D3 已于 2026-10-24 拍板，D5 = 暂不实施（先评审）。
**关联：** [`DESIGN.md`](../DESIGN.md)（产品 SoT）· [`launch.rs`](../crates/easymc-launch-desktop/src/launch.rs)（当前启动链路）· [`java.rs`](../crates/easymc-launch-desktop/src/java.rs)（Java 探测/安装）· [`paths.rs`](../crates/easymc-core/src/paths.rs)（数据布局）

---

## 1. 结论（先看这里）

| 判定 | 说明 |
|------|------|
| **方向可行** | 「实例化运行」在 **Windows（WSL2）** 和 **Linux（rootless 容器）** 上都有成熟的**硬件直通**路径，GL 性能可接近原生；这两条是主战场。 |
| **参考项目要重新理解** | `kokodio/metallum` **不是容器方案**：它是 macOS 上「用 Apple Metal 替换 OpenGL/Vulkan」的 **Fabric 渲染 mod**（Java/Mixin，仓库 100% Java）。它证明的是**「渲染后端可替换/可直通」**，与「隔离」是**两个正交维度**。本方案在不同平台分别解决这两件事。 |
| **macOS 是硬边界** | Apple Silicon **没有**把 GPU 直通给 Linux 虚拟机的官方能力（Lima/UTM 只有 virtio-gpu → 软件渲染，跑 MC 不现实）。所以 macOS 上「Linux 容器 + 硬件直通」**不成立**；macOS 走**原生**加速（Metal/OpenGL-over-Metal，即 metallum 那类思路），用隔离性换性能。 |
| **Alpine 有坑** | Minecraft 的 **LWJGL 本地库是 glibc 链接**的，Alpine 是 **musl**。直接用 Alpine 需要 `gcompat` + `libstdc++` 兜底，且不保证所有 native 都能跑。**镜像基底需二选一**（§9-D1）：Alpine+gcompat，或 Debian-slim（更稳但更大）。 |
| **默认不破坏现状** | 新增 `RuntimeBackend` 抽象，**`NativeBackend` = 今天的直接启动**，作为永远可用的兜底。任何后端探测失败都静默回落，绝不挡住用户开玩。 |
| **最大收益点** | ① 内容（mods/存档/日志）**只存在于实例卷**，不污染宿主；② Agent 通过**结构化检查接口**读容器，而不是满盘找文件；③ 卸载 = 删一个目录，天然可回滚。 |

---

## 2. 需求拆解

| # | 需求 | 本方案对应 |
|---|------|-----------|
| R1 | 运行**彻底实例化** | 每个实例 = 一个「不可变镜像层 + 可写数据卷」的运行单元（§5.3） |
| R2 | Alpine **子系统** | 以 Linux 用户态根文件系统作为运行环境基底（Windows 走 WSL2 导入，Linux 走容器）（§4） |
| R3 | **硬件直通** | GPU：`/dev/dxg`(Windows) / `/dev/dri`(Linux)；显示：WSLg(Wayland) / 宿主 Wayland/X11 socket（§5.4） |
| R4 | **Agent 只分析容器** | `RuntimeBackend::inspect()` 结构化接口 + 默认无宿主 FS 访问（§5.5、§6） |
| R5 | **内容隔离、不污染** | 实例卷是唯一可写区；宿主与实例互相只见约定挂载点（§5.3） |
| R6 | **跨平台好做** | 后端抽象 + 平台矩阵 + 优雅降级（§4、§5.1） |

**非目标（明确不做）：**
- 不做 Minecraft 私有协议 / 账号系统改造。
- 不重新打包分发游戏文件（仍用官方 jar / 官方下载源）。
- 不在 macOS 上强行做 Linux 容器加速（做不了，见 §4.3）。
- 不承诺「容器 = 强安全沙箱」；WSL2 是 VM 边界，Linux 容器不是（§8）。

---

## 3. 现状与技术澄清

### 3.1 今天的启动链路（保留为 Native 兜底）

- [`launch.rs`](../crates/easymc-launch-desktop/src/launch.rs)：拼 classpath / JVM 参数 / natives 目录 → 交给 `process.rs` 起子进程。
- [`java.rs`](../crates/easymc-launch-desktop/src/java.rs)：`detect_java()` 跨平台探测 + `ensure_java()` 下载 Temurin 到 `runtimes/`。
- [`paths.rs`](../crates/easymc-core/src/paths.rs)：`versions/ libraries/ assets/ instances/<id>/.minecraft`。

**这套保持不变**，未来只是把「执行体」从「宿主 `java`」换成「运行单元里的 `java`」。

### 3.2 metallum 到底做了什么（避免误判）

- 形态：**Fabric mod**，Mixin 进原版渲染管线，用 **Metal** 提交绘制，替代 OpenGL/Vulkan 后端。
- 平台：**仅 macOS + Apple Silicon**。
- 与隔离的关系：**无**。它运行在**宿主**、进程**未被隔离**，是纯粹的「渲染器直通/替换」。

**对本方案的启示：** 「渲染加速」和「运行隔离」要拆成两层分别做；指望一个技术同时解决两者会两头不讨好。

---

## 4. 平台矩阵（这是全案的骨架）

| 平台 | 运行基底（隔离） | GPU 直通 | 显示桥 | 结论 |
|------|-----------------|----------|--------|------|
| **Windows 10 21H2+ / 11** | **WSL2**：`wsl --import easymc <dir> rootfs.tar` 导入 Alpine/Debian | WSL2 GPU 半虚拟化：宿主 `dxgkrnl` → 客体 `/dev/dxg` + Mesa `d3d12` | **WSLg**：Wayland + XWayland + PulseAudio，自动注入 | ✅ **主方案**。VM 级隔离 + 硬件加速 |
| **Linux** | **rootless 容器**：`bubblewrap`（首选）或 `podman` | 直接 bind `/dev/dri`（AMD/Intel）；NVIDIA 需 `nvidia-container-toolkit` 或厂商库 bind | 宿主 `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY` socket bind；无 Wayland 时回落 X11 socket | ✅ **主方案**。近乎原生 |
| **macOS (Apple Silicon)** | ❌ 无 GPU 直通给 Linux VM | — | — | ⚠️ 走**原生**加速运行（OpenGL-Over-Metal / metallum 类）；**不做 Linux 容器** |
| **macOS (Intel)** | VM 可跑但极慢 | ❌ | — | ⚠️ 同 Apple Silicon：原生 |
| **兜底（任意平台）** | **`NativeBackend`**：宿主直接起进程（= 今天的链路） | 原生 | 原生 | ✅ 永远可用 |

**关键门槛（Windows）：** 需要 `Windows 10 21H2+` 或 `Windows 11` + `WDDM 2.9` 驱动（`dxdiag` 可查）+ WSL2 已启用。不满足 → 回落 `NativeBackend`。

---

## 5. 架构设计

### 5.1 后端抽象（新增 crate：`crates/easymc-runtime`）

```
trait RuntimeBackend {
    fn kind(&self) -> BackendKind;              // Native | Wsl | Container
    fn probe(&self) -> RuntimeProbe;            // 可用性 + 原因（给 UI 展示）
    async fn provision(&self, spec: &ImageSpec, on_log: LogFn) -> Result<ImageHandle>;
    async fn start(&self, handle: &ImageHandle, req: &RunRequest) -> Result<RunHandle>;
    async fn status(&self, run: &RunHandle) -> Result<RunState>;   // Running/Exited(code)/Failed
    async fn stop(&self, run: &RunHandle) -> Result<()>;
    fn exec(&self, cmd: &[String]) -> Result<ExecOutput>;          // 一次性命令
    async fn inspect(&self, run: &RunHandle, q: &InspectQuery) -> Result<InspectResult>; // §5.5
    async fn destroy(&self, handle: &ImageHandle) -> Result<()>;   // 删除实例单元
}
```

- `BackendKind`：`Native` | `Wsl` | `Container`。选择顺序 = 平台首选 → 探测失败 → `Native`。
- `probe()` 必须返回**可读原因**（如「WSL 未启用」「驱动 WDDM 版本过低」），前端直接展示，不静默失败。
- 模块：`native.rs` / `container.rs`（linux） / `wsl.rs`（windows） / `mod.rs`（选择与 facade）。

### 5.2 与现有代码的接入点

- `easymc-launch-desktop::launch` 产出的是「JVM 参数 + classpath + natives + 环境变量」。把这部分抽成**平台无关的 `LaunchSpec`**，交给 `RuntimeBackend` 执行；`NativeBackend` 的实现就是现有 `process.rs` 逻辑。**这是 P0 重构，行为零变化。**
- Tauri 命令（`src-tauri/src/lib.rs`）新增：`runtime_probe_cmd` / `runtime_provision_cmd` / `runtime_launch_cmd` / `runtime_inspect_cmd` / `runtime_reset_cmd`。
- 前端：设置页「运行环境」卡展示后端 + 状态；实例页加「隔离运行」开关（默认开，探测不可用时灰掉并说明原因）。

### 5.3 实例单元布局（R1 / R5）

```
<data_dir>/
  images/                     # 不可变镜像层（按内容哈希命名，可共享）
    debian-slim-mesa-jre21-<sha256>/
      rootfs/                 # 解开的用户态根
      manifest.json           # 基底 + apk/apt 包锁 + 哈希
  instances/<id>/
    .minecraft/               # ← 唯一可写数据卷（存档/mods/配置/日志）
    runtime.json              # 绑定哪个 image、GPU/显示挂载、资源限额
```

- **不可变**：`.minecraft` 之外的系统层**从不写入**；同一镜像可被多实例共享。
- **可丢弃**：删 `instances/<id>/` 即彻底卸载，零残留。
- **挂载约定**（唯一跨边界面）：
  - 客体 → 宿主：日志、崩溃报告（`logs/`、`crash-reports/`）**只读导出**给 Agent。
  - 宿主 → 客体：`.minecraft` 卷（读写）、`libraries/`+`assets/`+`versions/`（**只读**，避免容器改坏共享缓存）。

### 5.4 GPU / 显示直通（R3）

**Windows / WSL2**
- GPU：WSL2 自动暴露 `/dev/dxg`；镜像内装 `mesa`（含 `d3d12` gallium 驱动），设 `MESA_D3D12_DEFAULT_ADAPTER_NAME` 可选。
- 显示：WSLg 提供 `/mnt/wslg`（`wayland-0` socket、`DISPLAY`、`PULSE_SERVER`）。镜像内装 `libwayland-client` + `xwayland`。
- 校验：`glxinfo -B` 应显示 renderer 为 `D3D12 (…)`。

**Linux**
- GPU：`--dev-bind /dev/dri /dev/dri`（AMD/Intel）。NVIDIA：bind `/dev/nvidia*` + 宿主的 `libnvidia-*.so*`（或 `nvidia-container-toolkit`）。
- 显示：bind `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY`；X11 兜底 bind `/tmp/.X11-unix/X0` + `XAUTHORITY`。
- 校验：容器内 `glxinfo -B` renderer 应为真实 GPU 名。

**落不了地时的兜底**：渲染退化为 `llvmpipe`（软件）→ 在 UI 明确标注「软件渲染，仅建议用于排障」，不假装加速成功。

### 5.5 Agent 检查面（R4）—— 本次最核心的契约

Agent **不直接读宿主文件系统**，只调 `inspect()`：

| query | 返回 |
|-------|------|
| `Fs { path, depth }` | 客体 `.minecraft` 内的目录树（受限路径白名单） |
| `Log { tail }` | `logs/latest.log` / `crash-reports/*` 尾部 N 行 |
| `Proc` | 客体进程树 + 资源占用 |
| `Gpu` | 渲染后端、驱动、是否软件渲染 |
| `Meta` | 实例 id、镜像哈希、JVM 参数、Java 版本 |

好处：Agent 的输入是**稳定 schema**，不受宿主路径差异影响；同时天然限制了它能看到的范围。这也直接满足「agents 只需要分析容器」。

---

## 6. 关键数据流

```
用户点「启动」
  └─> 后端选择（probe：Wsl/Container → 否则 Native）
        └─> 确保镜像已 provision（命中哈希则复用；否则构建 + 落盘）
              └─> 组装 LaunchSpec（classpath 全指向客体挂载点）
                    └─> backend.start()
                          ├─ 直通 GPU/显示挂载
                          ├─ stdout/stderr → 现有 "launch-log" 事件（UI 复用）
                          └─ RunHandle(id) 存入 AppState
游戏崩溃 / 用户点「让助手看看」
  └─> agent → runtime_inspect_cmd(Log/Proc/Gpu/Meta) → 结构化诊断
```

---

## 7. 分阶段实施

| 阶段 | 内容 | 风险 | 可验证结果 |
|------|------|------|-----------|
| **P0** | 抽 `LaunchSpec` + `RuntimeBackend` trait；实现 `NativeBackend`（= 现有逻辑搬家） | 极低（行为等价） | 全平台启动行为与今天**完全一致**；`cargo test` 绿 |
| **P1** | `ContainerBackend`（Linux，bubblewrap + `/dev/dri` + Wayland socket）；镜像 = Debian-slim + mesa + Temurin | 中（NVIDIA 库绑定） | Linux 上启动 1.21.1 原版/Fabric，`glxinfo` 为真实 GPU，FPS 与原生同档 |
| **P2** | `WslBackend`（`wsl --import`；WSLg + `/dev/dxg`）；示例 `wsl.conf` / `unshare` 计费验证 | 中高（驱动门槛、WSLg 依赖） | Win11 上启动成功，`glxinfo` = `D3D12`；不满足门槛时**明确提示 + 回落** |
| **P3** | `inspect()` 契约 + Agent 接入；`runtime-inspect` 命令与日志导出 | 低 | Agent 仅凭 inspect 输出能定位一次崩溃原因 |
| **P4** | macOS：判定并落地「原生加速」路径说明；`NativeBackend` 提示文案 | 低 | macOS 上明确显示「原生运行（无隔离）」而不是假装隔离成功 |
| **P5** | 镜像构建进 CI（预烘焙，免运行时联网）、缓存与 GC、实例迁移 | 低 | 首启无需联网即可开工；镜像可被多实例共享 |

**建议顺序**：P0 立即做（无风险且是地基）→ P1 拿 Linux 验证整条链路 → P2 上 Windows（用户主力）→ P3 接 Agent。

---

## 8. 风险与对策

| 风险 | 影响 | 对策 |
|------|------|------|
| **musl vs glibc（Alpine）** | LWJGL native 加载失败，起不了游戏 | 首选 Debian-slim 基底；若坚持 Alpine：装 `gcompat` + `libstdc++` + `libgcc`，并把「native 全绿」列为 P1 验收项 |
| **Windows 驱动门槛** | 老机器无 WDDM 2.9 / 未启 WSL2 | `probe()` 提前检测并**给出可操作提示**；不满足即回落 Native |
| **NVIDIA 在容器内** | `libnvidia-*.so` 版本错配导致 GL 失败 | 从宿主 bind 同版本库；探测失败就回落，绝不半途崩 |
| **容器 ≠ 安全沙箱** | 用户误以为「隔离 = 安全」 | 文案明确：Linux 容器是**内容隔离**，非安全边界；WSL2 才接近 VM 边界。文档与 UI 一致表述 |
| **性能开销** | WSL2 有一次 VM + GPU-PV 开销 | 基准测试（原版/Fabric 各 1 版本）纳入 P2 验收；若不达标则该平台默认回落 Native |
| **首次下载体积** | 镜像数百 MB，首启慢 | CI 预烘焙 + 多实例共享 + 断点续传；UI 显示镜像下载进度（复用现有进度条组件） |
| **Mojang EULA** | 分发/运行需合规 | 不重新分发游戏文件，只在客体里跑**官方下载**的内容 |

---

## 9. 待决策项

| # | 问题 | 选项 | 建议 | 决议（2026-10-24） |
|---|------|------|------|-------------------|
| **D1** | 镜像基底 | (a) Alpine + `gcompat`（小、贴合原意） / (b) Debian-slim（稳、约大 100MB） | (b) 为先 | ✅ **(b) Debian-slim**；先保证「能跑」，Alpine 列为 P5 体积优化 |
| **D2** | Linux 容器运行时 | (a) 自带 `bubblewrap`（零安装、最轻） / (b) 依赖系统 `podman`（功能全，需用户装） | (a) | ✅ **(a) bubblewrap**（App 自带二进制） |
| **D3** | 默认是否开启 | (a) 默认隔离、可关 / (b) 默认原生、可开 | P1/P2 期内默认原生 | ✅ **(b) 默认原生、可开**；后端稳定后渐进翻默认 |
| **D4** | macOS 定位 | (a) 只做原生 + 明示无隔离 / (b) 尝试 VM+软渲染（不推荐） | (a) | ⏳ 随 P4 确认（建议 (a)） |
| **D5** | 是否现在开工 P0 | P0 无行为变化、纯重构 | 建议先做 P0 | ⏸ **暂不动代码**——本方案先作为评审稿 |

---

## 10. 验收标准（Definition of Done）

- **功能**：Linux 与 Win11 各能启动「原版 1.21.1」与「Fabric 1.21.1」，进世界不闪退。
- **加速**：`glxinfo -B` 报真实 GPU（Linux）/ `D3D12`（WSLg），非 `llvmpipe`；FPS 与原生同档（±10%）。
- **隔离**：删除 `instances/<id>/` 后**无残留**；镜像层 mtime 在整局游戏前后**不变**。
- **Agent**：仅凭 `inspect()` 输出可定位一次人为制造的崩溃（附回归用例）。
- **降级**：在无 WSL2 的 Win10 / 无 `/dev/dri` 的机器上，**自动回落 Native 且给出明确原因**，无黑屏、无卡死。
- **不回归**：`NativeBackend` 路径的启动行为与改造前**逐字节一致**（同一 LaunchSpec）。

