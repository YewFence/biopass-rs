# 卸载与重装

简体中文 | [English](uninstall.md)

## TL;DR

卸载 biopass-rs 包**不会**删除你的用户数据——配置和已注册的人脸都保留在 `~/.config/biopass-rs/` 和 `~/.local/share/biopass-rs/`。想彻底清理就手动删这两个路径。重装时只要不动数据目录，已注册的人脸就不会丢。

## 数据存放在哪

biopass-rs 的所有用户级数据都在 home 目录下，它们**不属于包本身**，所以 `apt` / `dnf` / `pacman` 卸载时不会动它们：

| 内容 | 路径 |
| :--- | :--- |
| 配置文件 | `~/.config/biopass-rs/config.yaml` |
| 数据目录 | `~/.local/share/biopass-rs/` |

数据目录里包含：

| 子目录 | 说明 | 删除后果 |
| :--- | :--- | :--- |
| `faces/` | 你注册的人脸图片 | 需要重新注册人脸才能继续认证 |
| `models/` | ONNX 推理模型 | 重装或运行 `install` 时会自动重新下载 |
| `debugs/` | 调试模式下失败认证保存的诊断帧 | 无影响，可用 `biopass-rs-helper clean` 清理 |

## 卸载

### 1. 先在 PAM 里停用 biopass-rs

卸载包之前，先把 biopass-rs 从系统登录链路里摘掉，否则卸载后登录可能卡在缺失的 PAM 模块上。具体命令取决于你的发行版（在 `pam-auth-update` 里取消勾选 `Biopass` profile、对 `authselect` 做反向操作、或手动编辑 `/etc/pam.d/` 下当初改过的文件）。详见 [PAM 设置指南](PAM.zh-CN.md)——把那里"启用"的步骤反过来做即可。

### 2. 卸载包

**Debian/Ubuntu：**

```bash
sudo apt remove biopass-rs
sudo apt autoremove   # 顺带清掉不再需要的依赖
```

**Fedora/RHEL：**

```bash
sudo dnf remove biopass-rs
```

**Arch Linux：**

```bash
sudo pacman -R biopass-rs
```

这一步只删除 `/usr/bin/biopass-rs-helper`、PAM 模块 `libbiopass_rs_pam.so`、桌面应用等**包文件**，配置和数据目录都会保留。

### 3.（可选）彻底清理用户数据

如果你确定不再使用、想彻底清除痕迹：

```bash
rm -rf ~/.local/share/biopass-rs
rm -f  ~/.config/biopass-rs/config.yaml
# 配置目录现在空了，可以一并删掉：
rmdir ~/.config/biopass-rs 2>/dev/null || true
```

> **多用户机器**：每个注册过人脸的用户都要清理自己的目录。以 root 身份操作时，给 helper 传 `--username <用户名>`，或直接删对应用户 home 下的路径。

## 重装

- **想保留已注册的人脸**：直接重装即可，**不要**删数据目录。包的安装后脚本会运行 `biopass-rs-helper install`，重新下载缺失的模型、确保默认配置存在；你的 `faces/` 原封不动，重装后立刻可用。
- **想全新开始**：先按上面"彻底清理用户数据"删掉数据目录和配置，再重装。重装后需要重新捕获并注册人脸（`biopass-rs-helper capture-face` 或桌面应用里的注册流程）。

> 重装**不会**自动恢复 `faces/`。`install` 只在上游 biopass 数据目录存在时复制上游人脸（迁移场景），普通重装没有这个来源，所以删了 `faces/` 就只能重新注册。

## 另请参阅

- [PAM 设置](PAM.zh-CN.md) — 卸载前如何从系统登录链路摘掉 biopass-rs。
- [`biopass-rs-helper` CLI 参考](biopass-rs-helper.zh-CN.md) — `clean`、`install`、`config reset` 等命令。
- [从上游 biopass 迁移](upstream-migration.zh-CN.md)
