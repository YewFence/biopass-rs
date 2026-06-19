# PAM 设置指南

简体中文 | [English](PAM.md)

如果您是从上游 biopass 迁移过来的，请先阅读[从上游 biopass 迁移](upstream-migration.zh-CN.md)。上游 PAM 模块和 `libbiopass_rs_pam.so` 不应同时为同一个 PAM 服务启用。

## 开始之前

**严重警告**：不正确的 PAM 配置可能会将您锁在系统外。始终保持 root 终端打开，并在单独的终端中测试一切正常后再关闭 root 会话。

```bash
sudo -i
```

在另一个普通终端中完成所有 PAM 改动后使用如下命令测试：

```bash
sudo -k
sudo true
```

如果您在 biopass-rs 中启用了指纹认证，请同时禁用发行版自带的 `pam_fprintd.so` 认证路径，避免两套指纹认证同时运行。

## Debian/Ubuntu

1. 确认 biopass-rs 的 PAM profile 文件存在：
    ```bash
    ls /usr/share/pam-configs/biopass-rs
    ```
2. 运行：
    ```bash
    sudo pam-auth-update
    ```
3. 启用 `biopass-rs` 提供的 `Biopass` profile；如果同时启用了上游 biopass，请禁用上游 profile。如果您在 biopass-rs 中启用了指纹认证，请禁用发行版的 `Fingerprint authentication`。
4. 验证：
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```

## Fedora/RHEL

基于 Fedora 的发行版（Fedora、RHEL、CentOS、Rocky、Alma）使用 `authselect`。

如果您还没有自定义 profile，基于当前 profile 创建一个：

```bash
sudo authselect current
sudo authselect create-profile biopass -b sssd   # 或: local / minimal / ...
```

编辑前先备份 authselect 模板和生成文件：

```bash
sudo cp -a /etc/authselect/custom/biopass /etc/authselect/custom/biopass.bak.$(date +%Y%m%d-%H%M%S)
sudo cp -a /etc/pam.d/system-auth /etc/pam.d/system-auth.bak.$(date +%Y%m%d-%H%M%S)
sudo cp -a /etc/pam.d/password-auth /etc/pam.d/password-auth.bak.$(date +%Y%m%d-%H%M%S)
```

编辑 authselect 模板，把 biopass-rs 放在 `pam_unix.so` auth 行之前：

```bash
sudoedit /etc/authselect/custom/biopass/system-auth
sudoedit /etc/authselect/custom/biopass/password-auth
```

```pam
auth        sufficient        libbiopass_rs_pam.so
```

如果存在上游 `pam_biopass.so` 或 `libbiopass_pam.so` 行，请注释掉。应用变更：

```bash
sudo authselect select custom/biopass --force
sudo authselect apply-changes -b
```

如果您在 biopass-rs 中启用了指纹认证，并且 `authselect current` 显示启用了 `with-fingerprint`，请关闭系统单独的指纹路径：

```bash
sudo authselect disable-feature with-fingerprint
sudo authselect apply-changes -b
```

## Arch Linux

Arch Linux 默认不使用 `pam-auth-update` 或 `authselect`，需要手动编辑 PAM 文件。

1. 确认 PAM 模块已经安装：
    ```bash
    ls /usr/lib/security/libbiopass_rs_pam.so
    ```
2. 备份要编辑的 PAM 文件：
    ```bash
    sudo cp /etc/pam.d/system-auth /etc/pam.d/system-auth.bak
    ```
3. 编辑 PAM 文件：
    ```bash
    sudoedit /etc/pam.d/system-auth
    ```
4. 在现有的 `pam_unix.so` auth 规则之前插入 biopass-rs：
    ```pam
    auth      sufficient  libbiopass_rs_pam.so
    auth      [success=1 default=ignore]  pam_unix.so nullok
    auth      requisite   pam_deny.so
    ```
5. 如果存在上游 `pam_biopass.so` 或 `libbiopass_pam.so` 行，请注释掉。如果您在 biopass-rs 中启用了指纹认证，请检查同一个 PAM 栈里是否还有 `pam_fprintd.so`，并按需删除或注释：
    ```bash
    grep -r "pam_fprintd.so" /etc/pam.d/
    ```

## 故障排查

### 认证没有触发

1. 检查 PAM 模块是否被引用：
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```
2. 检查模块文件是否存在：
    ```bash
    ls -l /usr/lib/security/libbiopass_rs_pam.so    # 大多数发行版
    ls -l /lib/security/libbiopass_rs_pam.so        # 部分 Debian 系发行版
    ls -l /usr/lib64/security/libbiopass_rs_pam.so  # 部分 RHEL 系发行版
    ```
3. 在 `~/.config/biopass-rs/config.yaml` 中启用 debug，然后查看 journal：
    ```bash
    sudo journalctl -f | grep biopass
    ```

### 摄像头权限问题

```bash
ls -l /dev/video*
sudo usermod -aG video $USER
```

对于 systemd 服务（例如 polkit），请参阅 [Polkit](Polkit.zh-CN.md)。

### Fedora 登录界面的 SELinux 摄像头拒绝

在 Fedora/RHEL 系系统上，开机登录界面可能会记录如下 SELinux 拒绝：

```text
SELinux is preventing biopass-rs-help from map access on the chr_file /dev/video0
```

使用 `ausearch` 查看时，常见上下文类似：

```text
scontext=system_u:system_r:xdm_t:s0-s0:c0.c1023
tcontext=system_u:object_r:v4l_device_t:s0
tclass=chr_file
denied { map }
```

这里的 `biopass-rs-help` 通常是 `biopass-rs-helper` 被内核进程名长度截断后的显示。`xdm_t` 表示 helper 是在 GDM 或其他显示管理器的登录界面上下文中运行，`v4l_device_t` 表示 `/dev/video0` 的 SELinux 标签是视频设备，`map` 表示摄像头库尝试以内存映射方式读取设备。

如果这条拒绝只在开机登录界面出现，而登录后 biopass-rs 和摄像头都能正常使用，可以暂时忽略。若确实需要在登录界面使用人脸认证，并且强制模式下失败，可以先查看拒绝记录：

```bash
sudo ausearch -m avc,user_avc -ts boot -c biopass-rs-help
```

确认只涉及 `xdm_t`、`v4l_device_t` 和 `chr_file map` 后，可以生成本机 SELinux 策略模块并先检查内容：

```bash
sudo ausearch -m avc,user_avc -ts boot -c biopass-rs-help --raw | audit2allow -M biopass-rs-helper-local
cat biopass-rs-helper-local.te
```

生成的规则通常类似：

```te
allow xdm_t v4l_device_t:chr_file map;
```

这条规则只放开登录界面域对视频设备的 `map` 权限，但它作用于整个 `xdm_t` 域，而不是只作用于 `biopass-rs-helper`。如果这个范围符合本机需求，再安装模块：

```bash
sudo semodule -i biopass-rs-helper-local.pp
```

如果之后想撤销：

```bash
sudo semodule -r biopass-rs-helper-local
```
