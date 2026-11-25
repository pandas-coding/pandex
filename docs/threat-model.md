# 威胁模型与密钥策略（P5 定稿）

本文定稿威胁模型、KDF 参数、密钥层级、锁定与敏感内存清零策略，关闭
[architecture.md](architecture.md) 待定事项中的威胁模型/密钥相关条目。
库文件格式见 P6，平台硬件密钥见 P8，同步范围见 P12（[roadmap.md](roadmap.md)）。

## 资产与攻击者模型

受保护资产：密码库条目（凭据）、主密码、库密钥 VEK。

防御的攻击者：

- **离线攻击者**拿到库文件 / 包裹的 VEK（P6 落盘后）：只见密文与 KDF 参数/salt，
  无法低成本离线暴力破解主密码（Argon2id 内存硬化）。
- **本地恶意应用 / 填充服务 / 浏览器扩展**：进程边界即信任边界，只拿按域名过滤、
  可撤销的条目快照，永不交出主密码或 VEK。
- **内存窥探 / 冷启动**：敏感密钥 material 置于 `Zeroizing` 缓冲，锁定即清零。
- **屏幕窥探 / 截屏**：窗口 `FLAG_SECURE` 全覆盖（M0 门槛 2）。

非目标（本阶段不防御）：运行时已被完全控制的设备（root / 调试器附加）、侧信道、
恶意 OS 内核。主密码强度由用户负责。

## 密钥层级

- 主密码 →(Argon2id)→ **KEK**（32 字节，仅存内存，`Zeroizing`）。
- **VEK**：随机 256-bit 库密钥，被 KEK 经 XChaCha20-Poly1305 包裹为 `WrappedVek`
  （KDF 参数 + salt + nonce + 密文）。
- 改主密码只重包裹 VEK，不重加密全库（P6 内容加密后受益）。
- 主密码与 KEK 永不落盘；磁盘上只有 `WrappedVek` 与密文内容（P6）。

## KDF 参数

- **Argon2id**（RFC 9106），OWASP 低内存档：`m = 19456 KiB (19 MiB)`、`t = 2`、`p = 1`，
  输出 32 字节。
- 参数与 salt 随 `WrappedVek` 存储，便于日后调参而不破坏旧库。
- 选型理由：纯 Rust、无硬件依赖、全平台一致；中端 Android 冷启动友好。
- 实测：本机（WSL，debug）单次派生约 0.3s 量级；低端机耗时待真机补测
  （结合 M0 门槛 4）。

## AEAD

- **XChaCha20-Poly1305**：24 字节随机 nonce（nonce-misuse 抵抗）、16 字节 tag、
  无硬件依赖、纯 Rust 全平台一致。
- VEK 包裹绑定域分隔 AAD `pandex-vek-wrap-v1`，防止跨上下文重放。
- tag 校验失败 → `InvalidPassword`（错误密码或篡改）。

## 锁定与清零

- 解锁态 core 持有 VEK（`Zeroizing<[u8; 32]>`）；`lock()` 显式 `zeroize` VEK 并丢弃
  解密内容。
- KEK 与解包裹中间明文均为 `Zeroizing`，离开作用域即清零。
- 锁定态快照 / 条目接口不暴露任何敏感数据。
- UI 锁定后清空密码输入框（薄壳已有行为）。
- 超时自动锁定在 P7；时间由调用方显式注入（FFI 可描述）。

## 信任边界

主进程持有 VEK；autofill / 浏览器扩展经 UniFFI（P9）只拿按域名过滤、可撤销的条目
快照，按需披露单条凭据；主密钥永不出主进程。

## P5 实现边界

- 解锁走真实 KDF + AEAD（VEK 包裹/解包裹）；库内容本阶段仍为内存演示条目，
  用 VEK 加密内容并持久化在 P6。
- core 公共 API 已改为 owned `String` / plain struct（FFI 前置）；core 不含演示密码。
  演示库由 app 薄壳以临时夹具密码在启动时创建，P6 首次运行流程将其替换。

## 后续阶段关闭的待定项

- 库文件格式 / 存储 / 备份迁移 → P6。
- 后台 / 超时锁定策略细节 → P7。
- 平台硬件密钥（Keystore / Windows Hello）与生物识别 → P8。
- 同步范围与冲突策略 → P12。
