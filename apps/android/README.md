# Pandex Android 宿主（M0 spike）

门槛 2/3 的最小 Gradle 宿主（见 [../../docs/m0-spike.md](../../docs/m0-spike.md)）：
`MainActivity` 设 `FLAG_SECURE` 后以 `NshView` 承载 Rust 应用；门槛 3 的 Kotlin
autofill module 将加在同一工程。NSH 0.6.1 无 FLAG_SECURE API 且 `AppActivity`
为 final，故宿主自建（architecture.md 规划的 `apps/android/` 形态）。

## 模块

- `:nsh` — NSH Android runtime 的 Java 侧，编译自 crate 源码（见下）。
- `:app` — 宿主 Activity + Rust 应用产物（`jniLibs/`、`assets/nsh/`）。

## Provenance（版本锁定 0.6.1，升级单独评审）

- `nsh/src/main/java/{com,org}`： vendored 自
  `nsh-platform-android-0.6.1` crate 的 `java/`（Apache-2.0，Longbridge）。
  NSH 官方 runtime dex 即由这些源码编译（javac + d8，无 Gradle）；dev/release
  差异在 native 侧（release `libnsh.so` 不含 dev support），Java 源码两变体通用。
- `app/src/main/jniLibs/arm64-v8a/libnsh.so` 与 `app/src/main/assets/nsh/`：
  取自 `nsh package --target android --abi arm64-v8a` 产物 APK（`.nsh/out/Pandex.apk`）
  的解包（Rust 应用 cdylib 改名 `libnsh.so`，静态链接引擎；`Native` 以
  `System.loadLibrary("nsh")` 加载）。**两者不入库**（gitignore）：锁定工具链下
  可由仓库源码完全复现，故按构建产物管理；新克隆后先执行下节刷新再构建。

## 刷新产物（Rust UI 变更后；新克隆后同样需要）

```sh
nsh package --target android --abi arm64-v8a          # 仓库根目录
unzip -o .nsh/out/Pandex.apk -d /tmp/pandex-apk
cp -r /tmp/pandex-apk/lib/arm64-v8a apps/android/app/src/main/jniLibs/
cp -r /tmp/pandex-apk/assets/nsh    apps/android/app/src/main/assets/
```

注意产物 APK 在 `.nsh/out/`（`nsh package` 输出）；`.nsh/rust/android/` 是
`nsh run` 的 dev 构建，勿混用（曾因此复制到过期产物）。

NSH 升级时同步重新 vendored `java/` 源码（crate 版本与 nsh CLI 一致）。

## 构建 / 安装

```sh
# 前置：已执行上节「刷新产物」（产物不入库）；环境见 ~/.bashrc.d/android.sh
cd apps/android && gradle :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk   # 真机需点允许；荣耀首次常自动拒绝，重试一次
adb shell am force-stop com.pandex.app                    # 否则 am start 只送达旧进程
adb shell am start -n com.pandex.app/.MainActivity
```

## FLAG_SECURE 全覆盖（门槛 2 修复）

Activity 自身的 `FLAG_SECURE` 不继承到 NSH 弹窗/PopupWindow 的独立窗口。
`MainActivity.getSystemService(WINDOW_SERVICE)` 返回 `SecureWindowManager`
包装层：经其 `addView`/`updateViewLayout` 的窗口一律加 `FLAG_SECURE`。
注意 `WindowManager` 接口 default 方法默认抛异常，须全委托；
`createLocalWindowManager` 等为 hidden API（反射也被拦），以纯 public API
实现（返回同 delegate 的新包装层）。真机验证：主窗口与弹窗窗口 `fl=` 均含
`SECURE`，最近任务缩略图空白。

## 与官方 AppActivity 的已知差异（spike 取舍）

- 不调 `useStandaloneLimits()`（package-private）：只影响 JS VM 限额，纯 Rust
  应用无 QuickJS，无实际影响。
- 不调 `ownSystemBars()`（package-private）：系统栏图标归宿主，本宿主已按
  昼夜模式自设（`systemBars`），spike 足够。
- `onConfigurationChanged` 只重设系统栏图标，不调 `mounter().applySystemBars()`
  （package-private）；spike 根组件未设 statusBar 样式。
