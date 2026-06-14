# 从上游 biopass 迁移

简体中文 | [English](upstream-migration.md)

## TL;DR

安装 `biopass-rs` 后，安装脚本会自动复制上游 biopass 的已注册人脸图片，但**不会**迁移配置。你需要在桌面应用里重新填写配置，并按照 [PAM 设置指南](PAM.zh-CN.md) 替换 PAM 模块。

## 简介

biopass-rs 是上游 [biopass](https://github.com/TickLabVN/biopass) 的非官方 Rust 重写版本。它使用不同的二进制文件名称、PAM 模块名称和每用户存储路径，因此迁移有两个独立的部分：

1. 用户配置和已注册的生物识别数据。
2. 系统 PAM 配置。

本文主要说明 PAM 之外的迁移，也就是用户配置、用户数据、配置 schema 和安装后迁移行为。PAM 配置、上游 PAM 模块替换、`pam_fprintd` 冲突处理和发行版差异，请参阅 [PAM 设置指南](PAM.zh-CN.md)。

## 变化内容

| 项目 | 上游 biopass | biopass-rs |
| :--- | :--- | :--- |
| 用户配置路径 | `~/.config/com.ticklab.biopass/config.yaml` | `~/.config/biopass-rs/config.yaml` |
| 用户数据路径 | `~/.local/share/com.ticklab.biopass` | `~/.local/share/biopass-rs` |
| Helper 二进制文件 | `biopass-helper` | `/usr/bin/biopass-rs-helper` |
| PAM 模块 | 上游 PAM 模块 | `libbiopass_rs_pam.so`，具体配置见 [PAM 设置指南](PAM.zh-CN.md) |
| Debian PAM 配置文件 | 上游配置文件，通常是 `biopass` | `/usr/share/pam-configs/biopass-rs`，具体配置见 [PAM 设置指南](PAM.zh-CN.md) |

配置 schema 已和上游有明显差异（例如反欺骗部分被拆分为显式的 `ai` 和 `ir` 子配置）。biopass-rs **不会**自动迁移上游配置——上游 schema 每个版本都在变，为每个版本维护一个转换器不可持续。安装时会写入全新的默认配置，并复制你已注册的**人脸图片**（与 schema 无关）；其余设置需要你在桌面应用里重新填写。

## 确认状态

验证上游 biopass 是否已安装并处于活动状态：

```bash
# 检查已安装的包
dpkg -l | grep biopass          # Debian/Ubuntu
rpm -qa | grep biopass          # Fedora/RHEL
pacman -Q | grep biopass        # Arch Linux

# 检查 PAM 配置中的上游 PAM 模块。
# 根据上游包版本，模块名可能是 pam_biopass.so 或 libbiopass_pam.so。
grep -r "pam_biopass\|libbiopass_pam" /etc/pam.d/
grep -r "biopass" /usr/share/pam-configs/  # 仅 Debian/Ubuntu
```

### 迁移步骤

1. 安装 `biopass-rs`。

   包的安装后脚本会自动运行：

   ```bash
   /usr/bin/biopass-rs-helper install
   ```

   该命令刷新动态链接器缓存，写入默认配置（**不会**导入上游配置），从上游数据目录复制已注册的人脸图片，并下载所需的 ONNX 模型。

2. 确认生成了预期的文件。

   ```bash
   ls ~/.config/biopass-rs/config.yaml
   ls ~/.local/share/biopass-rs/faces
   ```

3. 打开桌面应用并根据您的需求自行修改配置。

4. 配置 PAM。

   本文不展开 PAM 配置。请按照 [PAM 设置指南](PAM.zh-CN.md) 中对应发行版的“干净安装”或“从上游迁移”流程启用 `libbiopass_rs_pam.so`，并禁用上游 biopass PAM 模块。

5. （可选）在确认 biopass-rs 正常工作后删除上游 biopass 包：

   **Debian/Ubuntu：**
   ```bash
   sudo apt remove biopass
   sudo apt autoremove
   ```

   **Fedora/RHEL：**
   ```bash
   sudo dnf remove biopass
   ```

   **Arch Linux：**
   ```bash
   sudo pacman -R biopass
   ```

   删除后，验证没有上游引用残留：
   ```bash
   grep -r "pam_biopass\|libbiopass_pam" /etc/pam.d/
   ls /usr/lib/security/pam_biopass.so 2>/dev/null
   ls /usr/lib64/security/libbiopass_pam.so 2>/dev/null
   ```
