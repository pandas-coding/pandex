# Pandex 实施路线图（M1 之后）

前置状态：M0 门槛 1–3、5 通过（门槛 4 跳过）；M1 第 1–4 阶段完成
（状态下沉 `pandex-core` + NSH 薄壳 + Android/Windows 构建链回归，见 [m1.md](m1.md)）。
本文把 M1 遗留的候选任务清单展开为可执行阶段 **P5–P12**，供新会话按序接续。

架构约束（全程有效，见 [architecture.md](architecture.md) / [tech-selection.md](tech-selection.md)）：

- 全部安全逻辑（加解密、密钥、锁定、超时、清零）在 `pandex-core`；NSH app 只做快照渲染与事件转发。
- core 公共 API 按 FFI 可描述粒度设计：plain struct、owned 数据、无生命周期泄漏。
- 进程边界即信任边界：autofill / 浏览器扩展只拿按域名过滤、可撤销的条目快照，永不交出主密钥。
- NSH 锁定 0.6.1；NSH 应用保留仓库根目录；不重排 M0 门槛 4。
- 每阶段结束跑构建链回归（步骤见 [m1.md](m1.md) 第 4 阶段与 [apps/android/README.md](../apps/android/README.md)），结论回写本文档对应小节。

## 候选任务 → 阶段映射

| 候选任务 | 阶段 |
| --- | --- |
| 主密码密钥派生、真正加解密、密钥管理（软件层） | P5 |
| 持久化（含数据模型与首次运行流程） | P6 |
| 超时自动锁定 | P7 |
| 生物识别、密钥管理（平台硬件层） | P8 |
| UniFFI | P9 |
| Android Autofill 接入 | P10 |
| 浏览器扩展接入 | P11 |
| 同步 | P12 |

依赖关系：P5 → P6 → P7；P8 依赖 P5；P9 依赖 P5/P6 的 API 形态；P10 依赖 P8+P9；
P11 依赖 P6（+P7）；P12 依赖 P6 与威胁模型定稿。里程碑分组：**M2 = P5–P7**（可用的本地密码库）、
**M3 = P8–P10**（Android 真实自动填充）、**M4 = P11**（桌面浏览器填充）、**M5 = P12**（同步）。

---

## P5 加密基础：密钥派生 + 加解密 + 软件层密钥管理

**范围**：仅 `pandex-core`（纯 Rust，headless 可测），不动 UI 行为以外的平台代码。
开始前先写 `docs/threat-model.md` 定稿：威胁模型、KDF 参数、密钥层级、锁定与清零策略
（architecture.md 待定事项中威胁模型/密钥相关条目在此关闭；库文件格式见 P6，同步范围见 P12）。

**技术方案**：

- 密钥层级：主密码 →(Argon2id)→ KEK；随机 256-bit VEK（库密钥）被 KEK 包裹存储。
  改主密码只重包裹 VEK，不重加密全库。
- crate：`argon2`（RustCrypto，Argon2id）、`chacha20poly1305`（XChaCha20-Poly1305，
  nonce misuse 抵抗、无硬件依赖、纯 Rust 全平台一致）、`rand`/`getrandom`、`zeroize`。
- KDF 参数取 OWASP 低内存档（m=19456 KiB, t=2, p=1）起步，中端 Android 冷启动友好；
  参数与 salt 随包裹头存储，便于日后调参。
- `Core::unlock` 不再比较明文：派生 KEK → 解包裹 VEK，AEAD tag 校验失败即 `InvalidPassword`。
  解锁态 core 持有 VEK；`lock()` 用 `zeroize` 清零 VEK 与中间密钥。
- **API 改造（FFI 前置）**：`Snapshot`/`EntrySummary` 的 `&'static str` 全部改 owned `String`；
  演示常量与 `DEMO_PASSWORD` 移除或仅留 `#[cfg(test)]` 测试夹具。
- 本阶段库内容仍可为内存演示条目（持久化在 P6），但解锁必须走真实 KDF+AEAD。

**验收**：

1. `cargo test --workspace`：新增 KDF/AEAD 单元测试（正确密码解锁、错误密码拒绝、
   lock 后密钥清零、known-answer 向量）；原 6 测试适配后全绿。
2. `nsh test` 2 用例适配新演示密码流程后通过；UI 行为不变（Unlock/Lock/掩码）。
3. `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --check` 干净。
4. 构建链回归通过；结论回写本节。

**风险**：Argon2id 在低端机的耗时需实测（P5 只做参数记录，中端机测量可结合 M0 门槛 4 补测）。

### P5 验收记录（2026-11-26）

威胁模型定稿见 [threat-model.md](threat-model.md)（architecture.md 对应待定项已关闭）。

实现：`pandex-core` 新增 `crypto.rs`（Argon2id KEK 派生 + XChaCha20-Poly1305 VEK
包裹/解包裹，域分隔 AAD `pandex-vek-wrap-v1`）；`Core::create_vault` 生成随机 VEK 并
包裹，`unlock` 走真实 KDF+AEAD（tag 失败 → `InvalidPassword`），`lock` 用 `zeroize`
清零 VEK 与中间密钥。`Snapshot`/`EntrySummary` 改 owned `String`，新增 `KdfParams`/
`WrappedVek` plain struct（FFI 前置）；core 移除演示密码。依赖：argon2 0.6、
chacha20poly1305 0.11、zeroize 1.9、getrandom 0.4。P5 库内容仍为内存演示条目，
演示库由 app 薄壳以临时夹具密码启动时创建（P6 首次运行流程替换）。

- 本地基线（WSL）：`cargo fmt --check` ✓、`cargo clippy --workspace --all-targets
  --locked -D warnings` ✓、`cargo test --workspace --locked` ✓（core 16 passed，含
  KDF/AEAD known-answer 向量、错误密码拒绝、篡改检测、lock 清零）；`nsh test` ✓
  （2 用例，解锁已走真实 KDF+AEAD，UI 行为不变）。
- Android 构建链（WSL 本地）：`nsh package --target android --abi arm64-v8a` ✓
  （libnsh.so 交叉编译 1m35s）→ 刷新 jniLibs/assets ✓ → `gradle :app:assembleDebug
  :app:assembleRelease :autofill:lintDebug` ✓（`Unable to strip libnsh.so` 为 M0 已知
  现象）→ `test_m0_autofill_apk.py` ✓ 2/2。产物 sanity：libnsh.so 含
  `pandex-vek-wrap-v1`/argon2/`Crypto` 错误变体，旧 `correct-password` 已消失。
  未做真机验收（无 adb 设备）。
- Windows 构建链（GitHub Actions）：提交 `ddfcfcb` 推送触发 run
  [45661630331](https://github.com/pandas-coding/pandex/actions/runs/45661630331)：
  `check` ✓ 3m48s（fmt/clippy/test）；`windows-package` ✓ 21m29s，exe 冒烟通过，
  artifact `pandex-windows` 35,045,574 bytes（ID 15046402805）。Cargo.lock 新增
  加密依赖使缓存 key 变化、crate 重编译，故耗时高于缓存命中基线。唯一 annotation
  为 actions Node.js 20 弃用警告（后续维护事项）。

**结论**：P5 验收 1–4 全部通过。解锁已走真实 Argon2id + XChaCha20-Poly1305，
锁定清零密钥；API owned 化（FFI 前置）；threat-model 定稿。下一步 = P6。

## P6 数据模型 + 持久化 + 首次运行

**范围**：core 数据模型与存储、app 层首次运行/条目 UI 最小扩展、平台存储路径注入。

**技术方案**：

- 条目模型：`Entry { id, title, username, password, urls, notes, created_at, updated_at }`，
  全部 owned、可 serde；`password` 字段在快照 API 中默认不暴露（列表只给 summary，
  取明文走单独命令，为 P10/P11 的按需披露打基础）。
- 库文件格式（版本化）：magic + version + KDF 参数/salt + 包裹的 VEK + nonce +
  XChaCha20-Poly1305 密文（内层 serde_json 序列化的条目集）。单文件、本地优先。
- 原子写：临时文件 + rename；core 定义存储接口（路径由调用方注入），不依赖平台 SDK。
  Android 路径由 Gradle 宿主 `filesDir` 提供，Windows 由 app 层取 `%APPDATA%`。
- 首次运行：创建库（设置主密码 → 生成 VEK → 落盘）替代演示解锁；
  UI 增加「创建密码库」分支与最小条目增/改/删。
- 锁定态永不读写明文；磁盘上只有密文。

**验收**：

1. core 测试：roundtrip、篡改检测（改密文任一字节 → 解锁失败）、原子写、
   首次运行/重开库流程、锁定态不落明文。
2. `nsh test` 覆盖创建库 → 解锁 → 列表 → 锁定往返；真机/桌面手测重启后数据仍在。
3. 构建链回归 + Android 真机安装冒烟（创建库、杀进程重开、解锁）。

### P6 验收记录（2026-11-26）

实现：

- core 新增 `model.rs`（`Entry`/`EntrySummary`/`EntryDraft`，owned + serde，`Entry`
  derive `Zeroize`）、`storage.rs`（`VaultStore` 接口 + `FileStore` 原子写 +
  `MemoryStore`）、`format.rs`（版本化库文件 `PANDEX` v1，头部绑定为内容 AAD）；
  `crypto.rs` 增内容加解密。`Core` 改为注入存储：`new(store)` 加载密文头、
  `create_vault` 首跑落盘并解锁、`unlock` 解包裹 VEK + 解密内容、`lock` 清零 VEK 与
  条目、`entries` 只给 summary、`entry(id)` 按需披露、`add/update/delete_entry`
  每次重加密原子落盘。`UnlockError` 统一为 `Error`（增 Storage/Corrupt/VaultExists/
  NoVault/Locked/NotFound）。依赖增 serde/serde_json、zeroize derive 特性。
- app 薄壳移除演示密码夹具：`data_dir()` 注入 `FileStore`（无数据目录回退
  `MemoryStore`，headless 测试隔离），三分支 UI（创建库 / 解锁 / 库列表）+ 最小条目
  增改删表单。**额外（用户要求，超出 P6 最小 UI 范围）**：参考 Proton Pass 的视觉
  刷新——`nsh.json` `"theme"` Proton 紫双主题（light/dark 跟随系统），`App()` 内
  `set_tokens` 统一各入口；卡片式条目 + 字母头像 + 品牌徽章 + 圆角输入 + 全宽主按钮。
  修复刘海安全区 bug：`p_safe_area()` 的 env padding 曾被固定 `px/py` 覆盖，改为
  嵌套两层 view 使安全区与内容 padding 相加。

验证：

- 本地基线（WSL）：`cargo fmt --check` ✓、`cargo clippy --workspace --all-targets
  --locked -D warnings` ✓、`cargo test --workspace --locked` ✓（core 37 passed：
  roundtrip、内容/头部篡改检测、原子写、首跑/重开、锁定态不落明文、CRUD、按需披露、
  KAT 向量保留）；`nsh test` ✓ 5 用例（创建→添加→锁定/解锁往返、回车提交、确认不符
  拒绝、错误密码拒绝、编辑+删除）。
- Android 构建链（WSL 本地）：`nsh package --target android --abi arm64-v8a` ✓ →
  刷新 jniLibs/assets ✓ → `gradle :app:assembleDebug :app:assembleRelease
  :autofill:lintDebug` ✓（`Unable to strip libnsh.so` 为 M0 已知现象）→
  `test_m0_autofill_apk.py` ✓ 2/2。产物 sanity：libnsh.so 含
  `pandex-vault-content-v1`/`pandex-vek-wrap-v1`/`Entry` 字段与 Proton 主题 cssVars，
  演示密码 `correct horse` 已消失。
- Android 真机（BVL-AN16，WiFi adb）：安装 → `pm clear` 首跑 → 创建库
  （`vault.pandex` 158B、权限 0600）→ 添加条目（333B，磁盘无 GitHub/octocat/明文
  密码泄漏）→ `am force-stop` 冷重开进解锁屏 → 解锁后条目恢复。安全区实测：Vault
  头部 top=279px、Add/Lock top=295px，均避开 141px 刘海（修复前 Add/Lock 在 112px
  压刘海）。Proton 配色 headless 实测：light 卡片 #FFFFFF、dark 卡片 #232135。
- Windows 构建链（GitHub Actions）：run
  [45841459112](https://github.com/pandas-coding/pandex/actions/runs/45841459112)
  （提交 `22a73bc`）：`check` ✓ 1m24s（fmt/clippy/test）；`windows-package` ✓ 15m52s，
  exe 冒烟通过，artifact `pandex-windows` 33,292,214 bytes（ID 15130555128）。
  期间发现 install.ps1 默认装 latest 已升到 nsh 0.7.1、拒绝 0.6.1 依赖致
  windows-package 失败（P6 首推 run 45839781558 的 `check` ✓ 但 windows-package ✗）；
  修复把 CI 的 NSH CLI 钉到 `v0.6.1`（`$env:NSH_VERSION`），重跑装得 nsh 0.6.1
  (2f51d86) 与本地一致后转绿。NSH 仍锁定 0.6.1，未升级。

**结论**：P6 验收 1–3 全部通过。库内容真实加密持久化（PANDEX v1，头部绑定 AAD）、
首跑创建库替换演示夹具、锁定清零、按需披露；Proton 风格 UI 与刘海安全区修复真机
验证通过；构建链回归（Android 本地 + 真机、Windows CI）全绿。下一步 = P7。

## P7 超时自动锁定 + 会话加固

**范围**：core 超时状态机 + UI/宿主事件转发。

**技术方案**：

- core 维护 `last_activity` 与可配置超时；**时间由调用方显式传入**（FFI 可描述，
  不依赖 core 内系统时钟），UI 定时器/平台生命周期事件驱动 `tick(now)` 与 `record_activity(now)`。
- Android 宿主把 `onPause/onResume` 转发为事件（后台立即锁或短超时，策略写入 threat-model）；
  Windows 侧会话锁/最小化事件同理（NSH 窗口事件能力若不足，记录缺口再定方案）。
- 锁定路径复用 P5 的清零逻辑；UI 锁定后清空输入框（已有行为保持）。

**验收**：core 测试用注入时间戳覆盖超时/活动重置/后台策略；`nsh test` 模拟 tick；
真机验证退后台超时回锁。构建链回归。

### P7 验收记录（2026-11-26）

实现：

- core：`LockPolicy { idle_timeout_secs, background_grace_secs }`（默认 300/0；
  idle 0 表示关闭空闲超时）。`record_activity(now)` / `tick(now) -> bool` /
  `on_background(now)` / `on_foreground(now)`，unix 秒一律调用方注入；解锁/创建后
  首个 tick 播种活动时间线（久置时钟不会立即锁掉刚解锁的库）；`on_foreground`
  复查后台宽限（挂起期间定时器可能不运行）；自动锁定复用 `lock()` 清零路径，
  `lock()` 同时清活动/后台时间戳。
- app 薄壳：`set_interval(1s)` 驱动 tick；`effect` 订阅 `lifecycle()`
  （Background → `on_background`、Active → `on_foreground`、Inactive 不另设策略）；
  用户事件（按钮、表单键入）`record_activity`；锁定后 `clear_secrets` 清空主密码
  与条目表单全部明文信号（手动与自动同路径）。
- **宿主零改动**：NSH 0.6.1 的 NshView 内部注册 `ActivityLifecycleCallbacks`
  （onStop → Background）；Windows presence 层把会话锁（Win+L）、息屏、会话断开、
  虚拟桌面 cloak、最小化/隐藏映射为 Background——roadmap 预设的「NSH 窗口事件
  能力缺口」不存在，该项关闭。
- threat-model.md 增「超时与后台锁定策略（P7 定稿）」并关闭对应待定项。
- 顺带（用户要求）：Proton Pass 风格应用图标定稿——off-white 底 + 紫→暖橙渐变
  P-key（竖笔钥匙杆、圆头钥匙头镂空钥匙孔），光学居中下移 16px；`assets/icon/`
  主 SVG + 纯标准库双风格渲染器 + 512/ICO/预览；Android mipmap legacy 五档 +
  自适应双层 + anydpi-v26 XML + Manifest icon/roundIcon，真机桌面验证。缺口记录：
  NSH 0.6.1 打包不支持嵌入 exe 图标，`pandex.ico` 暂存供安装器/后续 NSH 支持。

验证：

- 本地基线（WSL）：`cargo fmt --check` ✓、`cargo clippy --workspace --all-targets
  --locked -D warnings` ✓、`cargo test --workspace --locked` ✓（core 49 passed：
  新增 12 个注入时间戳状态机测试——超时边界/活动重置/首 tick 播种/可配置策略/
  后台宽限/回前台复查/锁定态 no-op）；`nsh test` ✓ 8 用例（新增 3：空闲自动锁
  清空表单、活动重置计时、后台立即锁且回前台保持锁定；手动时钟
  `open(".", { clock: {} })` + `clock.fastForward` / `setLifecycle`）。
- Android 构建链（WSL 本地）：`nsh package --target android --abi arm64-v8a` ✓ →
  刷新 jniLibs/assets ✓ → `gradle :app:assembleDebug :app:assembleRelease
  :autofill:lintDebug` ✓ → `test_m0_autofill_apk.py` ✓ 2/2。
- Android 真机（BVL-AN16，WiFi adb）：首跑创建 → 添加条目 → `am force-stop`
  冷重开解锁恢复 ✓；Home 退后台 3s 回前台**立即回锁**（列表清空）✓；310s 无交互
  自动回锁 ✓（空闲 tick 与息屏后台策略共同作用，空闲 tick 单独由 headless 注入
  时钟证明）；磁盘 `vault.pandex` 338B、权限 0600、`PANDEX` magic、无
  GitHub/octocat/明文密码/主密码泄漏 ✓。空闲测量期间息屏超时临时调 600s，
  测毕恢复 300000。
- Windows 构建链（GitHub Actions）：run
  [45858167728](https://github.com/pandas-coding/pandex/actions/runs/45858167728)
  （提交 `20b2c23`）：`check` ✓ 2m29s（fmt/clippy/test）；`windows-package` ✓ 3m21s
  （缓存命中），exe 冒烟通过，artifact `pandex-windows` 33,311,375 bytes
  （ID 15135954793）。唯一 annotation 为已知的 actions Node.js 20 弃用警告
  （维护事项）。

**结论**：P7 验收 1–4 全部通过（core 注入时间戳、nsh 模拟 tick、真机退后台回锁、
构建链回归 Android 本地 + 真机 + Windows CI）。下一步 = P8。

## P8 生物识别 + 平台密钥管理

**范围**：Android Keystore/BiometricPrompt（Kotlin 宿主模块）、Windows Hello（Rust `windows` crate）；
core 增加「平台包裹的 VEK」接口（core 不直接调平台 SDK，密钥材料经宿主注入）。

**技术方案**：

- Android：Keystore（StrongBox 优先）生成 AES-GCM 密钥，`setUserAuthenticationRequired(true)`；
  `BiometricPrompt` 成功后解包裹 VEK 副本供解锁。生物识别只是解锁便捷路径，
  主密码仍是最终恢复手段；重启后首次解锁必须主密码（Keystore 认证绑定策略按此设计）。
- Windows：Windows Hello（`UserConsentVerifier`/`KeyCredentialManager`）验证后取 DPAPI
  保护的 VEK；无 Hello 设备回退纯主密码。
- core 侧：`unlock_with_platform_key(wrapped_vek)` 类命令 + 能力查询快照字段；
  UI 增加「生物识别解锁」按钮（能力可用时显示）。

**验收**：core 测试（平台密钥路径的状态机与清零）；Android 真机指纹/面部解锁往返、
重启后强制主密码；Windows Hello 在 CI 不可测，记录为手测项。构建链回归。

## P9 UniFFI C-ABI 门面

**范围**：新 crate `crates/pandex-ffi`（门面独立成 crate，core 保持 `#![forbid(unsafe_code)]`），
Kotlin 绑定生成与 Android 构建链接入。

**技术方案**：

- `uniffi` crate：以 P5/P6 定型后的 core API 为蓝本定义门面（plain struct、owned String、
  无生命周期）；生成 Kotlin 绑定，编入 `:autofill` 模块可用的独立 `libpandex_ffi.so`
  （与 NSH 的 `libnsh.so` 分离，autofill 进程不加载 UI 引擎）。
- 构建链：cargo ndk 目标产物 → Gradle `jniLibs`（沿用 M0 产物刷新模式，产物不入库）；
  绑定生成步骤写入 `apps/android/README.md`。
- 门面首批 API：解锁/锁定、按域名查询条目快照、取单条凭据（按需披露）、能力查询。

**验收**：Kotlin 侧 instrumented 冒烟（`:autofill` 进程内加载 .so、调用门面、返回演示数据）；
`cargo test` 门面与 core 行为一致性测试；构建链回归（双 .so 共存、包体记录）。

## P10 Android Autofill 真实接入（替换 spike）

**范围**：`:autofill` 模块从固定虚构凭据升级为真实服务；主进程 ContentProvider；
release 签名（顺带关闭 M1 遗留的 keystore 事项）。

**技术方案**（对齐 architecture.md 既定设计）：

- 主进程 ContentProvider 只暴露**加密的、按域名过滤的**条目快照，授权可撤销；
  autofill 进程经 UniFFI 调 core 解密（VEK 来自 P8 的 Keystore 生物识别路径），
  主密钥永不出主进程。
- `AutofillService` 真实实现：域名匹配（含 eTLD+1 归并）、填充数据集、保存流程
  （新凭据回写主进程库）；M0 的 `SpikeLoginActivity`/测试服务移除或仅留 debug 变体。
- 信任边界测试：未解锁/未授权时 autofill 拿不到明文；撤销授权立即失效。

**验收**：真机端到端（第三方 App/浏览器表单填充、保存新凭据、独立进程 PID 核对）；
`test_m0_autofill_apk.py` 升级为真实服务的 APK 结构测试；release 签名包构建；构建链回归。

## P11 浏览器扩展 + Native Messaging（Windows 优先）

**范围**：`extensions/browser/`（WebExtension，TS/React，首发 Chromium + Firefox）；
NSH Windows app 内 Native Messaging 宿主。

**技术方案**：

- 扩展不含密钥：经 Native Messaging 向主应用请求按域名过滤的快照与单条凭据，
  主应用解锁态 + 用户确认才披露；会话可撤销（对齐 P10 信任边界）。
- Native Messaging 宿主注册（Windows 注册表 manifest）由打包/安装步骤处理；
  宿主逻辑在 app 层转发 core 命令，安全决策仍在 core。
- 扩展 UI 最小化： popup 列表 + 填充按钮 + 锁定态提示。

**验收**：Chromium/Firefox 手测填充与保存；未解锁时扩展只读锁定态；
Windows CI 打包链含宿主注册产物；构建链回归。

## P12 同步（本地优先，端到端加密）

**范围**：core 同步状态机 + 服务端（选型另议）+ 冲突展示态。

**技术方案（立项时先细化，本文只定边界）**：

- 端到端加密：服务器只见密文；同步单位是加密库 blob 或加密条目记录（立项时定）。
- 冲突策略起步：条目级 last-write-wins + tombstone；冲突展示态在 core，UI 只渲染。
- 服务端候选：自托管 Rust（axum）最小存储 API；账号/认证模型与威胁模型一并评审。
- 同步状态（进度/错误/冲突）全部进 `Snapshot`，app 薄壳不变。

**验收**：core 同步状态机测试（离线/冲突/重试）；双设备端到端手测；
服务端部署文档；构建链回归。

---

## 新会话执行指引

1. 读本文档 + [m1.md](m1.md) + 项目记忆，确认当前所处阶段（各阶段验收后在本文打勾并记录证据）。
2. 从当前阶段的「范围」开工，不提前实现后续阶段内容；跨阶段依赖未就绪时先补依赖。
3. 每阶段固定收尾：`cargo fmt/clippy -D warnings/test --workspace` + `nsh test` +
   构建链回归（Android 本地、Windows 走 CI）+ 结论回写本文档 + 更新项目记忆。
4. 提交/推送前征得用户同意；纯文档改动也会触发 CI，属预期。

## 进度

- [x] P5 加密基础（含 threat-model 定稿）
- [x] P6 数据模型 + 持久化（含 Proton 风格 UI 刷新与刘海安全区修复）
- [x] P7 超时自动锁定（含会话加固与 Proton Pass 风格应用图标）
- [ ] P8 生物识别 + 平台密钥管理
- [ ] P9 UniFFI 门面
- [ ] P10 Android Autofill 真实接入
- [ ] P11 浏览器扩展 + Native Messaging
- [ ] P12 同步

维护事项（可随任意阶段顺带）：CI actions 升级消除 Node.js 20 弃用警告；
release 签名 keystore（P10 前必须）；NSH 升级评审（保持 0.6.1 锁定，除非单独评审）。
已处理：Windows CI 的 NSH CLI 安装钉到 `v0.6.1`（`$env:NSH_VERSION`）——install.ps1
默认装 latest，0.7.1 发布后会拒绝 0.6.1 依赖导致 windows-package 失败（P6 期间发现）。
