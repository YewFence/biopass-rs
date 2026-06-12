# Biopass-rs - 上游 Biopass 的非官方 Rust 重写版本

简体中文 | [English](README.md)

<p align="center">
    <img src="https://public-r2.ticklab.site/media/tc1oN21KXhMM1B2jOecRhk=" alt="Biopass Logo" width="120" />
</p>

<p align="center">
    <a href="https://github.com/YewFence/biopass-rs/releases/latest">
        <img src="https://img.shields.io/github/v/release/YewFence/biopass-rs?label=Last%20Release&style=flat-square" alt="Latest release" />
    </a>
    <a href="https://github.com/YewFence/biopass-rs/stargazers">
        <img src="https://img.shields.io/github/stars/YewFence/biopass-rs?style=flat-square" alt="GitHub stars" />
    </a>
    <a href="https://github.com/YewFence/biopass-rs/issues">
        <img src="https://img.shields.io/github/issues/YewFence/biopass-rs?style=flat-square" alt="Open Issues" />
    </a>
</p>

<h2 align="center">Biopass-rs</h2>
<p align="center"><b><a href="https://github.com/TickLabVN/biopass">Biopass</a> 的非官方 Rust 重写版本</b></p>
<p align="center">面向 Linux 桌面的快速、安全、注重隐私的生物识别模块，支持面部和指纹识别。</p>

> **注意**：这是由 [@phucvinh57](https://github.com/phucvinh57) 和 [@thaitran24](https://github.com/thaitran24) 在 TickLab 开发的原始 [Biopass](https://github.com/TickLabVN/biopass) 项目的个人非官方 Rust 重写版本。原始的 C++ 实现已完全替换为 Rust，项目以尽力而为的方式维护。对于官方版本，请访问[上游仓库](https://github.com/TickLabVN/biopass)。

---

## 为什么选择 Biopass-rs？

[Biopass](https://github.com/TickLabVN/biopass) 由 TickLab 开发，提供了一个快速、安全、现代化的 Linux 生物识别套件，不仅仅局限于面部识别。这个 Rust 重写版本是我对该项目的个人尝试——将 C++ 实现转换为更加安全且清晰的 Rust。

## 与上游版本的对比

| 功能 | [Biopass](https://github.com/TickLabVN/biopass) | [Biopass-rs](https://github.com/YewFence/biopass-rs) |
| :--- | :--- | :--- |
| **AI 模型安装** | Shell 脚本 | 原生 Rust 代码 |
| **反欺骗配置结构** | 扁平数组，`ai` 和 `ir` 开关状态不明确 | 重构为独立的 `ai` 和 `ir` 模块，配置更清晰 |
| **反欺骗重试** | 功能被显式删除[#94](https://github.com/TickLabVN/biopass/pull/94) | AI 和 IR 反欺骗检查支持独立的重试配置 |
| **相机处理** | 无 | 添加了图像自动优化选项 |
| **IR 相机捕获帧质量检测** | 优化中，详情见 [#116](https://github.com/TickLabVN/biopass/issues/116) | 自动跳过暗帧 |
| **图片处理路径** | GUI 使用浏览器 API 处理图像，认证时的 PAM 模块使用 OpenCV，[#114](https://github.com/TickLabVN/biopass/issues/114) | GUI 和 PAM 模块都使用 Rust 的 jpeg crate 进行图像处理，确保一致的图片质量 |
| **`helper` CLI** | `auth` 和 `crop-face` 命令 | 新增子命令：`migrate`、`install`、`capture-face`、`preview-session` 和 `completion`；`auth` 子命令的 `--username` 会自动从环境变量查找 |

## 安装

- 请参考[上游 Biopass 发布页](https://github.com/TickLabVN/biopass/releases)获取预构建的 Debian 和 RPM 包，或参考 [AUR 包](https://aur.archlinux.org/packages/biopass-bin)用于基于 Arch 的发行版。
- 系统登录设置在可用时使用发行版管理的 PAM 配置（例如 Debian/Ubuntu 上的 `pam-auth-update`）：[docs/PAM.zh-CN.md](docs/PAM.zh-CN.md)
- 从上游 Biopass 迁移需要同时进行每用户配置/数据迁移和 PAM 审查，以确保上游和 Rust 重写的 PAM 模块不会同时为同一服务启用：[docs/upstream-migration.zh-CN.md](docs/upstream-migration.zh-CN.md)
- 交互式 `polkit` 认证设置：[docs/Polkit.zh-CN.md](docs/Polkit.zh-CN.md)
- [IR 相机设置指南](docs/IR%20camera.zh-CN.md)
- [`biopass-rs-helper` CLI 参考](docs/biopass-rs-helper.zh-CN.md) — 认证、面部捕获、模型安装和 shell 补全。

## 特性

- [x] 认证：用户可以注册多个生物识别信息进行认证。认证方法可以并行或顺序执行。
    - [x] 面部：
      - [x] 识别
      - [x] 反欺骗
        - [x] 使用 AI 模型
          - [x] 可配置的重试
        - [x] 使用 IR 相机
          - [x] 可配置的重试
    - [x] 指纹

欢迎通过提出 issue 来请求新功能或报告错误。关于贡献，请阅读 [CONTRIBUTING.zh-CN.md](docs/contributing.zh-CN.md)。

## 参考

本项目使用的模型（来源于上游项目）：
- 面部识别：**[EdgeFace](https://github.com/otroshi/edgeface)**
- 面部检测：**[YOLO-Face](https://github.com/akanametov/yolo-face)**

## 致谢

本项目是 [TickLabVN/biopass](https://github.com/TickLabVN/biopass) 的非官方 Rust 重写版本。

- **原始设计和架构**：TickLab 的 [@phucvinh57](https://github.com/phucvinh57) 和 [@thaitran24](https://github.com/thaitran24)
- **AI 模型**：EdgeFace 和 YOLO-Face，与上游项目相同
- **C++ → Rust 转写**：以尽力而为的方式维护；更新可能会滞后于上游项目

特别感谢 TickLab 团队创建 Biopass 并将其作为开源项目发布。没有他们的原创工作，就不会有这个重写版本。

如果您觉得 Biopass 有用，请考虑首先支持[上游项目](https://github.com/TickLabVN/biopass)。
