# 流量统计功能：移植与维护指南

> 本文档面向未来的维护者（人或 AI 会话）：当 Clash Verge Rev 发布新版本、需要把流量统计功能
> 重新移植到新版时，按此操作。移植一次的全部工作量约 0.5~1 小时（不含编译）。
> 最后更新：2026-11-04，基于 v2.5.10（tag c83f7ad5）。

## 功能概览

后台每秒轮询 mihomo `/connections`，通过连接 ID diff 检测已关闭的连接，按连接的累计上传/下载
字节数生成记录，批量写入本地 SQLite（WAL 模式）。前端新增"流量统计"页面，按域名/进程聚合展示，
支持时间范围筛选与排序。

- 数据库：`%APPDATA%\io.github.clash-verge-rev.clash-verge-rev\traffic_stats.db`
- 容量策略：主库+WAL 合计 100MB 上限（超限删最旧 30% 循环），90 天滚动保留
- 固有精度边界：存活不足 1 秒的连接、连接关闭前最后约 1 秒的流量不计入（轮询原理所致，页面有脚注）

## 文件清单（移植 = 把这些改动重新落到新版代码上）

**新增文件（原样复制即可）：**

| 文件 | 说明 |
|---|---|
| `src-tauri/src/traffic_stats/{mod,types,monitor,batcher,store,manager}.rs` | 后端模块（6 个文件） |
| `src-tauri/src/cmd/traffic_stats.rs` | 两个 Tauri 命令（specta 化） |
| `src/pages/traffic-stats.tsx` | 前端页面 |
| `src/assets/image/itemicon/traffic-stats.svg` | 侧边栏图标 |
| `src/locales/<lang>/trafficStats.json` ×14 | 各语言文案（ar de en es fa id jp ko pt ru tr tt zh zhtw） |

**修改文件（每个文件只加几行，按下列"挂点"找位置）：**

| 文件 | 挂点 |
|---|---|
| `src-tauri/src/lib.rs` | 模块声明区加 `pub mod traffic_stats;` |
| `src-tauri/src/cmd/mod.rs` | `pub mod traffic_stats;` + `pub use traffic_stats::*;` |
| `src-tauri/src/ipc.rs` | `collect_commands![...]` 末尾追加 `cmd::query_traffic_stats, cmd::get_traffic_summary` |
| `src-tauri/src/utils/resolve/mod.rs` | `futures::join!(...)` 里加 `init_traffic_stats()`；同文件加该 async fn（仿 `init_timer`） |
| `src-tauri/src/feat/window.rs` | `quit()` 里 `set_is_exiting()` 之后加 `TrafficStatsManager::global().stop().await;` |
| `src-tauri/Cargo.toml` + 根 `Cargo.toml` | workspace.dependencies 加 `rusqlite = { version = "0.41", features = ["bundled"] }`，src-tauri 里 `workspace = true` 引用 |
| `src/pages/_navigation-meta.ts` | navigationItems 加 trafficStats 条目 |
| `src/pages/_navigation.tsx` | import 页面和图标，navItems 加条目（放 connections 后面） |
| `src/locales/<lang>/layout.json` ×14 | `components.navigation.tabs` 里加 `"trafficStats"` 标签 |
| `src/services/i18n.ts` | **`STARTUP_LANGUAGE_SECTIONS` 数组加 `'trafficStats'`**（漏掉这个页面会显示原始键名而非翻译，2026-11-04 踩过） |

## 移植步骤（官方发新版后）

1. `cd G:\clash-verge-rev && git fetch origin --tags && git checkout -B feature/traffic-stats-v3 v<新tag>`
2. 从 `feature/traffic-stats-v2` 摘取两个提交：`git cherry-pick f7f83bd4 a433a3f4`（或对照上表手动落盘）
3. 解决冲突：重点看 ipc.rs / resolve/mod.rs / _navigation* / Cargo.toml——上游半年间的重构都在这些挂点附近
4. **重新生成两类生成物**（忘了这步前端会缺类型/绑定）：
   - `pnpm i18n:types`（i18n 键类型）
   - `pnpm gen:contract`（IPC bindings，需要 cargo 能跑）
5. 若上游新增了语言目录，为它补 `trafficStats.json` + `layout.json` 标签 + `i18n.ts` 若有新机制
6. 质量关卡（必须全绿）：
   ```
   cargo fmt -p clash-verge && cargo clippy -p clash-verge   # 零警告（workspace lint 很严）
   pnpm typecheck && pnpm lint && pnpm i18n:check
   ```
7. 构建：`pnpm install --frozen-lockfile && pnpm prebuild && pnpm build`
   （结尾 updater 签名报错是正常的，本地无签名私钥，安装包已生成）
8. 安装：先备份 `F:\Clash\Clash Verge\clash-verge.exe`，taskkill 掉 clash-verge，
   `powershell Start-Process <安装包> -ArgumentList '/S' -Wait`，再启动
9. 验证清单：
   - 侧边栏出现"流量统计"，页面为中文（若是键名说明 i18n.ts 漏了）
   - `curl -x http://127.0.0.1:7897 https://httpbin.org/bytes/51200` 几次 → DB 里出现 httpbin.org/curl.exe 记录，方向正确（下载 >> 上传）
   - 日志（`%APPDATA%\io.github.clash-verge-rev.clash-verge-rev\logs\latest.log`）：有 "Traffic stats manager started"；启动内核未就绪窗口只有 1 条 warn + 1 条 recovered
   - 重启应用，历史数据仍在

## 构建环境（本机，全在 G 盘）

```bash
export PATH="/g/devtools/node-v26.13.0-win-x64:/g/devtools/npm-global:/g/rust/cargo/bin:$PATH"
export RUSTUP_HOME='G:\rust\rustup' CARGO_HOME='G:\rust\cargo'
export HTTPS_PROXY=http://127.0.0.1:7897 HTTP_PROXY=http://127.0.0.1:7897
```

版本要求看仓库根 `.tool-versions`（v2.5.10 时代是 node 26.13.0 / pnpm 12.16.1 / rust 1.99.1，
新版若更新需对应升级，node 解压到 `G:\devtools\node-v<版本>-win-x64`）。

## 已知陷阱（都是实际踩过的）

1. **Git Bash 的 tar 解不了 zip**——解压 node 用 `/c/Windows/system32/tar.exe`。
2. **pnpm install 时 node 版本不对 → rolldown 原生 optional 依赖漏装**，vite 构建报
   `Cannot find module '@rolldown/binding-win32-x64-msvc'`——删 node_modules 重装。
3. **cargo 拉 git 依赖走 libgit2 会 SSH 失败**——`G:\rust\cargo\config.toml` 已配
   `net.git-fetch-with-cli = true`，若重建 cargo 目录记得恢复。
4. **Rust `if let` 中的锁守卫跨 await** 会把 feat::quit 整条调用链变非 Send（exit_app/hotkey/tray
   连锁报错）。守卫必须先赋给局部变量释放（见 manager.rs stop() 的注释）。
5. **husky 钩子需要完整 PATH**（node+pnpm+cargo）；pre-push 的 check-contract-changes 传远端名
   会崩，可 `--no-verify`。
6. **`pnpm i18n:format` 会重排全仓 100+ 语言文件**产生无关 diff，提交时注意只保留本功能相关文件。
7. **`STARTUP_LANGUAGE_SECTIONS` 是硬编码清单**，新 namespace 必须加入，否则页面显示原始键名。
8. 中文路径会毁部分工具链，工作区保持 ASCII 路径（G:\clash-verge-rev）。

## 与上游的关系

- 上游至今无同类功能（issue #8108 请求中；曾有 PR #7022 未被合并）。
- **2026-11-05：我们提交的 PR #8563 被维护者 Tunglies 静默关闭（未合并、无说明、CI 从未运行）。**
  查证过这是该仓库对所有外部功能 PR 的常规操作（他人功能 PR 连提 3 次被关 3 次，连 AI 门禁
  判"合格"的也被关），不是代码问题。**用户已决定不再争取上游，纯自用路线。**
  除非用户主动提出，以后的会话**不要**重开 PR、不要去上游留言。
- 自用更新方式：官方出新版 → 按本文档步骤移植到新 tag → 重新构建安装。
- 官方自动更新会覆盖本功能版：升级前先来这里重新移植，或在设置中关闭自动更新（建议关闭）。
