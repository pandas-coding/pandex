# Pandex 应用图标（Proton Pass 风格，2026-11-26 定稿白底渐变）

设计（对齐真机上 Proton Pass 现行图标语言：浅色底 + 渐变图形）：off-white
`#F7F6FA` 圆角方形 + 紫→暖橙渐变（`#6D4AFF → #F2A98A`，紫端即 `nsh.json`
主题主色）的几何「P」——竖笔为钥匙杆、实心圆点为钥匙头，钥匙头内镂空钥匙孔：
远看是 P（延续应用内品牌徽章），近看是钥匙（Proton Pass 的钥匙意象）。

备选变体（经典 Proton 家族：紫渐变底 `#8F6BFF → #5A31F4` + 白色字形）可由
`render.py purple` 复现；两变体预览见 `preview-white.png` / `preview-purple.png`。

## 文件

- `pandex.svg` — 主源文件（512 设计空间，几何与渐变轴同 `render.py` 保持同步）。
- `render.py` — 纯标准库渲染器（无第三方依赖）：超采样 + 面积加权降采样。
  重新生成全部栅格产物：`python3 assets/icon/render.py [white|purple]`（默认 white）。
- `pandex-512.png` — 全出血方形（Play Store 上架用，商店自行裁圆角）。
- `pandex.ico` — Windows 图标（16–256）。**NSH 0.6.1 打包不支持嵌入 exe
  图标**（nsh.json 无 icon 字段），暂存供安装器/后续 NSH 支持使用。
- `preview.png` — 明暗双背景联系表（legacy 各尺寸 + 自适应方/圆裁切）。

## Android 产物（由 render.py 生成，入库）

- `apps/android/app/src/main/res/mipmap-{mdpi..xxxhdpi}/ic_launcher.png`
  （48–192，圆角 legacy）。
- `mipmap-xxxhdpi/ic_launcher_background.png`（432 全出血渐变）与
  `ic_launcher_foreground.png`（432 透明底白色字形，放大 1.1× 后仍在
  66dp 安全区内）——自适应图标双层。
- `mipmap-anydpi-v26/ic_launcher{,_round}.xml` 引用上述双层；
  Manifest `android:icon`/`android:roundIcon` 指向 `@mipmap/ic_launcher{,_round}`。
