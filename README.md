# Pandex 🐼

以 Rust 为共享核心的跨平台密码管理器，由 pandas-coding 发起。

目标平台：Windows、Android，并探索 iOS 支持。桌面端与移动端的 UI 技术尚未确定，候选包括 Tauri 和 Kotlin / Compose Multiplatform。

当前处于项目初始化阶段：仅包含可构建的 Rust workspace 和客户端目录占位，尚未实现加密、存储或密码管理功能。

## 项目结构

```text
pandex/
├── crates/
│   └── pandex-core/       # 跨平台 Rust 核心
├── apps/
│   ├── desktop/           # 桌面客户端（技术待定）
│   └── mobile/            # 移动客户端（技术待定）
├── docs/
│   └── architecture.md    # 架构边界与待定事项
├── .github/workflows/     # Rust 持续集成
└── Cargo.toml             # Rust workspace
```

## 本地开发

安装支持 Rust 2024 edition 的稳定版 Rust，并确保 rustfmt 和 Clippy 可用：

```sh
rustup component add rustfmt clippy
cargo check --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 开发方向

- 定义密码库数据模型、威胁模型与密钥生命周期。
- 选择成熟的密码学库，设计加密存储与解锁流程。
- 确定客户端技术，并接入共享核心。
- 逐步完善自动锁定、密码生成、自动填充与同步能力。

参见 [架构说明](docs/architecture.md)。
