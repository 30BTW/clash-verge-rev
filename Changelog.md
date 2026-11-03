<!-- Release-notes skeleton. Entries accumulate in ./Changelog.md as PRs land;
     before publishing a release, distill them:
     - user-visible changes only — no dev tooling, CI fixes, or refactors whose
       effect is already covered by another entry
     - describe outcomes, not internals: no internal module names, IPC or
       protocol details, or implementation mechanics — write what the user
       observes, not how the code achieves it
     - one line per theme: merge entries that share a root cause
     - each entry fits a single readable line (about 60 CJK characters); if it
       overflows, it is packing multiple themes or detail that belongs in the
       linked issue/PR — split it or drop the detail
     - line grammar: open with a verb (修复/新增/优化), with the platform
       first (Windows/macOS/Linux) when the change is platform-specific;
       merge sub-symptoms as 主题：症状、症状; full-width punctuation,
       「」 for UI labels, backticks for config keys and commands
     - platform entries sit under their platform heading (bold, emoji
       prefixed, ordered Windows/macOS/Linux, combined for pairs like
       macOS/Linux) at the end of their section;
       scripts-workflow/group_platforms.sh regroups, --check verifies
     HTML comments are stripped by the update dialog's sanitizer and hidden on
     GitHub, so they never reach users. -->
## v2.5.10

<details>
<summary><strong> 🐞 修复问题 </strong></summary>

- 修复全局模式缺少 `GLOBAL` 策略组，无法单独选择全局出口
- 修复服务模式下，SSH 节点的 `private-key` 使用私钥文件时内核无法启动
- 修复服务模式下，内核更新的 GEO 数据库不会同步回应用目录，导致使用新分类的规则无法通过校验
- 修复订阅的 DNS 配置变化后，「DNS 覆写」实际已关闭但开关仍显示开启且没有提示

**🖥️ Windows**

- Windows：修复安装程序重新安装或覆盖时未提示退出运行中应用，导致卸载报错或覆盖失败

</details>

<details>
<summary><strong> ✨ 新增功能 </strong></summary>

- 新增代理组测速遵循当前筛选条件，仅测试匹配节点
- 新增可从连接列表快速添加域名规则
- 新增混合代理端口自动避让，被占用时临时使用其他端口
- 新增系统代理和 TUN 权限引导，可直接安装服务并恢复原设置
- 新增服务模式多用户隔离，支持旧版服务升级迁移
- 新增手动取消节点固定功能
- 新增代理组标题栏拖动调整，可交换组名和工具栏的位置
- 新增前置规则与后置规则互相移动功能
- 新增 DNS 覆写保护：订阅含自定义 DNS 时先确认，订阅更新后自动关闭覆写
- 新增代理页排查提示，方便检查订阅和内核状态
- 新增订阅更新频率提醒，间隔不足 24 小时时提示
- 新增启动失败提示，显示原因并保存启动日志
- 新增 TUN「Mips」堆栈选项，需配合 mihomo v1.19.31 及以上内核

- 新增虚拟网卡设置的「DNS 模式」选项，支持配置 mihomo 系统 DNS 行为
- 新增「IP 信息」自动刷新开关，关闭后仍可手动刷新
- 新增 Muse AI 解锁检测，针对 Muse AI 特有的 Cloudflare 人机验证增加「Risky」状态标记
- 新增 Windows「无线热点」：将 TUN 出口共享为 Wi-Fi 热点，可配置名称、密码、频段与发射网卡

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>


</details>
