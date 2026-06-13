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
