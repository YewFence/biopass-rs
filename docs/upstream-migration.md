# Migrating from upstream biopass

[简体中文](upstream-migration.zh-CN.md) | English

## TL;DR

After you install `biopass-rs`, the install script automatically copies your enrolled face images from upstream biopass, but it does **not** migrate the config. You need to re-enter your config in the desktop app, and replace the PAM module following the [PAM setup guide](PAM.md).

## Introduction

biopass-rs is an unofficial Rust rewrite of upstream [biopass](https://github.com/TickLabVN/biopass). It uses different binary names, PAM module names, and per-user storage paths, so a migration has two separate parts:

1. User configuration and enrolled biometric data.
2. System PAM configuration.

This document covers the migration outside of PAM — that is, user config, user data, config schema, and post-install migration behavior. For PAM configuration, upstream PAM module replacement, `pam_fprintd` conflict handling, and distro-specific differences, see the [PAM setup guide](PAM.md).

## What changes

| Item | upstream biopass | biopass-rs |
| :--- | :--- | :--- |
| User config | `~/.config/com.ticklab.biopass/config.yaml` | `~/.config/biopass-rs/config.yaml` |
| User data | `~/.local/share/com.ticklab.biopass` | `~/.local/share/biopass-rs` |
| Helper binary | `biopass-helper` | `/usr/bin/biopass-rs-helper` |
| PAM module | upstream PAM module | `libbiopass_rs_pam.so`, see the [PAM setup guide](PAM.md) for details |
| Debian PAM profile | upstream profile, commonly `biopass` | `/usr/share/pam-configs/biopass-rs`, see the [PAM setup guide](PAM.md) for details |

The config schema has diverged from upstream (the anti-spoofing section, for example, was split into explicit `ai` and `ir` sub-configs). biopass-rs does **not** auto-migrate the upstream config — upstream's schema changes every release and maintaining a converter for every version is unsustainable. The installer writes a fresh default config and copies your enrolled **face images** (which are schema-independent); everything else you re-enter in the desktop app.

## Confirm status

Verify whether upstream biopass is installed and active:

```bash
# Check for installed package
dpkg -l | grep biopass          # Debian/Ubuntu
rpm -qa | grep biopass          # Fedora/RHEL
pacman -Q | grep biopass        # Arch Linux

# Check for upstream PAM modules in PAM configs.
# Depending on the upstream package version, the module can be named
# pam_biopass.so or libbiopass_pam.so.
grep -r "pam_biopass\|libbiopass_pam" /etc/pam.d/
grep -r "biopass" /usr/share/pam-configs/  # Debian/Ubuntu only
```

### Migration steps

1. Install `biopass-rs`.

   The package post-install script runs:

   ```bash
   /usr/bin/biopass-rs-helper install
   ```

   That command refreshes the dynamic linker cache, writes a default config (it does **not** import the upstream config), copies any enrolled face images from the upstream data directory, and downloads the required ONNX models.

2. Verify the install landed the expected files.

   ```bash
   ls ~/.config/biopass-rs/config.yaml
   ls ~/.local/share/biopass-rs/faces
   ```

3. Open the desktop app and adjust the config to your needs.

4. Configure PAM.

   This document does not cover PAM configuration. Follow the "clean install" or "migrate from upstream" flow for your distro in the [PAM setup guide](PAM.md) to enable `libbiopass_rs_pam.so` and disable the upstream biopass PAM module.

5. (Optional) Remove the upstream biopass package after confirming biopass-rs works:

   **Debian/Ubuntu:**
   ```bash
   sudo apt remove biopass
   sudo apt autoremove
   ```

   **Fedora/RHEL:**
   ```bash
   sudo dnf remove biopass
   ```

   **Arch Linux:**
   ```bash
   sudo pacman -R biopass
   ```

   After removal, verify no upstream references remain:
   ```bash
   grep -r "pam_biopass\|libbiopass_pam" /etc/pam.d/
   ls /usr/lib/security/pam_biopass.so 2>/dev/null
   ls /usr/lib64/security/libbiopass_pam.so 2>/dev/null
   ```
