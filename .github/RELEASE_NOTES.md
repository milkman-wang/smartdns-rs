# SmartDNS-rs IPv4 preference and LuCI update

## 中文

### 更改与修复

- 新增可选配置 `dualstack-ip-prefer-ipv4 yes`，默认关闭。启用后优先使用可用 IPv4；IPv4 测速失败、查询失败或无 A 记录时仍保留 IPv6，A 查询不受影响。
- 尊重原有 DNS 分组和测速设置。禁用测速的代理分组依据有效 A 记录判断，不额外对代理地址执行直连测速。
- IPv4 优先产生的 AAAA 空响应按对应 A 记录和配置的 TTL 上限缓存，避免 `local-ttl 1` 导致频繁重复查询和测速。
- 参照 C SmartDNS 重整 Lua 与 JS 界面的设置分组、上游编辑页和域名/IP 规则列表，减少重复表单。
- 普通单行输入框与下拉框统一宽度，恢复主题原生标签对齐；改善行间距、手机布局和运行状态栏。
- 修复配置编辑器过大的行高与宽度收缩，以及 Lua 文件上传显示“文件无法访问”的问题。
- Lua compat 包内置简体中文，无需另装 compat 中文语言包；兼容新旧 LuCI 的翻译加载接口。
- 配置验证明确区分成功和失败，修复空白橙色提示条，并显示失败详情。

### 安装与验证

- Headless 与 WebUI 为独立版本；按需求选择对应核心包。
- Lua 界面安装 `luci-app-smartdns-rs-compat`；新版 LuCI 使用 Lua 界面时还需固件提供 `luci-compat`。
- JS 界面安装 `luci-app-smartdns-rs`，中文界面另装 `luci-i18n-smartdns-rs-zh-cn`。Lua 与 JS 界面包互斥，切换前先卸载原界面包，保留核心及配置。
- opkg 固件使用 IPK，apk 固件使用 APK，不能混用。OpenWrt IPK 面向 24.10.7，APK 面向当前快照；LuCI 包与架构无关，核心包必须匹配架构。
- IPv4 优先可在自定义配置中启用；本次未新增 LuCI 开关。它是 DNS 层面的优先策略，不会关闭 IPv6 接口，也不能代替应用连接健康检查。
- IPv4 优先已通过功能回归和 AP8220 实机验证，IPv6-only 域名仍可解析。固定负载短测完成 25,000 次查询且无错误或超时；开启策略有额外查询、测速和缓存开销，不代表零成本或长期内存测试。
- 已在新旧 QWRT/LuCI 上检查布局与验证按钮，并通过相关 Lua、JS 和打包回归检查。

## English

### Changes and fixes

- Add optional `dualstack-ip-prefer-ipv4 yes`, disabled by default. When enabled, prefer usable IPv4 while retaining IPv6 if the IPv4 probe fails, the A lookup fails, or no A record exists. A queries remain unchanged.
- Respect existing DNS groups and probe settings. Proxy groups with probing disabled use valid A records without additional direct-path probes of proxy destinations.
- Cache empty AAAA responses using the supporting A record's TTL and configured TTL caps, avoiding repeated queries and probes when `local-ttl` is set to 1.
- Reorganize Lua and JS settings, upstream editors, and domain/IP rule tables around the C SmartDNS layout, reducing repeated forms.
- Give ordinary single-line inputs and dropdowns a consistent width and restore theme-native label alignment; improve spacing, mobile layouts, and the service status panel.
- Fix excessive configuration-editor line height, shrinking editor widths, and the Lua upload control incorrectly reporting an inaccessible file.
- Bundle Simplified Chinese in the Lua compat package and support translation loading on both legacy and modern LuCI.
- Distinguish successful and failed configuration checks, replacing the empty orange warning with a clear result and error details.

### Installation and validation

- Headless and WebUI are independent variants; choose the corresponding core package.
- Install `luci-app-smartdns-rs-compat` for the Lua interface. Modern LuCI also needs the firmware's `luci-compat` support.
- Install `luci-app-smartdns-rs` for the JS interface and `luci-i18n-smartdns-rs-zh-cn` for Chinese. The Lua and JS interface packages conflict; remove the previous interface package before switching, keeping the core and configuration.
- Use IPK on opkg firmware and APK on apk firmware; the formats are not interchangeable. OpenWrt IPK targets 24.10.7, while APK targets current snapshots. LuCI packages are architecture-independent; core packages must match the device architecture.
- Enable IPv4 preference in custom configuration; this update does not add a LuCI toggle. This DNS-level preference neither disables IPv6 interfaces nor replaces application connection health checks.
- Functional regressions and AP8220 device checks cover IPv4 preference and IPv6-only fallback. A short fixed-load test completed 25,000 queries without errors or timeouts. The policy adds query, probe, and cache overhead; this is not a zero-cost claim or a long-term memory test.
- Layout and validation-button behavior were checked on legacy and modern QWRT/LuCI, with related Lua, JS, and packaging regression checks passing.
