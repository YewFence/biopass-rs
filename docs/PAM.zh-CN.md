# PAM 设置指南

简体中文 | [English](PAM.md)

如果您正在从上游 Biopass 迁移，请先阅读[从上游 Biopass 迁移](upstream-migration.zh-CN.md)。上游 PAM 模块和 `libbiopass_rs_pam.so` 不应同时为同一 PAM 服务启用。

## Debian/Ubuntu

1. 验证 Biopass PAM 配置文件存在：
    ```bash
    ls /usr/share/pam-configs/biopass-rs
    ```
2. 启用 Biopass PAM 配置文件。
    ```bash
    sudo pam-auth-update
    ```
3. 启用 `biopass-rs` 提供的 `Biopass` 选项。如果上游 Biopass 配置文件仍然存在，请禁用它。如果选择了 `Fingerprint authentication`，并且您在 Biopass 中启用了指纹认证，请禁用它。
4. 在新终端中测试：
    ```bash
    sudo -k
    sudo true
    ```

## Fedora/RHEL

基于 Fedora 的操作系统（Fedora、RHEL、CentOS、Rocky、Alma 等）使用 `authselect`，而不是 `pam-auth-update`。

按照您的发行版的 `authselect` 工作流程，只为每个服务保持一个 Biopass PAM 模块处于活动状态，并在关闭当前 root 会话之前在新终端中测试。

## Arch Linux

Arch Linux 默认不使用 `pam-auth-update` 或 `authselect`。手动配置 PAM。

在测试时保持 root 终端打开。不正确的 PAM 配置可能会将您锁在系统外。如果同一服务中已存在上游 Biopass 行，请在添加 `libbiopass_rs_pam.so` 之前删除或注释它。

1. 验证 PAM 模块已安装：
    ```bash
    ls /usr/lib/security/libbiopass_rs_pam.so
    ```
2. 编辑您要保护的 PAM 服务，例如：
    ```bash
    sudoedit /etc/pam.d/system-auth
    ```
3. 在现有的 `pam_unix.so` auth 规则之前插入 Biopass：
    ```pam
    auth sufficient libbiopass_rs_pam.so
    auth [success=1 default=ignore] pam_unix.so nullok
    auth requisite pam_deny.so
    ```
4. 在关闭 root 终端之前在新终端中测试：
    ```bash
    sudo -k
    sudo true
    ```
