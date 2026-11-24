# M0 Spike 实施计划

M0 门槛定义见 [tech-selection.md](tech-selection.md)「M0 门槛」与 [architecture.md](architecture.md)。
本文是 spike 的实施计划：每项门槛的最小验证代码、真机验证步骤与通过判据。
任一门槛不通过且无解 → 回退 Compose Multiplatform。

## 进度（2026-11-24）

- [x] 门槛 1/2 的最小 UI：`src/lib.rs` 已从模板计数器替换为「主密码输入 + 登录列表」（一次性 spike 代码，真实密码库状态机与加解密后续在 pandex-core 实现）。
- [x] `nsh check` / `nsh test`（2 个 headless 用例）/ `cargo fmt` / `cargo clippy -D warnings` 通过。
- [x] `nsh component add android` 已安装 Android runtime（0.6.1）。
- [x] 环境准备（2026-11-24）：SDK cmdline-tools + platform-tools（adb 37.0.3）+ platforms;android-35 + build-tools;35.0.0 + NDK r27c（27.2.12479018）装于 `~/Android/Sdk`；JDK 17（mise 全局）；`aarch64-linux-android` rust target；环境变量在 `~/.bashrc.d/android.sh`（`ANDROID_HOME`/`ANDROID_SDK_ROOT`/`ANDROID_NDK_HOME` + PATH）。
- [x] 真机无线调试配对（2026-11-24）。
- [x] 门槛 1 真机验证：**通过**（证据与厂商怪癖见下）。
- [x] 门槛 2 真机验证：**通过**（含 Dialog 窗口泄露的发现与宿主侧修复，见下）。
- [x] 成果提交（2026-11-24）：spike UI/测试/Gradle 宿主/本文档入库；`libnsh.so` 与 `assets/nsh/` 为可复现构建产物，**不入库**（gitignore；刷新步骤见 `apps/android/README.md`，新克隆后先刷新再构建）。
- [x] 门槛 3 真机验证：**通过**（Kotlin 服务同 APK、独立进程及双字段填充，NSH 主界面正常）。
- [ ] 门槛 4–5。

## 调研结论（NSH 0.6.1，决定各门槛的验证路径）

1. **门槛 1 有原生 API 支持**。Rust `Input` 的属性在 Android 侧的映射（`nsh-platform-android` 的 `InputBox.java`）：
   - `.masked(true)` → `TYPE_TEXT_VARIATION_PASSWORD`（数字键盘时为 `TYPE_NUMBER_VARIATION_PASSWORD`）
   - `.auto_correct(false)` → `TYPE_TEXT_FLAG_NO_SUGGESTIONS`
   - `.auto_fill(AutoFill::Off)` → `IMPORTANT_FOR_AUTOFILL_NO`（不设置 autofillHints）
   - `.auto_capitalize(AutoCapitalize::None)` → 无大写 flag
2. **门槛 2 无现成 API，但有解**：NSH 0.6.1 全栈（nsh-core `WindowAttributes`、Android runtime Java 侧）均无 FLAG_SECURE / 防截屏能力；打包用的 `AppActivity` 是 `final`，不可继承。解法＝自建 Gradle 宿主 Activity，在 `onCreate` 里 `window.addFlags(FLAG_SECURE)` 后承载 `NshView`——这正是 architecture.md 已规划的 `apps/android/` 形态，不构成回退理由。
3. **Android 产物结构**：`nsh package --target android` 直接产出 APK（不导出 Gradle 工程）。Rust 应用编译为 cdylib 并改名 `libnsh.so`（静态链接引擎，`Native.java` 用 `System.loadLibrary("nsh")` 加载）；Java 侧 runtime 是 `classes.dex`。runtime 安装目录：`<nsh 安装目录>/runtime/android/{dev,release}/`。自建宿主需要：app 的 `libnsh.so` + `assets/nsh/`（取自打包 APK）+ NSH Java 类（编译 crate 内 `java/` 源码，或取官方 nsh.aar）。
4. **`nsh run --target android`** 用 dev host（`DevActivity`）+ 热重载，适合门槛 1 的快速真机迭代；门槛 2/3/4 需要自建宿主与 release 包。

## 环境准备（WSL，当前缺口）

现状：WSL 内无 adb / Android SDK / NDK / JDK；rustup 未装 android target。按 [dev-environment-wsl.md](dev-environment-wsl.md)：真机无线调试为主（NAT 下 `adb pair` 可用），模拟器不可硬件加速。

1. Android SDK cmdline-tools → `platform-tools`（adb）、`platforms;android-35`、`build-tools`、`ndk`（版本以 nsh 构建报错提示为准）。
2. `rustup target add aarch64-linux-android`（真机 ABI；nsh 默认还打 armeabi-v7a，可用 `--abi arm64-v8a` 只打真机需要的）。
3. JDK 17（Gradle 宿主阶段才需要；可用 mise 安装）。
4. 真机开启无线调试 → `adb pair <ip:port>` → `adb connect <ip:port>`；统一只用 WSL 侧 adb server。

参考命令（2026-11-24，用户自行执行）：

```sh
# SDK cmdline-tools（版本号以 https://developer.android.com/studio#command-line-tools-only 为准）
mkdir -p ~/Android/Sdk/cmdline-tools && cd /tmp
curl -fsSLO https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
unzip -q commandlinetools-linux-11076708_latest.zip -d ~/Android/Sdk/cmdline-tools
mv ~/Android/Sdk/cmdline-tools/cmdline-tools ~/Android/Sdk/cmdline-tools/latest

# 环境变量（写入 ~/.bashrc / ~/.profile）
export ANDROID_HOME="$HOME/Android/Sdk"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"

# 组件（NDK 先装 r27c：16 KB page 支持；nsh 构建若报版本要求再调整）
sdkmanager --licenses
sdkmanager "platform-tools" "platforms;android-35" "build-tools;35.0.0" "ndk;27.2.12479018"

# Rust android target
rustup target add aarch64-linux-android

# 真机（手机开发者选项 → 无线调试；pair 与 connect 端口不同）
adb pair <ip:配对端口>      # 输入手机上显示的配对码
adb connect <ip:连接端口>
adb devices                 # 期望看到 device 状态

# Gradle 宿主阶段再装 JDK 17（mise use -g java@17 或发行版 openjdk-17）
```

## 门槛 1：安全文本输入

**最小代码**（已完成，`src/lib.rs`）：主密码 `input()` 设 `masked(true)`、`auto_correct(false)`、`auto_fill(AutoFill::Off)`、`auto_capitalize(None)`、`return_key_type(Go)`；解锁后显示演示登录列表。

**真机步骤**：

```sh
nsh run --target android            # dev host 热重载；或 nsh package --target android --abi arm64-v8a --run
# 聚焦主密码输入框后：
adb shell dumpsys input_method | grep -iE "inputType|contentType"
adb shell dumpsys autofill | grep -i pandex
```

**通过判据**：
- 聚焦时 `inputType` 含 password variation 与 NO_SUGGESTIONS（文本键盘期望 `0x80081`）；
- 输入显示为掩码圆点，IME 无候选/学习行为，长按无系统 autofill 提议（`importantForAutofill=NO`）；
- 明文切换（未来「显示密码」）不在 M0 范围。

**真机结果（2026-11-24，BVL-AN16 / Android 17 / API 37 / SM8650）：通过**。

- `dumpsys input_method`：聚焦时 `inputType=0x280081`、`imeOptions=0x2`（GO）。分解：`TYPE_CLASS_TEXT|TYPE_TEXT_VARIATION_PASSWORD|TYPE_TEXT_FLAG_NO_SUGGESTIONS`（=0x80081，与 `InputBox.java` 设置一致）+ `0x200000`（`NO_PERSONALIZED_LEARNING`，隐藏常量；框架在 InputConnection 路径对密码字段自动添加，AssistStructure 中视图自身 mInputType 仍为 0x80081）——方向为安全加分。
- uiautomator：EditText `password="true"`；输入 "correct horse" 后截屏为 13 个掩码圆点。
- 荣耀 IME 在聚焦密码框时切换**安全键盘**（键盘窗口带 FLAG_SECURE）：期间 `screencap` 直接失败（"Capturing failed"），收起键盘即恢复。属平台/IME 侧加分行为，非 NSH 设置；副作用是验证时截屏需先收键盘。
- autofill：NSH 已设 `importantForAutofill=NO`（AssistStructure `important=2`）。**厂商怪癖**：荣耀框架 augmented autofill 仍把该字段纳入 fill context 并加 hint `passwordAuto`（NSH 源码无此字符串）；但系统服务（com.hihonor.keychain）两次响应均 `NUM_DATASETS=-1`，无填充 UI、解锁前后均无 save UI（`mShowingSaveUi=false`）。
- 真机闭环：Unlock 点击解锁 → Vault 列表渲染 → Lock 回锁定页且密码清空（headless 另覆盖 GO 回车提交）。

**残留风险（记录，不阻塞门槛 1）**：荣耀 augmented autofill 在 `NO` 之下仍把字段值送进系统 autofill 服务的 fill context。应用侧 per-view `NO` 已是最大信号，厂商覆盖不受应用控制。对策：门槛 3 落地后 Pandex 自身即 autofill 服务；并在其他 OEM/IME 上复测。若未来某 OEM 对主密码弹 save UI，需单独评估（候选：主密码框改用自绘输入、不接受系统 IME）。

## 门槛 2：窗口 FLAG_SECURE

**最小代码**：`apps/android/` 最小 Gradle 宿主（门槛 3 复用；已落地，细节与 provenance 见 `apps/android/README.md`）：

- `:nsh` 模块：编译 `nsh-platform-android-0.6.1` crate 的 `java/` 源码（仅依赖 SDK 自带的 org.json，无 androidx/kotlin；dev/release 差异在 native 侧，源码通用）；
- `:app` 模块：`MainActivity`（自有包 `com.pandex.app`）在 `onCreate` 中 `window.addFlags(FLAG_SECURE)`，然后 `NshView` + `runRustApp("nsh")`；`jniLibs/arm64-v8a/libnsh.so` 与 `assets/nsh/` 取自 `nsh package --target android --abi arm64-v8a` 的 APK 解包；manifest 属性对齐官方 APK（aapt2 解码：theme/configChanges/adjustResize/FilesProvider 等）；
- 只用 public 嵌入 API；`useStandaloneLimits`/`ownSystemBars`/`mounter()` 为 package-private，取舍与影响见 README；edge-to-edge/系统栏图标用 public Android API 复刻 DevActivity 逻辑；
- 宿主 Activity 同时是门槛 4 的测量对象。

**真机步骤**：

```sh
adb shell screencap -p /sdcard/m0.png && adb pull /sdcard/m0.png   # 期望全黑
# 系统录屏 / 最近任务缩略图同样检查
adb shell dumpsys window windows | grep -iE "pandex|secure"
```

**通过判据**：截屏、录屏、最近任务缩略图均不泄露内容。

**真机结果（2026-11-24，BVL-AN16 / 荣耀 Magic6 Pro / Android 17）：通过（含一处泄露的发现与修复）**。

- 主窗口：`dumpsys window windows` 的 `fl=` 含 `SECURE`；无键盘时 `screencap` 直接失败（"Capturing failed"）。
- **泄露发现**：NSH 系统弹窗（`SystemUi.dialog` → `AlertDialog.Builder`）是独立窗口，实测 `fl=DIM_BEHIND ALT_FOCUSABLE_IM …` **无 SECURE**——Activity 的 flag 不继承，与计划预判一致。
- **修复（有解，已验证）**：`MainActivity` 覆写 `getSystemService(WINDOW_SERVICE)` 返回 `SecureWindowManager` 包装层，对经其 `addView`/`updateViewLayout` 的所有窗口强制 `FLAG_SECURE`。Dialog/PopupWindow 的 WindowManager 取自 context.getSystemService，故被覆盖。修复后弹窗窗口 `fl=DIM_BEHIND SECURE …`。
  - 实现坑（已踩）：`WindowManager` 接口的 default 方法默认抛 `UnsupportedOperationException`，须全部委托；其中 `createLocalWindowManager`/`createPresentationWindowManager` 是 hidden API（SDK stub 无、反射被 hidden-API 拦截）——以纯 public API 实现：返回同一 delegate 的新包装层（Dialog 自身 decor 的 token 由 Dialog 设置，父窗口绑定只影响子窗口）。
- 最近任务：Pandex 卡片缩略图为空白占位（MagicOS 对 secure 快照渲染为白/灰），无内容泄露。
- 录屏未单独实测：与截屏同走 SurfaceFlinger secure 路径，截屏失败 + 窗口 flag 已覆盖判据；如需补证用 `adb shell screenrecord` 抽帧。

## 门槛 3：Kotlin autofill module 同 APK 共存

**最小代码**：`apps/android/autofill/` 是同一 Gradle 宿主的 Kotlin 模块，仅由 `:app` 的 debug 变体打包。服务声明 `android:process=":autofill"`、`BIND_AUTOFILL_SERVICE`、系统 intent-filter 和 `android.autofill` 元数据。只对当前 APK 的 `SpikeLoginActivity` 返回固定虚构凭据，不读取输入值、不提供保存、不接 UniFFI/core。release APK 不带测试服务或登录页。

**本地验证**（先按 [Android 宿主 README](../apps/android/README.md) 刷新 NSH 产物）：

```sh
cd apps/android
source ~/.bashrc.d/android.sh
gradle :app:assembleDebug :app:assembleRelease :autofill:lintDebug
python3 test_m0_autofill_apk.py -v
```

**真机步骤**：

1. 安装 `app/build/outputs/apk/debug/app-debug.apk`，在手机上允许安装。记录原 Autofill 服务，在系统设置中选 `Pandex Autofill (M0)`（可搜索「自动填充」）。
2. 打开桌面上的 `Pandex M0 Test Login`，或执行 `adb shell am start -n com.pandex.app/com.pandex.autofill.SpikeLoginActivity`。先点 `Check demo values` 确认空白表单显示 `FAIL`，再聚焦用户名并点系统建议 `Pandex M0 demo login`。不要手工输入测试值；再次点 `Check demo values`，应显示 `PASS: both demo fields match`，密码仍是掩码。
3. 用 `adb logcat -d -s PandexM0Autofill:I` 确认服务返回数据集；用 `adb shell pidof com.pandex.app:autofill` 与 `adb shell pidof com.pandex.app` 核对独立 PID。保持测试服务启用，打开主入口 `adb shell am start -n com.pandex.app/.MainActivity`，确认 NSH 锁定页和演示解锁、回锁正常，主密码不出现此测试建议。
4. 验收后在系统设置恢复原 Autofill 服务。

**通过判据**：单 APK 安装无冲突；服务可绑定、可填充；主 UI 不受影响。

**真机结果（2026-11-24，荣耀 BVL-AN16 / Android 17）：通过。**

- 同一 debug APK 首次安装返回 `INSTALL_FAILED_ABORTED: User rejected permissions`；用户在手机上确认后重试成功。系统设置已选中 `com.pandex.app/com.pandex.autofill.SpikeAutofillService`。
- 空白表单检查为 `FAIL`；用户选择系统建议 `Pandex M0 demo login` 后，未手工输入即显示 `PASS: both demo fields match`。`pidof` 显示宿主 PID `26536`、`:autofill` PID `26621`。本次事后未抓到服务的 logcat 行，以实际填充和独立 PID 为证。
- 保持 Pandex 为默认服务，`MainActivity` 正常启动；用户确认主密码框未出现测试建议，`Unlock → Vault → Lock` 往返成功。
- debug/release 构建、`:autofill:lintDebug` 和两项 APK 冒烟测试通过。`:app:lintDebug` 仍报未改动的 `MainActivity` 的 `NewApi` / `MissingPermission` 问题，不属于本次变更。

## 门槛 4：中端真机冷启动 / 内存基线

**最小代码**：无新增；用门槛 2 的宿主 release 包（non-debuggable）。

**设备注意**：当前配对真机 BVL-AN16 为旗舰（SM8650 / 16GB RAM），**不是中端机**——门槛 1–3 可用它验证，门槛 4 的「中端基线」需另借中端真机；旗舰数据只作上界参考。

**真机步骤**（中端真机，重复 5 次取均值）：

```sh
adb shell am force-stop com.pandex.app
adb shell am start -W -n com.pandex.app/.MainActivity   # 记录 TotalTime（冷启动）
adb shell dumpsys meminfo com.pandex.app                # 稳定后记录 TOTAL PSS
# 记录 APK 体积（arm64 单 ABI）
```

**通过判据（暂定阈值，实测前确认）**：冷启动 ≤ 800ms；演示列表态 PSS ≤ 150MB。超标先归因（NSH runtime vs spike 代码）再下结论；数据记入本文档。

## 门槛 5：Windows 构建链

**最小代码**：`.github/workflows/` 增加 `windows-latest` job：安装 nsh CLI → `nsh package --target windows` → 上传产物。WSL 本地不构建 Windows（见 dev-environment-wsl.md）。

**通过判据**：CI 产出可运行的 `<name>.exe` + bundle（zip）；在 Windows 侧冒烟运行。

## 真机操作怪癖（荣耀 Magic6 Pro / MagicOS，门槛 1–2 踩坑记录）

- 安装：`adb install` 首次常被自动拒绝（`INSTALL_FAILED_ABORTED: User rejected permissions`）；用户看着手机重试一次即成功。勿把首次失败当结论。
- 重启：`install -r` 后旧进程可能仍存活，`am start` 只把 intent 送达旧实例；先 `adb shell am force-stop com.pandex.app` 再 start。
- 无线调试：连接端口会随重连变化；会话开始先 `adb devices` 确认，必要时请用户在手机上重看端口并 `adb connect`（配对关系保留，无需重新 pair）。
- 熄屏：手机自动锁屏熄屏会丢输入事件；验证期间由用户手动保持亮屏。
- 截屏：安全键盘或任何 FLAG_SECURE 窗口可见时 `screencap` 直接失败；先 `keyevent 4` 收键盘。失败本身亦可作 secure 证据（门槛 2）。

## 执行顺序

环境准备 → 门槛 1 真机 → 门槛 2/3 Gradle 宿主（一次搭建，两项复用）→ 门槛 4 测量 → 门槛 5 CI。
每项结论（通过 / 不通过 + 证据）回写本文档；触发回退时按 tech-selection.md 保留的架构约束迁移。
