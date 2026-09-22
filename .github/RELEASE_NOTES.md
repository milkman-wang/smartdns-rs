# SmartDNS-rs LuCI update

## 中文

### 更改与修复

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
- 已在新旧 QWRT/LuCI 上检查布局与验证按钮，并通过相关 Lua、JS 和打包回归检查。本次未修改 Rust DNS 核心。

## English

### Changes and fixes

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
- Layout and validation-button behavior were checked on legacy and modern QWRT/LuCI, with related Lua, JS, and packaging regression checks passing. This update does not change the Rust DNS core.
