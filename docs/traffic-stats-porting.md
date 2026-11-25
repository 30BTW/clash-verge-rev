# Clash Verge 自用版：功能移植与维护指南

> 本文档面向未来的维护者（人或 AI 会话）：当 Clash Verge Rev 发布新版本、需要把自用功能
> 重新移植到新版时，按此操作。移植一次约 0.5~2 小时/功能（不含编译）。
> 最后更新：2026-11-25，基于 **v2.6.0-dev，upstream/dev `4015f37e`**，自用分支
> `feature/traffic-stats-v3`（本工作树 `worker/t-20261125-cvr-v3`）。

## 自用版包含的功能

| 功能 | 来源 PR | v3 落地提交 | 备注 |
|---|---|---|---|
| 流量统计页（按域名/进程） | 自研 #8563 | b8888df1 + c218e0e0 + ca5df3b4 | 见下文专节 |
| Windows WiFi 热点共享 TUN | #8550 + #8562 | 36935549 + 2e836abb | 单测来自 #8562 |
| 节点旁快捷流媒体解锁检测 | #8369 | 37cff43c | 前端-only，无冲突落地 |

### v3 评估后放弃的功能

| 功能 | 来源 PR | 原因 |
|---|---|---|
| 筛选后测延迟只测筛选节点 | #8463（24443e24） | `proxy-render.tsx` / `use-filter-sort.ts` 与上游 `#8621`/`#8622` 的代理页重构大面积冲突 |
| 节点点击显示真实下载速度 | #8479（8d6f55a2 + e1ea1d79） | `feat/clash.rs`/`verge.rs` 与上游测速重构冲突 5 个文件 |
| 连接页右键加分流规则 | #7956（25810c7f） | 上游 `#8649` 已提供同类能力，保持上游实现 |

被放弃的 P1 可以直接从原分支重取（`git fetch origin pull/N/head:pr-N`），等上游代理/测速重构
稳定后再评估。

### 热点功能使用须知

- 仅 Windows、需 TUN 模式；入口：设置 → 无线热点
- 用 Windows 官方移动热点 API，无需管理员；正常退出自动关热点并还原系统原热点配置
- **崩溃/强杀时热点会残留**（需去 Windows 设置手动关）
- 连热点的设备可访问宿主机 192.168.137.1 上开放的端口（Windows ICS 固有行为）

## 流量统计

后台每秒轮询 mihomo `/connections`，通过连接 ID diff 检测已关闭的连接，按连接的累计上传/下载
字节数生成记录，批量写入本地 SQLite（WAL 模式）。前端"流量统计"页面按域名/进程聚合展示，
支持时间范围筛选与排序。

- 数据库：`%APPDATA%\io.github.clash-verge-rev.clash-verge-rev\traffic_stats.db`
- 容量策略：主库+WAL 合计 100MB 上限（超限删最旧 30% 循环），90 天滚动保留
- 固有精度边界：存活不足 1 秒的连接、连接关闭前最后约 1 秒的流量不计入（页面有脚注）

## v3 移植要点（相对 v2 的变化）

### `smartstring` → `tristr`

上游已把 `smartstring::alias::String` 替换为 `tristr::alias::String`（`TriStr`）。移植时：

- `config/verge.rs`、`core/notify/notification.rs` 等文件用 `use tristr::alias::String;`。
- `core/hotspot/*` 内部统一用 `std::string::String`，无需改动；只有
  `cmd/hotspot.rs` 的 Windows `save_hotspot_config` 需要把表单值转成 `IVerge` 字段类型，
  因此那里 `use tristr::alias::String;`（`From<std::string::String> for TriStr`）。
- 全仓不得再出现 `smartstring` / `SmartString`。

### 跨平台 `IVerge` 与 `bindings.ts`

`IVerge` 的 5 个热点字段（`hotspot_ssid`、`hotspot_password`、`hotspot_band`、
`hotspot_adapter_id`、`hotspot_auto_start`）现在**无条件编译**（保留
`#[serde(skip_serializing_if = "Option::is_none")]`，密码字段仍走
`serialize_encrypted`/`deserialize_encrypted`），这样在 Linux 上 `pnpm gen:contract`
也能把它们写进 `src/services/bindings.ts`。

仍按平台区分的部分：

- 运行时实现：`core/hotspot/win.rs`（`#[cfg(windows)]`）与
  `core/hotspot/unsupported.rs`（`#[cfg(not(windows))]`）。
- `cmd/hotspot.rs` 的 `save_hotspot_config` 有 Windows 实现与返回
  "only supported on Windows" 的桩实现。
- `feat/effects.rs` 的 `verge_registry!` 用无 rest pattern 解构 `IVerge`，因此热点字段的
  注册项也必须无条件列出，否则 Linux 编译失败。
- `ipc.rs` 的 `PlatformVergeFields` 不再需要重复声明热点字段（它们已是 `IVerge` 真实字段）。

### `win.rs` 交叉审计（Linux 上无法编译 Windows 目标，需人工核对）

- `windows` 0.62.2 + 所需 features：`Foundation`、`Foundation_Collections`、
  `Networking_Connectivity`、`Networking_NetworkOperators`、`Devices_WiFi`、
  `Win32_System_WinRT`、`Win32_Globalization`。
- `windows-sys` 0.61.2 + `Win32_Foundation`、`Win32_NetworkManagement_IpHelper`、
  `Win32_Networking_WinSock`（工作区已开启；根 `Cargo.toml` 另补
  `Win32_NetworkManagement_Ndis`）。
- WinRT 全部调用在专用线程的 MTA 中执行（`RoInitialize(RO_INIT_MULTITHREADED)` /
  `RoUninitialize`），不跨 Tauri runtime。
- 依赖内部接口均已对照 `upstream/dev` 核对：`Config::verge()`/`Config::runtime()` 的
  `latest_arc()`、`Handle::notify_hotspot_status`、`singleton!`、`crate::APP_HANDLE`、
  `feat::window::quit → stop_hotspot_before_core`。
- 非 Windows 下用 `#[cfg(windows)]` 收掉仅 Windows 使用的常量/方法，并以
  `#[allow(clippy::unnecessary_wraps)]` / `#[allow(clippy::unused_async)]` 保持桩实现与
  Windows 版签名一致，保证 `cargo clippy-all -D warnings` 在 Linux 也通过。

### 生成物命名

当前上游用的 specta 导出会为同时出现在命令入参与返回值的类型生成
`_Serialize`/`_Deserialize` 后缀（如 `HotspotStatus_Serialize`、
`HotspotStartConfig_Deserialize`）。因此 `src/services/cmds.ts` 里继续使用
`_Serialize`/`_Deserialize` 别名；**不要**套用旧版 v2.5.10 的"无后缀"适配。
每次移植后必须重跑 `pnpm gen:contract`，并确认 `bindings.ts` 同时含有
`upstream/dev` 的 `ProxyViewV1_Serialize`、`VergeEventPayloads_Serialize`、
`switchProxyNode` 以及 `traffic_stats` + `hotspot` 命令与 `IVerge.hotspot_*` 字段。

## 文件清单（移植 = 把这些改动重新落到新版代码上）

**新增文件（原样复制即可）：**

| 文件 | 说明 |
|---|---|
| `src-tauri/src/traffic_stats/{mod,types,monitor,batcher,store,manager}.rs` | 后端模块 |
| `src-tauri/src/cmd/traffic_stats.rs` | 两个 Tauri 命令（specta 化） |
| `src-tauri/src/core/hotspot/{mod,win,unsupported}.rs` | 热点状态机 + WinRT 实现 + 桩 |
| `src-tauri/src/cmd/hotspot.rs` | 热点命令与配置保存 |
| `src/pages/traffic-stats.tsx` | 前端页面 |
| `src/components/setting/setting-hotspot.tsx` | 热点设置区块 |
| `src/components/setting/mods/hotspot-viewer.tsx` | 热点配置弹窗 |
| `src/hooks/use-hotspot.ts` | 热点 hooks |
| `src/assets/image/itemicon/traffic-stats.svg` | 侧边栏图标 |
| `src/locales/<lang>/trafficStats.json` ×14 | 各语言文案（ar de en es fa id jp ko pt ru tr tt zh zhtw） |

**修改文件（每个文件只加几行，按下列"挂点"找位置）：**

| 文件 | 挂点 |
|---|---|
| `src-tauri/src/lib.rs` | 模块声明区加 `pub mod traffic_stats;` |
| `src-tauri/src/core/mod.rs` | `pub mod hotspot;` |
| `src-tauri/src/cmd/mod.rs` | `pub mod {traffic_stats,hotspot};` + `pub use ...::*;` |
| `src-tauri/src/ipc.rs` | `collect_commands![...]` 追加流量统计与热点命令 |
| `src-tauri/src/core/notify/notification.rs` | `FrontendEvent::HotspotStatusChanged` + `frontend_events!` 注册 |
| `src-tauri/src/utils/resolve/mod.rs` | 初始化流量统计；`core::hotspot::start_auto_watch()` |
| `src-tauri/src/feat/window.rs` | `quit()` 停流量统计；清理前 `stop_hotspot_before_core()` |
| `src-tauri/src/feat/effects.rs` | 热点字段的 `verge_registry!` 项（无条件） |
| `src-tauri/src/config/verge.rs` | 5 个 `hotspot_*` 字段（无条件）+ `patch_opt_field!` |
| `src-tauri/Cargo.toml` + 根 `Cargo.toml` | `rusqlite`（bundled）、`windows`/`windows-sys` features |
| `src/pages/_navigation-meta.ts` / `_navigation.tsx` | 流量统计导航条目 |
| `src/locales/<lang>/layout.json` ×14 | `components.navigation.tabs` 加 `"trafficStats"` |
| `src/services/i18n.ts` | **`STARTUP_LANGUAGE_SECTIONS` 加 `'trafficStats'`** |

## 移植步骤（官方发新版后）

1. `git fetch origin --tags`，从新 tag 建 `feature/traffic-stats-v4` 工作分支。
2. 摘取提交（或对照上表手动落盘）：
   - 流量统计：`b8888df1 c218e0e0 ca5df3b4`
   - 热点：`36935549 2e836abb`（热点单测在 `mod.rs` 的 `#[cfg(test)] mod tests`）
   - 快捷解锁：`37cff43c`
3. 解决冲突：重点看 `ipc.rs` / `resolve/mod.rs` / `feat/*` / `config/verge.rs` /
   `Cargo.toml`——上游半年间的重构都在这些挂点附近。
4. **重新生成两类生成物**（忘了这步前端会缺类型/绑定）：
   - `pnpm i18n:types`（i18n 键类型）
   - `pnpm gen:contract`（IPC bindings，需要 cargo 能跑且 sidecar 占位存在）
5. 若上游新增语言目录，为它补 `trafficStats.json` + `layout.json` 标签 + `i18n.ts`。
6. 质量关卡（必须全绿）：
   ```
   pnpm typecheck && pnpm test && pnpm lint && pnpm arch:check && pnpm web:build
   cd src-tauri && cargo clippy-all && cargo test --lib
   ```
7. 构建：`pnpm install --frozen-lockfile && pnpm prebuild && pnpm build`
   （结尾 updater 签名报错是正常的，本地无签名私钥，安装包已生成）
8. 验证清单：
   - 侧边栏出现"流量统计"，页面为中文（若是键名说明 `i18n.ts` 漏了）
   - `curl -x http://127.0.0.1:7897 https://httpbin.org/bytes/51200` → DB 出现记录，方向正确
   - 日志：有 "Traffic stats manager started"
   - 重启应用，历史数据仍在
   - Windows 设置页出现"无线热点"，TUN 开启后可启动；退出应用后热点关闭

## Linux/CI 下跑 Rust 检查的前置

`cargo` 的 build script 需要 sidecar 占位文件（已在 `.gitignore`，不要提交）：

```bash
mkdir -p src-tauri/sidecar src-tauri/resources dist
touch dist/index.html
for bin in clash-verge-service clash-verge-service-install \
  clash-verge-service-uninstall verge-mihomo verge-mihomo-alpha; do
  touch "src-tauri/sidecar/${bin}-x86_64-unknown-linux-gnu"
done
```

## 已知陷阱（都是实际踩过的）

1. **Git Bash 的 tar 解不了 zip**——解压 node 用 `/c/Windows/system32/tar.exe`。
2. **pnpm install 时 node 版本不对 → rolldown 原生 optional 依赖漏装**，vite 构建报
   `Cannot find module '@rolldown/binding-win32-x64-msvc'`——删 node_modules 重装。
3. **cargo 拉 git 依赖走 libgit2 会 SSH 失败**——`net.git-fetch-with-cli = true`。
4. **`verge_registry!` 无 rest pattern**：`IVerge` 新增字段必须同步登记，否则编译失败。
5. **Rust `if let` 中的锁守卫跨 await** 会把调用链变非 Send，守卫先赋局部变量释放。
6. **`pnpm i18n:format` 会重排全仓语言文件**产生无关 diff，提交时只保留本功能相关文件。
7. **`STARTUP_LANGUAGE_SECTIONS` 是硬编码清单**，新 namespace 必须加入。
8. 中文路径会毁部分工具链，工作区保持 ASCII 路径。
9. **Windows 崩溃/强杀会残留热点**，需手动关闭；正常退出路径会清理。

## 与上游的关系

- 流量统计：上游 issue #8108 请求中，无同类功能。
- **2026-11-05：我们提交的 PR #8563 被维护者 Tunglies 静默关闭（未合并、无说明、CI 从未运行）。**
  用户已决定不再争取上游，纯自用路线。除非用户主动提出，以后的会话**不要**重开 PR、
  不要去上游留言。
- 自用更新方式：官方出新版 → 按本文档步骤移植到新 tag → 重新构建安装。
- 官方自动更新会覆盖本功能版：升级前先来这里重新移植，或在设置中关闭自动更新（建议关闭）。
