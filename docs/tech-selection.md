# 技术选型与设计决策

本文记录最终采纳的技术决策及其理由。历史讨论过程（v1 Tauri / v2 Compose Multiplatform / 各方案对比分析）已略去。

## 决策摘要

| 决策点 | 结论 | 备注 |
| --- | --- | --- |
| UI 框架 | **NSH（Native Shell，longbridge/nsh）**：平台原生 views + Rust 驱动 | Apache 2.0；0.x 版本，锁定精确版本，升级单独评审 |
| Rust ↔ UI | 同工作区 crate 直连（`pandex-app` 依赖 `pandex-core`），主应用零 FFI | 唯一 FFI 消费者是 autofill 进程 |
| Rust ↔ autofill 服务 | **UniFFI**（C-ABI 门面，Kotlin 独立进程调用） | 同一门面未来服务 Swift / ArkTS |
| Android 形态 | Gradle 宿主（`NshView` / nsh.aar）+ Kotlin autofill module | autofill 必须是 Kotlin Service，与 UI 框架无关 |
| Windows 壳 | 同一 `pandex-app`（NSH Win32/D2D 后端） | 无 JVM、无 WebView |
| Windows 自动填充 | 浏览器扩展（TS/React）+ Native Messaging | 首发 Chromium + Firefox |
| iOS/macOS（未来） | NSH iOS target + Swift Credential Provider 扩展 | 立项时验证 NSH iOS 成熟度 |
| 鸿蒙（未来） | ArkTS/ArkUI 壳 + Node-API 加载 core（.so） | core 层零返工；若升级为一等公民再重估 |
| 回退方案 | **Compose Multiplatform** | 触发条件见下文「M0 门槛」 |

## 选型理由

- **Android 是第一优先平台**，冷启动/内存/包体为硬指标。NSH 走平台原生 views 渲染路线，理论上优于自绘方案（Compose/Flutter/Impeller）与 WebView 方案（Tauri），且 IME/无障碍/系统主题走平台原生机制。
- **Rust 集成最紧**：主应用同进程直连零 FFI；UI↔core 本就是窄且机械的边界，真正的维护大头在平台原生模块（autofill/Keystore/生物识别必须 Kotlin/Swift），换任何框架都省不掉。
- **Windows 端零 JVM**：同一 NSH 应用直接打包，资源画像优于 Compose Desktop（JVM）。
- WSL 开发环境与纯 Rust 工具链（cargo + nsh CLI）高度契合，Gradle 只在 autofill 阶段引入。

已知代价与对策：

- NSH 开源时间短、0.x 版本（API 不稳定）：**运行时 TSX 动态加载对本项目为负资产，禁用（纯 Rust 模式）**；依赖锁定精确版本；升级单独评审。
- 成熟度风险由「状态下沉」（见 [architecture.md](architecture.md)）对冲：换 UI 框架时迁移面仅限薄 UI 层。

## M0 门槛（NSH 采纳验收）

3–5 天 spike，任一项不通过且无解 → 回退 Compose Multiplatform（文档与架构约束已为其保留）：

1. 安全文本输入（inputType 行为）
2. 窗口 FLAG_SECURE
3. Kotlin autofill module 与 NSH 宿主同 APK 共存
4. 中端真机冷启动/内存基线
5. Windows 构建链（`nsh package --target windows`）

## 关键设计约束

1. **进程边界即信任边界**：主应用解锁后，向 autofill 服务与浏览器扩展只提供有限条目快照（按域名过滤、可撤销），永不交出主密钥。
2. **UI 层不做加密决策**：锁定、超时、敏感内存清零全部在 core 内执行——未来可替换 UI 框架而安全逻辑零改动的前提。
3. **core 公共 API 按 FFI 可描述粒度设计**（plain struct、无生命周期泄漏、异步用回调/轮询）：Rust API 与 UniFFI 门面共用同一套类型，保证主进程与 autofill 进程行为一致。
4. **状态下沉（state-in-core）**：view-model 级逻辑全部在 core；app 层只做「状态快照 → 渲染」与「用户事件 → core 命令」。
