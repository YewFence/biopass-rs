# PAM Setup Guide

[简体中文](PAM.zh-CN.md) | English

If you are migrating from upstream biopass, read [Migrating from upstream biopass](upstream-migration.md) first. The upstream PAM module and `libbiopass_rs_pam.so` should not both be enabled for the same PAM service.

## Before You Start

**⚠️ Critical Warning**: Incorrect PAM configuration can lock you out of your system. Always keep a root terminal open and test authentication in a separate terminal before closing the root session.

After any PAM change, test in a new terminal:

```bash
sudo -k
sudo true
```

If you enabled fingerprint authentication in biopass-rs, disable `pam_fprintd` for the same PAM service to avoid two fingerprint stacks running at once.

## Debian/Ubuntu

1. Verify the biopass-rs PAM profile exists:
    ```bash
    ls /usr/share/pam-configs/biopass-rs
    ```
2. Enable the biopass-rs PAM profile:
    ```bash
    sudo pam-auth-update
    ```
3. Enable the `Biopass` option from `biopass-rs`. If `Fingerprint authentication` is selected, disable it if you have enabled fingerprint auth in biopass-rs. If upstream biopass is installed, disable its profile too.
4. Verify the configuration was applied:
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```

## Fedora/RHEL

Fedora-based distributions (Fedora, RHEL, CentOS, Rocky, Alma) use `authselect`, not `pam-auth-update`.

If you do not yet have a custom profile, create one based on the current one:

```bash
sudo authselect current
sudo authselect create-profile biopass -b sssd   # or: local / minimal / ...
```

Back up the authselect templates and generated files before editing:

```bash
sudo cp -a /etc/authselect/custom/biopass /etc/authselect/custom/biopass.bak.$(date +%Y%m%d-%H%M%S)
sudo cp -a /etc/pam.d/system-auth /etc/pam.d/system-auth.bak.$(date +%Y%m%d-%H%M%S)
sudo cp -a /etc/pam.d/password-auth /etc/pam.d/password-auth.bak.$(date +%Y%m%d-%H%M%S)
```

Edit the authselect templates and add biopass-rs before the `pam_unix.so` auth rule:

```bash
sudoedit /etc/authselect/custom/biopass/system-auth
sudoedit /etc/authselect/custom/biopass/password-auth
```

```pam
auth        sufficient        libbiopass_rs_pam.so
```

If an upstream `pam_biopass.so` or `libbiopass_pam.so` line is present, comment it out. Apply the changes:

```bash
sudo authselect select custom/biopass --force
sudo authselect apply-changes -b
```

If you enabled fingerprint authentication in biopass-rs and `authselect current` shows `with-fingerprint` enabled, turn it off:

```bash
sudo authselect disable-feature with-fingerprint
sudo authselect apply-changes -b
```

## Arch Linux

Arch Linux does not use `pam-auth-update` or `authselect`. Configure PAM manually.

1. Verify the PAM module is installed:
    ```bash
    ls /usr/lib/security/libbiopass_rs_pam.so
    ```
2. Back up the file you are about to edit:
    ```bash
    sudo cp /etc/pam.d/system-auth /etc/pam.d/system-auth.bak
    ```
3. Edit the PAM service you want to protect:
    ```bash
    sudoedit /etc/pam.d/system-auth
    ```
4. Insert biopass-rs before the existing `pam_unix.so` auth rule:
    ```pam
    auth      sufficient  libbiopass_rs_pam.so
    auth      [success=1 default=ignore]  pam_unix.so nullok
    auth      requisite   pam_deny.so
    ```
5. If an upstream `pam_biopass.so` or `libbiopass_pam.so` line is present, comment it out. If you enabled fingerprint authentication in biopass-rs, check for and remove any `pam_fprintd.so` lines in the same stack:
    ```bash
    grep -r "pam_fprintd.so" /etc/pam.d/
    ```

## Troubleshooting

### Authentication Not Triggering

1. Check the PAM module is referenced:
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```
2. Check the module file exists:
    ```bash
    ls -l /usr/lib/security/libbiopass_rs_pam.so    # Most distros
    ls -l /lib/security/libbiopass_rs_pam.so        # Some Debian-based
    ls -l /usr/lib64/security/libbiopass_rs_pam.so  # Some RHEL-based
    ```
3. Enable debug in `~/.config/biopass-rs/config.yaml` and watch the journal:
    ```bash
    sudo journalctl -f | grep biopass
    ```

### Camera Permission Issues

```bash
ls -l /dev/video*
sudo usermod -aG video $USER
```

For systemd services (like polkit), see [Polkit](Polkit.md).

### SELinux Camera Denial on the Fedora Login Screen

On Fedora/RHEL-based systems, the graphical login screen may log a SELinux denial like:

```text
SELinux is preventing biopass-rs-help from map access on the chr_file /dev/video0
```

When inspected with `ausearch`, the denial commonly looks like:

```text
scontext=system_u:system_r:xdm_t:s0-s0:c0.c1023
tcontext=system_u:object_r:v4l_device_t:s0
tclass=chr_file
denied { map }
```

`biopass-rs-help` is usually the kernel-truncated process name for `biopass-rs-helper`. `xdm_t` means the helper is running from the GDM or display-manager login-screen context, `v4l_device_t` means `/dev/video0` has the normal video-device SELinux label, and `map` means the camera stack tried to memory-map the video device.

If the denial only appears at the boot login screen and biopass-rs works after you sign in, it is usually safe to leave it alone. If you need face authentication on the login screen and it fails in SELinux enforcing mode, inspect the denial first:

```bash
sudo ausearch -m avc,user_avc -ts boot -c biopass-rs-help
```

After confirming that the denial only involves `xdm_t`, `v4l_device_t`, and `chr_file map`, generate a local SELinux module and review it before installing:

```bash
sudo ausearch -m avc,user_avc -ts boot -c biopass-rs-help --raw | audit2allow -M biopass-rs-helper-local
cat biopass-rs-helper-local.te
```

The generated rule is usually:

```te
allow xdm_t v4l_device_t:chr_file map;
```

This allows only `map` access from the display-manager domain to video devices, but it applies to the whole `xdm_t` domain rather than only to `biopass-rs-helper`. If that scope is acceptable for your machine, install the module:

```bash
sudo semodule -i biopass-rs-helper-local.pp
```

To remove it later:

```bash
sudo semodule -r biopass-rs-helper-local
```
