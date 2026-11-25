# 架构

技术选型结论见 [tech-selection.md](tech-selection.md)，本文只描述落地后的边界。

## Rust 核心

`crates/pandex-core` 承载跨平台领域模型与业务逻辑：密码库条目、密码生成、加解密与存储接口、同步、以及全部 view-model 级状态机。不依赖 NSH、Gradle 或平台 SDK。

core 暴露两层 API：

- **Rust API**：供 `pandex-app`（NSH 应用）同进程直连，零 FFI。
- **UniFFI（C-ABI 门面）**：供 Android autofill 服务（Kotlin 独立进程）调用；未来同一门面思路服务 Swift 扩展。两层共用同一套类型，保证主进程与 autofill 进程行为一致。

公共 API 按"可被 FFI 描述"的粒度设计：plain struct、无生命周期泄漏、异步用回调或轮询描述。

**状态下沉**：解锁/锁定状态机、过滤/搜索、同步状态与冲突展示态全部在 core 内实现；`app/` 只做"状态快照 → NSH 渲染"与"用户事件 → core 命令"，保持薄壳（这也是 NSH 0.x 版本风险的主要保险：换 UI 框架时迁移面仅限此层）。

## UI 与客户端

- `app/`（pandex-app）：NSH 应用（Rust，平台原生 views 渲染）。桌面（Windows）直接以该 crate 构建打包（`nsh package --target windows`），并承载浏览器扩展的 Native Messaging 宿主；Windows Hello 解锁经 `windows` crate 在 Rust 侧调用，密钥保护用 DPAPI。
- `apps/android/`：Gradle 宿主工程。`NshView`（nsh.aar）承载主界面；生物识别用 `BiometricPrompt`，密钥放 Keystore（StrongBox 优先）；内含 Kotlin `autofill/` module（`AutofillService` + Credential Manager，独立进程，经 ContentProvider 取加密条目快照，解密在填充服务进程内经 UniFFI 调用 core 完成）。
- `extensions/browser/`：WebExtension（TS/React），负责桌面端浏览器内填充，首发 Chromium + Firefox。扩展不含密钥。
- 未来 iOS/macOS：NSH iOS target + Swift Credential Provider / Safari Web Extension（需届时验证 NSH iOS 成熟度）。

## 信任边界

主应用解锁后，向填充服务与浏览器扩展只提供有限条目快照（按域名过滤、可撤销），永不交出主密钥。UI 层不做加密决策：锁定、超时与敏感内存清零全部在 core 内执行。

## M0 门槛（NSH 采纳验收）

①安全文本输入（inputType 行为）②窗口 FLAG_SECURE ③Kotlin autofill module 与 NSH 宿主同 APK 共存 ④中端真机冷启动/内存基线 ⑤Windows 构建链。任一不通过且无解 → 回退 Compose Multiplatform（见 [tech-selection.md](tech-selection.md)）。

## 待定事项

- ~~威胁模型、主密码派生、密钥管理、锁定与敏感内存清理策略~~：已于 P5 定稿，
  见 [threat-model.md](threat-model.md)。
- 密码库文件格式、存储方式、备份与迁移机制。
- 本地优先与同步的范围、冲突解决策略。
- 各平台最低支持版本（Android API level、Windows 版本）、NSH 版本锁定与升级评审流程。
- 开源许可证与发布签名方案。
