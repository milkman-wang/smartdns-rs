# SmartDNS-rs interface binding fix

## 中文

### 本次更新

- 修复 Linux 下 `@设备名` 将通配监听地址替换为单一接口地址的问题。Passwall 生成的 `bind [::]:15354@lo` 现在可同时接收 IPv4 和 IPv6 本机 DNS 请求。
- 保留显式配置的监听地址，通过套接字的接口绑定机制限制入口；不再擅自替换为该接口的第一个地址。
- 增加监听配置及 UDP/TCP 双栈收发回归测试。

### 升级说明

- Headless 和 WebUI 核心均包含此修复；选择匹配设备架构和包管理器的版本。
- 升级并确认本机 DNS 正常后，可移除为此问题临时补充的 IPv4 回环监听，继续使用 Passwall 原有的 `[::]:端口@lo` 配置。

## English

### Changes in this update

- Fix `@device` replacing wildcard bind addresses with a single interface address on Linux. Passwall's `bind [::]:15354@lo` now accepts both IPv4 and IPv6 loopback DNS requests.
- Preserve explicitly configured bind addresses and restrict the interface through the socket binding mechanism instead of selecting the interface's first address.
- Add configuration and bidirectional UDP/TCP dual-stack regression tests.

### Upgrade notes

- Both Headless and WebUI cores include this fix. Select the variant matching the device architecture and package manager.
- After upgrading and verifying local DNS, remove temporary IPv4 loopback listeners added for this issue and retain Passwall's original `[::]:port@lo` configuration.
