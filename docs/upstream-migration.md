# Migrating from upstream Biopass

[简体中文](upstream-migration.zh-CN.md) | English

This project is an unofficial Rust rewrite of the upstream [TickLabVN/biopass](https://github.com/TickLabVN/biopass) project. It uses different binary names, PAM module names, and per-user storage paths, so a migration has two separate parts:

1. User configuration and enrolled biometric data.
2. System PAM configuration.

Keep a root shell open while changing PAM. Test in a second terminal before closing the root shell.

## What changes

| Item | Upstream Biopass | biopass-rs |
| :--- | :--- | :--- |
| User config | `~/.config/com.ticklab.biopass/config.yaml` | `~/.config/biopass-rs/config.yaml` |
| User data | `~/.local/share/com.ticklab.biopass` | `~/.local/share/biopass-rs` |
| Helper binary | upstream helper | `/usr/bin/biopass-rs-helper` |
| PAM module | upstream PAM module | `libbiopass_rs_pam.so` |
| Debian PAM profile | upstream profile, commonly `biopass` | `/usr/share/pam-configs/biopass-rs` |

The config schema is mostly compatible, but the anti-spoofing section was split into explicit `ai` and `ir` sub-configs in this rewrite. The migration code rewrites old anti-spoofing fields into the current schema.

## Recommended package migration

1. Install `biopass-rs`.

   The package post-install script runs:

   ```bash
   /usr/bin/biopass-rs-helper install
   ```

   That command refreshes the dynamic linker cache, migrates existing user configurations, and downloads the required ONNX models.

2. Verify the migrated config for your user.

   ```bash
   ls ~/.config/biopass-rs/config.yaml
   ls ~/.local/share/biopass-rs
   ```

   If `~/.config/biopass-rs/config.yaml` already existed before install, the installer does not overwrite it with the upstream config. In that case, copy the upstream config manually only after backing up the current one.

3. Open the desktop app and review the Configuration page.

   Confirm the face camera, IR camera, anti-spoofing settings, enabled methods, method order, ignored PAM services, and model paths.

4. Enable only the `biopass-rs` PAM entry.

   On Debian and Ubuntu, run:

   ```bash
   sudo pam-auth-update
   ```

   Enable `Biopass` from `biopass-rs`, and disable the upstream Biopass profile if it is still present. Also disable the distro `Fingerprint authentication` profile if fingerprint authentication is enabled inside Biopass, otherwise `pam_fprintd` and Biopass fingerprint auth can both run in the same PAM stack.

5. Test in a new terminal.

   ```bash
   sudo -k
   sudo true
   ```

   Do not close the root shell until this succeeds or until you have reverted the PAM change.

## Manual migration

Use this flow when you are developing from the repository, when the package post-install script did not run, or when you want to migrate one user explicitly.

1. Copy the upstream config only if the new config does not exist.

   ```bash
   mkdir -p ~/.config/biopass-rs
   cp ~/.config/com.ticklab.biopass/config.yaml ~/.config/biopass-rs/config.yaml
   ```

2. Move or copy the user data directory.

   ```bash
   mv ~/.local/share/com.ticklab.biopass ~/.local/share/biopass-rs
   ```

   Use `cp -a` instead of `mv` if you want to keep the upstream install working during testing.

3. Migrate the copied config schema.

   ```bash
   biopass-rs-helper migrate --username "$USER"
   ```

   When developing from this repository, use:

   ```bash
   mise run helper -- migrate --username "$USER"
   ```

4. Install or verify the ONNX models.

   ```bash
   sudo /usr/bin/biopass-rs-helper install
   ```

   In development, follow the model setup shown in the AI Models page or run the helper binary you built locally.

## What `migrate` does and does not do

`biopass-rs-helper migrate --username <user>` only rewrites the current biopass-rs config file at `~/.config/biopass-rs/config.yaml`.

It does not copy `~/.config/com.ticklab.biopass/config.yaml` into the new location. The package `install` command and the desktop app do that first-start copy when the new config does not already exist.

It does not move `~/.local/share/com.ticklab.biopass` into `~/.local/share/biopass-rs`. The package `install` command attempts that data directory migration for all users when the new data directory is absent.

It does not edit `/etc/pam.d/*`, run `pam-auth-update`, remove upstream PAM profiles, or disable `pam_fprintd`.

It does not remove the upstream Biopass package.

## PAM conflicts

The upstream Biopass PAM module and `libbiopass_rs_pam.so` should not both be active for the same PAM service.

If both are present in the same PAM stack, both modules can try to authenticate the same login. Depending on the service order, that may cause duplicate prompts, camera or fingerprint device contention, inconsistent fall-through behavior, or one module succeeding while the other still runs for a later rule.

On Debian and Ubuntu, prefer `pam-auth-update` and keep only one Biopass profile enabled. The biopass-rs package installs `/usr/share/pam-configs/biopass-rs`, whose auth rule loads:

```pam
auth    sufficient    libbiopass_rs_pam.so
```

On Arch Linux or any manually edited PAM setup, remove the upstream module line from the services you want to protect and insert the biopass-rs module before the password fallback, for example:

```pam
auth sufficient libbiopass_rs_pam.so
auth [success=1 default=ignore] pam_unix.so nullok
auth requisite pam_deny.so
```

If fingerprint is enabled in Biopass, do not also keep a separate `pam_fprintd.so` auth rule for the same service unless you intentionally want a second fingerprint path.

## Rollback

To roll back system sign-in, disable the `biopass-rs` PAM profile or remove the `libbiopass_rs_pam.so` line from the affected PAM service, then re-enable the upstream profile if needed.

The per-user upstream data can be preserved by copying instead of moving during manual migration. If the package install already moved the data directory and you need to return to upstream, move it back:

```bash
mv ~/.local/share/biopass-rs ~/.local/share/com.ticklab.biopass
```

The config can be copied back only if the upstream version supports the schema you wrote. If you edited settings in biopass-rs after migration, review the YAML manually before reusing it with upstream.
