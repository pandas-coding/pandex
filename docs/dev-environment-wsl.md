# WSL 开发环境（NSH + Rust）

结论：主 UI 是纯 Rust 工具链（cargo + nsh CLI），Gradle 只在 `apps/android` 宿主工程（autofill module 阶段）才引入。

## 当前环境实测（2026-11）

- WSLg 已启用（DISPLAY/Wayland 均在）→ NSH 桌面运行（`nsh run`）与 GUI 工具可用；
- 内存 31GB → 充裕；
- 网络为 NAT 模式 → Windows 侧模拟器 `adb connect localhost` 不通：要么在 `.wslconfig` 补 `networkingMode=mirrored`（保留现有 `vmIdleTimeout` 配置），要么主用真机无线调试（NAT 下出站访问局域网正常，`adb pair` 可用）；
- `.wslconfig` 含 dxgkrnl BSOD 缓解（WSL2 vGPU 曾致宿主蓝屏）→ 若 WSLg GUI 不稳定，这是背景原因；主用真机调试可绕开。

## 工具链清单（全部 WSL 内）

```text
rustup（stable ≥1.97，满足 NSH MSRV）
nsh CLI（curl -fsSL https://nsh.dev/install.sh | sh）
rust targets：aarch64-linux-android 等（Android 打包，经 nsh CLI 驱动）
Android 侧：adb（真机 wireless debugging 为主）+ platform-tools
后期（apps/android 阶段）：JDK + Android Studio（WSLg）+ Gradle —— 仅 autofill module 需要
```

## 痛点与解法

1. **Android 模拟器无法在 WSL 内硬件加速**（无嵌套虚拟化）：首选真机无线调试；备选 Windows 侧模拟器 + 镜像网络 `adb connect localhost:5555`；usbipd-win 可用但繁琐。
2. **文件系统纪律**：项目必须留在 WSL ext4（当前 `~/github/repos/pandex` 满足）；放 `/mnt/c` 会使构建慢 5–10 倍。
3. **NSH 桌面（Linux target）需要 GTK4 系统库**：`app/Cargo.toml` 中 `nsh-platform-linux` 已设为 opt-in（`desktop-linux` feature），CI 与日常构建不需要 GTK；需要桌面运行时安装 `libgtk-4-dev` 等并启用该 feature。**注意：WSL 里跑的是 Linux 桌面版，Windows 打包交 CI（`windows-latest`）。**

## 已知的坑

- 两侧 adb server 冲突：统一只用一个 adb server（推荐 WSL 侧），Windows 侧模拟器用 TCP connect 接入。
- nsh CLI 安装的是独立二进制；CI 中按官方文档安装或缓存。
