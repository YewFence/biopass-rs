# PAM 设置指南

简体中文 | [English](PAM.md)

如果您正在从上游 Biopass 迁移，请先阅读[从上游 Biopass 迁移](upstream-migration.zh-CN.md)。上游 PAM 模块和 `libbiopass_rs_pam.so` 不应同时为同一 PAM 服务启用。

## 开始之前

**⚠️ 严重警告**：不正确的 PAM 配置可能会将您锁在系统外。始终保持 root 终端打开，并在关闭 root 会话之前在单独的终端中测试认证。

### 检查上游 Biopass

在启用 biopass-rs 之前，检查上游 Biopass 是否已安装并处于活动状态：

```bash
# 检查上游 Biopass 包是否已安装
dpkg -l | grep biopass          # Debian/Ubuntu
rpm -qa | grep biopass          # Fedora/RHEL
pacman -Q | grep biopass        # Arch Linux

# 检查上游 PAM 模块
ls /usr/lib/security/pam_biopass.so 2>/dev/null || \
ls /lib/security/pam_biopass.so 2>/dev/null || \
echo "未找到上游 PAM 模块"

# 检查哪些 PAM 服务引用了 Biopass
grep -r "pam_biopass\|biopass" /etc/pam.d/ 2>/dev/null
grep -r "biopass" /usr/share/pam-configs/ 2>/dev/null  # Debian/Ubuntu
```

如果存在上游 Biopass，请参阅下面的[从上游 Biopass 迁移](#从上游-biopass-迁移)部分。

## Debian/Ubuntu

1. 验证 Biopass PAM 配置文件存在：
    ```bash
    ls /usr/share/pam-configs/biopass-rs
    ```

2. 启用 Biopass PAM 配置文件：
    ```bash
    sudo pam-auth-update
    ```

3. 在对话框中：
   - **启用** `biopass-rs` 提供的 `Biopass`
   - **禁用**任何上游 Biopass 配置文件（如果存在）
   - **禁用** `Fingerprint authentication`（如果您在 Biopass 中启用了指纹，以避免与 `pam_fprintd` 冲突）

4. 验证 PAM 配置已应用：
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```
    
    您应该看到类似这样的行：
    ```
    /etc/pam.d/common-auth:auth	[success=2 default=ignore]	libbiopass_rs_pam.so
    ```

5. 在新终端中测试（保持 root 终端打开）：
    ```bash
    sudo -k
    sudo true
    ```
    
    如果认证失败，切换回 root 终端并运行 `sudo pam-auth-update` 进行回退。

## Fedora/RHEL

基于 Fedora 的发行版（Fedora、RHEL、CentOS、Rocky、Alma）使用 `authselect` 而不是 `pam-auth-update`。

### 检查当前配置文件

```bash
# 检查活动的 authselect 配置文件
sudo authselect current

# 列出可用的配置文件
sudo authselect list
```

### 选项 1：使用自定义配置文件（推荐）

1. 基于当前配置文件创建自定义配置文件：
    ```bash
    # 如果使用 sssd 配置文件
    sudo authselect create-profile biopass-custom -b sssd
    
    # 如果使用 minimal 配置文件
    sudo authselect create-profile biopass-custom -b minimal
    ```

2. 编辑自定义配置文件以添加 biopass-rs：
    ```bash
    sudo vi /etc/authselect/custom/biopass-custom/system-auth
    ```
    
    在 `pam_unix.so` auth 行之前添加此行：
    ```pam
    auth        sufficient    libbiopass_rs_pam.so
    ```
    
    如果存在上游 Biopass，删除或注释掉其行：
    ```pam
    # auth        sufficient    pam_biopass.so     # 已注释 - 使用 biopass-rs 代替
    ```

3. 应用自定义配置文件：
    ```bash
    sudo authselect select custom/biopass-custom --force
    ```

4. 验证更改：
    ```bash
    grep -r "biopass" /etc/pam.d/
    cat /etc/pam.d/system-auth
    ```

### 选项 2：直接编辑 PAM 文件（高级）

如果您更喜欢直接编辑 PAM 文件而不使用 authselect：

1. **禁用 authselect**（这使 PAM 文件可编辑）：
    ```bash
    sudo authselect opt-out
    ```

2. 编辑 `/etc/pam.d/system-auth`：
    ```bash
    sudo vi /etc/pam.d/system-auth
    ```
    
    在 `pam_unix.so` 之前添加：
    ```pam
    auth        sufficient    libbiopass_rs_pam.so
    auth        [success=1 default=ignore]    pam_unix.so nullok
    auth        requisite     pam_deny.so
    ```

3. **警告**：当 authselect 被禁用时，您必须手动维护 PAM 配置。系统更新不会自动更新 PAM 文件。

### 测试配置

保持 root 终端打开并在新终端中测试：
```bash
sudo -k
sudo true
```

如果认证失败，返回 root 终端并回退：
```bash
# 如果使用 authselect
sudo authselect select sssd --force

# 如果您选择退出 authselect，恢复备份
sudo cp /etc/pam.d/system-auth.bak /etc/pam.d/system-auth
```

## Arch Linux

Arch Linux 默认不使用 `pam-auth-update` 或 `authselect`。手动配置 PAM。

**在测试时保持 root 终端打开。** 不正确的 PAM 配置可能会将您锁在系统外。

1. 验证 PAM 模块已安装：
    ```bash
    ls /usr/lib/security/libbiopass_rs_pam.so
    ```

2. 备份 PAM 配置：
    ```bash
    sudo cp /etc/pam.d/system-auth /etc/pam.d/system-auth.bak
    ```

3. 检查并删除上游 Biopass 模块：
    ```bash
    # 检查上游模块是否存在
    grep "pam_biopass" /etc/pam.d/system-auth
    
    # 如果找到，删除或注释掉它
    sudo vi /etc/pam.d/system-auth
    ```

4. 编辑 PAM 服务：
    ```bash
    sudoedit /etc/pam.d/system-auth
    ```
    
    在现有的 `pam_unix.so` auth 规则之前插入 biopass-rs：
    ```pam
    auth      sufficient  libbiopass_rs_pam.so
    auth      [success=1 default=ignore]  pam_unix.so nullok
    auth      requisite   pam_deny.so
    ```

5. 在关闭 root 终端之前在新终端中测试：
    ```bash
    sudo -k
    sudo true
    ```
    
    如果失败，在 root 终端中恢复备份：
    ```bash
    sudo cp /etc/pam.d/system-auth.bak /etc/pam.d/system-auth
    ```

## 从上游 Biopass 迁移

如果您安装了上游 Biopass，您需要禁用或删除它以避免冲突。

### 选项 1：仅禁用上游 PAM 模块

保持上游包安装但禁用其 PAM 模块：

**Debian/Ubuntu：**
```bash
sudo pam-auth-update
# 禁用上游 Biopass 选项，启用 biopass-rs
```

**Fedora/RHEL：**
```bash
# 编辑自定义配置文件或 system-auth 文件
sudo vi /etc/pam.d/system-auth
# 注释掉或删除上游 pam_biopass.so 行
# 添加 libbiopass_rs_pam.so 行
```

**Arch Linux：**
```bash
sudo vi /etc/pam.d/system-auth
# 注释掉或删除上游 pam_biopass.so 行
# 添加 libbiopass_rs_pam.so 行
```

### 选项 2：完全删除上游 Biopass

如果您想完全切换到 biopass-rs，卸载上游包：

**Debian/Ubuntu：**
```bash
# 查找包名
dpkg -l | grep biopass

# 删除包（用实际包名替换）
sudo apt remove biopass
sudo apt autoremove

# 验证删除
dpkg -l | grep biopass
ls /usr/lib/security/pam_biopass.so 2>/dev/null
```

**Fedora/RHEL：**
```bash
# 查找包名
rpm -qa | grep biopass

# 删除包
sudo dnf remove biopass  # 或 sudo yum remove biopass

# 验证删除
rpm -qa | grep biopass
ls /usr/lib64/security/pam_biopass.so 2>/dev/null
```

**Arch Linux：**
```bash
# 查找包名
pacman -Q | grep biopass

# 删除包
sudo pacman -R biopass

# 验证删除
pacman -Q | grep biopass
ls /usr/lib/security/pam_biopass.so 2>/dev/null
```

### 删除上游后

1. 验证没有上游 PAM 引用残留：
    ```bash
    grep -r "pam_biopass" /etc/pam.d/
    ```

2. 按照上面针对您的发行版的说明启用 biopass-rs。

3. 验证 biopass-rs 处于活动状态：
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```

## 故障排除

### 认证未触发

如果 Biopass 在认证期间未激活：

1. 检查 PAM 模块是否已加载：
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```

2. 检查 PAM 模块文件是否存在：
    ```bash
    ls -l /usr/lib/security/libbiopass_rs_pam.so    # 大多数发行版
    ls -l /lib/security/libbiopass_rs_pam.so        # 某些基于 Debian 的
    ls -l /usr/lib64/security/libbiopass_rs_pam.so  # 某些基于 RHEL 的
    ```

3. 检查 Biopass 配置：
    ```bash
    ls ~/.config/biopass-rs/config.yaml
    cat ~/.config/biopass-rs/config.yaml | grep -A5 "enabled_methods"
    ```

4. 启用调试模式并检查日志：
    ```bash
    # 在配置中启用调试
    vi ~/.config/biopass-rs/config.yaml
    # 设置 debug: true
    
    # 尝试认证并检查系统日志
    sudo journalctl -f | grep biopass
    ```

### 相机权限问题

如果认证由于相机访问而失败：

```bash
# 检查相机权限
ls -l /dev/video*

# 将用户添加到 video 组
sudo usermod -aG video $USER

# 对于 systemd 服务（如 polkit），请参阅 docs/Polkit.zh-CN.md
```

### 与 pam_fprintd 的冲突

如果您在 Biopass 中启用了指纹，请禁用 `pam_fprintd`：

**Debian/Ubuntu：**
```bash
sudo pam-auth-update
# 取消选中"Fingerprint authentication"
```

**Fedora/RHEL/Arch：**
```bash
# 从 /etc/pam.d/system-auth 删除或注释 pam_fprintd.so 行
sudo vi /etc/pam.d/system-auth
```
