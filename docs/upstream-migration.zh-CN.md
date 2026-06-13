# 从上游 biopass 迁移

简体中文 | [English](upstream-migration.md)

biopass-rs 是上游 [biopass](https://github.com/TickLabVN/biopass) 的非官方 Rust 重写版本。它使用不同的二进制文件名称、PAM 模块名称和每用户存储路径，因此迁移有两个独立的部分：

1. 用户配置和已注册的生物识别数据。
2. 系统 PAM 配置。

本文主要说明 PAM 之外的迁移，也就是用户配置、用户数据、配置 schema 和安装后迁移行为。PAM 配置、上游 PAM 模块替换、`pam_fprintd` 冲突处理和发行版差异，请参阅 [PAM 设置指南](PAM.zh-CN.md)。

## 变化内容

| 项目 | 上游 biopass | biopass-rs |
| :--- | :--- | :--- |
| 用户配置 | `~/.config/com.ticklab.biopass/config.yaml` | `~/.config/biopass-rs/config.yaml` |
| 用户数据 | `~/.local/share/com.ticklab.biopass` | `~/.local/share/biopass-rs` |
| Helper 二进制文件 | 上游 helper | `/usr/bin/biopass-rs-helper` |
| PAM 模块 | 上游 PAM 模块 | `libbiopass_rs_pam.so`，具体配置见 [PAM 设置指南](PAM.zh-CN.md) |
| Debian PAM 配置文件 | 上游配置文件，通常是 `biopass` | `/usr/share/pam-configs/biopass-rs`，具体配置见 [PAM 设置指南](PAM.zh-CN.md) |

配置模式大部分兼容，但在此重写中，反欺骗部分被拆分为显式的 `ai` 和 `ir` 子配置。迁移代码将旧的反欺骗字段重写为当前模式。

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

如果存在上游 biopass，您有两个选择：
- **暂时共存**：在上游旁边安装 biopass-rs（推荐用于测试）
- **干净迁移**：先删除上游，然后安装 biopass-rs

### 迁移步骤

1. 安装 `biopass-rs`。

   包的安装后脚本会自动运行：

   ```bash
   /usr/bin/biopass-rs-helper install
   ```

   该命令刷新动态链接器缓存，将现有上游 biopass 用户配置迁移至 `biopass-rs`，并下载所需的 ONNX 模型。

2. 验证您的用户配置已经正确迁移。

   ```bash
   cat ~/.config/biopass-rs/config.yaml
   ```

3. 打开桌面应用并查看配置页面。

   确认面部相机、IR 相机、反欺骗设置、启用的方法、方法顺序、忽略的 PAM 服务和模型路径。

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

## 手动迁移

当您从仓库进行开发时、包的安装后脚本没有运行时，或者当您想要显式迁移一个用户时，使用此流程。

1. 仅当新配置不存在时才复制上游配置。

   ```bash
   mkdir -p ~/.config/biopass-rs
   cp ~/.config/com.ticklab.biopass/config.yaml ~/.config/biopass-rs/config.yaml
   ```

2. 移动或复制用户数据目录。

   ```bash
   mv ~/.local/share/com.ticklab.biopass ~/.local/share/biopass-rs
   ```

   如果您希望在测试期间保持上游安装正常工作，使用 `cp -a` 而不是 `mv`。

3. 迁移复制的配置模式。

   ```bash
   biopass-rs-helper migrate --username "$USER"
   ```

   从此仓库开发时，使用：

   ```bash
   mise run helper -- migrate --username "$USER"
   ```

4. 安装或验证 ONNX 模型。

   ```bash
   sudo /usr/bin/biopass-rs-helper install
   ```

   在开发中，按照 AI 模型页面中显示的模型设置或运行您在本地构建的 helper 二进制文件。

## `migrate` 做什么和不做什么

`biopass-rs-helper migrate --username <user>` 仅重写 `~/.config/biopass-rs/config.yaml` 的当前 biopass-rs 配置文件。

它不会将 `~/.config/com.ticklab.biopass/config.yaml` 复制到新位置。当新配置尚不存在时，包的 `install` 命令和桌面应用会执行该首次启动复制。

它不会将 `~/.local/share/com.ticklab.biopass` 移动到 `~/.local/share/biopass-rs`。当新数据目录不存在时，包的 `install` 命令会尝试为所有用户进行数据目录迁移。

它不会编辑 `/etc/pam.d/*`、运行 `pam-auth-update`、删除上游 PAM 配置文件或禁用 `pam_fprintd`。

它不会删除上游 biopass 包。

## 回滚

要回滚系统登录，禁用 `biopass-rs` PAM 配置文件或从受影响的 PAM 服务中删除 `libbiopass_rs_pam.so` 行，然后在需要时重新启用上游配置文件。

通过在手动迁移期间复制而不是移动，可以保留每用户的上游数据。如果包安装已经移动了数据目录并且您需要返回上游，将其移回：

```bash
mv ~/.local/share/biopass-rs ~/.local/share/com.ticklab.biopass
```

仅当上游版本支持您编写的模式时，才能将配置复制回去。如果您在迁移后在 biopass-rs 中编辑了设置，在将其与上游一起重用之前，请手动查看 YAML。
