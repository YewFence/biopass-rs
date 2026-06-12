# 从上游 Biopass 迁移

简体中文 | [English](upstream-migration.md)

本项目是上游 [TickLabVN/biopass](https://github.com/TickLabVN/biopass) 项目的非官方 Rust 重写版本。它使用不同的二进制文件名称、PAM 模块名称和每用户存储路径，因此迁移有两个独立的部分：

1. 用户配置和已注册的生物识别数据。
2. 系统 PAM 配置。

在更改 PAM 时保持 root shell 打开。在关闭 root shell 之前在第二个终端中测试。

## 变化内容

| 项目 | 上游 Biopass | biopass-rs |
| :--- | :--- | :--- |
| 用户配置 | `~/.config/com.ticklab.biopass/config.yaml` | `~/.config/biopass-rs/config.yaml` |
| 用户数据 | `~/.local/share/com.ticklab.biopass` | `~/.local/share/biopass-rs` |
| Helper 二进制文件 | 上游 helper | `/usr/bin/biopass-rs-helper` |
| PAM 模块 | 上游 PAM 模块 | `libbiopass_rs_pam.so` |
| Debian PAM 配置文件 | 上游配置文件，通常是 `biopass` | `/usr/share/pam-configs/biopass-rs` |

配置模式大部分兼容，但在此重写中，反欺骗部分被拆分为显式的 `ai` 和 `ir` 子配置。迁移代码将旧的反欺骗字段重写为当前模式。

## 推荐的包迁移

**开始之前**，验证上游 Biopass 是否已安装并处于活动状态：

```bash
# 检查已安装的包
dpkg -l | grep biopass          # Debian/Ubuntu
rpm -qa | grep biopass          # Fedora/RHEL
pacman -Q | grep biopass        # Arch Linux

# 检查 PAM 配置中的上游 PAM 模块
grep -r "pam_biopass" /etc/pam.d/
grep -r "biopass" /usr/share/pam-configs/  # 仅 Debian/Ubuntu
```

如果存在上游 Biopass，您有两个选择：
- **暂时共存**：在上游旁边安装 biopass-rs（推荐用于测试）
- **干净迁移**：先删除上游，然后安装 biopass-rs

### 逐步迁移

1. 安装 `biopass-rs`。

   包的安装后脚本运行：

   ```bash
   /usr/bin/biopass-rs-helper install
   ```

   该命令刷新动态链接器缓存，迁移现有用户配置，并下载所需的 ONNX 模型。

2. 验证您的用户的迁移配置。

   ```bash
   ls ~/.config/biopass-rs/config.yaml
   ls ~/.local/share/biopass-rs
   ```

   如果 `~/.config/biopass-rs/config.yaml` 在安装前已经存在，安装程序不会用上游配置覆盖它。在这种情况下，仅在备份当前配置后手动复制上游配置。

3. 打开桌面应用并查看配置页面。

   确认面部相机、IR 相机、反欺骗设置、启用的方法、方法顺序、忽略的 PAM 服务和模型路径。

4. 仅启用 `biopass-rs` PAM 条目。

   在 Debian 和 Ubuntu 上，运行：

   ```bash
   sudo pam-auth-update
   ```

   从 `biopass-rs` 启用 `Biopass`，如果上游 Biopass 配置文件仍然存在，请禁用它。如果在 Biopass 中启用了指纹认证，还要禁用发行版的 `Fingerprint authentication` 配置文件，否则 `pam_fprintd` 和 Biopass 指纹认证可能会在同一 PAM 堆栈中同时运行。

5. 在新终端中测试。

   ```bash
   sudo -k
   sudo true
   ```

   在成功之前或回退 PAM 更改之前，不要关闭 root shell。

6. （可选）在确认 biopass-rs 工作后删除上游 Biopass 包：

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
   grep -r "pam_biopass" /etc/pam.d/
   ls /usr/lib/security/pam_biopass.so 2>/dev/null
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

它不会删除上游 Biopass 包。

## PAM 冲突

上游 Biopass PAM 模块和 `libbiopass_rs_pam.so` 不应同时为同一 PAM 服务处于活动状态。

如果两者都存在于同一 PAM 堆栈中，两个模块都可能尝试认证同一登录。根据服务顺序，这可能会导致重复提示、相机或指纹设备争用、不一致的穿透行为，或一个模块成功而另一个仍为后面的规则运行。

### 检测冲突

检查两个模块是否都处于活动状态：

```bash
# 列出 auth 堆栈中的所有 PAM 模块
grep "^auth" /etc/pam.d/common-auth 2>/dev/null    # Debian/Ubuntu
grep "^auth" /etc/pam.d/system-auth 2>/dev/null    # Fedora/RHEL/Arch

# 在 PAM 配置中搜索两个模块
grep -r "pam_biopass\|libbiopass_rs_pam" /etc/pam.d/
```

如果您在输出中同时看到 `pam_biopass.so` 和 `libbiopass_rs_pam.so`，则存在冲突。

### 解决冲突

### 解决冲突

**Debian/Ubuntu：**

在 Debian 和 Ubuntu 上，首选 `pam-auth-update` 并仅保持一个 Biopass 配置文件启用。biopass-rs 包安装 `/usr/share/pam-configs/biopass-rs`，其 auth 规则加载：

```pam
auth    sufficient    libbiopass_rs_pam.so
```

修复冲突：
```bash
sudo pam-auth-update
# 从 biopass-rs 启用"Biopass"
# 禁用任何上游 Biopass 配置文件
```

验证修复：
```bash
grep "biopass" /etc/pam.d/common-auth
# 应该只显示 libbiopass_rs_pam.so，而不是 pam_biopass.so
```

**Fedora/RHEL：**

编辑您的 authselect 自定义配置文件或直接编辑 system-auth：

```bash
# 选项 1：使用 authselect 自定义配置文件
sudo vi /etc/authselect/custom/biopass-custom/system-auth

# 选项 2：直接编辑（在 authselect opt-out 后）
sudo vi /etc/pam.d/system-auth
```

删除或注释上游行：
```pam
# auth    sufficient    pam_biopass.so     # 已删除 - 与 biopass-rs 冲突
auth      sufficient    libbiopass_rs_pam.so
```

如果使用 authselect，应用更改：
```bash
sudo authselect select custom/biopass-custom --force
```

验证：
```bash
grep "biopass" /etc/pam.d/system-auth
```

**Arch Linux：**

在 Arch Linux 或任何手动编辑的 PAM 设置上，从您要保护的服务中删除上游模块行，并在密码回退之前插入 biopass-rs 模块，例如：

```bash
sudo vi /etc/pam.d/system-auth
```

删除或注释上游，添加 biopass-rs：
```pam
# auth    sufficient    pam_biopass.so     # 已删除 - 与 biopass-rs 冲突
auth      sufficient    libbiopass_rs_pam.so
auth      [success=1 default=ignore]    pam_unix.so nullok
auth      requisite     pam_deny.so
```

验证：
```bash
grep "biopass" /etc/pam.d/system-auth
```

### 其他冲突

如果在 Biopass 中启用了指纹，除非您有意想要第二个指纹路径，否则不要同时为同一服务保留单独的 `pam_fprintd.so` auth 规则。

删除 `pam_fprintd` 冲突：

**Debian/Ubuntu：**
```bash
sudo pam-auth-update
# 取消选中"Fingerprint authentication"
```

**Fedora/RHEL/Arch：**
```bash
sudo vi /etc/pam.d/system-auth
# 删除或注释 pam_fprintd.so 行
```

## 回滚

要回滚系统登录，禁用 `biopass-rs` PAM 配置文件或从受影响的 PAM 服务中删除 `libbiopass_rs_pam.so` 行，然后在需要时重新启用上游配置文件。

通过在手动迁移期间复制而不是移动，可以保留每用户的上游数据。如果包安装已经移动了数据目录并且您需要返回上游，将其移回：

```bash
mv ~/.local/share/biopass-rs ~/.local/share/com.ticklab.biopass
```

仅当上游版本支持您编写的模式时，才能将配置复制回去。如果您在迁移后在 biopass-rs 中编辑了设置，在将其与上游一起重用之前，请手动查看 YAML。
