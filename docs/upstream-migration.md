# Migrating from upstream biopass

[简体中文](upstream-migration.zh-CN.md) | English

biopass-rs is an unofficial Rust rewrite of upstream [biopass](https://github.com/TickLabVN/biopass). It uses different binary names, PAM module names, and per-user storage paths, so a migration has two separate parts:

1. User configuration and enrolled biometric data.
2. System PAM configuration.

Keep a root shell open while changing PAM. Test in a second terminal before closing the root shell.

## What changes

| Item | upstream biopass | biopass-rs |
| :--- | :--- | :--- |
| User config | `~/.config/com.ticklab.biopass/config.yaml` | `~/.config/biopass-rs/config.yaml` |
| User data | `~/.local/share/com.ticklab.biopass` | `~/.local/share/biopass-rs` |
| Helper binary | upstream helper | `/usr/bin/biopass-rs-helper` |
| PAM module | upstream PAM module | `libbiopass_rs_pam.so` |
| Debian PAM profile | upstream profile, commonly `biopass` | `/usr/share/pam-configs/biopass-rs` |

The config schema is mostly compatible, but the anti-spoofing section was split into explicit `ai` and `ir` sub-configs in this rewrite. The migration code rewrites old anti-spoofing fields into the current schema.

## Recommended package migration

**Before starting**, verify if upstream biopass is installed and active:

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

If upstream biopass is present, you have two choices:
- **Coexist temporarily**: Install biopass-rs alongside upstream (recommended for testing)
- **Clean migration**: Remove upstream first, then install biopass-rs

### Step-by-step migration

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

   Enable the `Biopass` profile provided by `biopass-rs`, and disable the upstream biopass profile if it is still present. Also disable the distro `Fingerprint authentication` profile if fingerprint authentication is enabled inside biopass-rs, otherwise `pam_fprintd` and biopass-rs fingerprint auth can both run in the same PAM stack.

5. Test in a new terminal.

   ```bash
   sudo -k
   sudo true
   ```

   Do not close the root shell until this succeeds or until you have reverted the PAM change.

6. (Optional) Remove the upstream biopass package after confirming biopass-rs works:

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

It does not remove the upstream biopass package.

## PAM conflicts

The upstream biopass PAM module and `libbiopass_rs_pam.so` should not both be active for the same PAM service.

If both are present in the same PAM stack, both modules can try to authenticate the same login. Depending on the service order, that may cause duplicate prompts, camera or fingerprint device contention, inconsistent fall-through behavior, or one module succeeding while the other still runs for a later rule.

### Detecting conflicts

Check if both modules are active:

```bash
# List all PAM modules in auth stack
grep "^auth" /etc/pam.d/common-auth 2>/dev/null    # Debian/Ubuntu
grep "^auth" /etc/pam.d/system-auth 2>/dev/null    # Fedora/RHEL/Arch

# Search for upstream and biopass-rs modules across PAM configs
grep -r "pam_biopass\|libbiopass_pam\|libbiopass_rs_pam" /etc/pam.d/
```

If you see an upstream module such as `pam_biopass.so` or `libbiopass_pam.so` and `libbiopass_rs_pam.so` in the same PAM stack, you have a conflict.

### Resolving conflicts

**Debian/Ubuntu:**

On Debian and Ubuntu, prefer `pam-auth-update` and keep only one biopass-related profile enabled. The biopass-rs package installs `/usr/share/pam-configs/biopass-rs`, whose auth rule loads:

```pam
auth    sufficient    libbiopass_rs_pam.so
```

To fix conflicts:
```bash
sudo pam-auth-update
# Enable the "Biopass" profile from biopass-rs
# Disable any upstream biopass profile
```

Verify the fix:
```bash
grep "biopass" /etc/pam.d/common-auth
# Should only show libbiopass_rs_pam.so, not pam_biopass.so or libbiopass_pam.so
```

**Fedora/RHEL:**

Fedora-based distributions commonly generate `/etc/pam.d/system-auth` and `/etc/pam.d/password-auth` from the selected `authselect` profile. If you used an upstream biopass RPM before, uninstalling that package can remove the upstream PAM module file but leave a custom profile such as `custom/biopass` selected. In that case, the generated PAM file can still contain an old upstream line such as:

```pam
auth        sufficient        libbiopass_pam.so
```

Check the active profile first:

```bash
sudo authselect current
```

If the active profile is `custom/biopass`, edit files under `/etc/authselect/custom/biopass/`. If the active profile is a different custom profile, replace `biopass` in the paths below with that profile name.

Before editing PAM, keep a root shell open and back up the current custom profile:

```bash
sudo -i
cp -a /etc/authselect/custom/biopass /etc/authselect/custom/biopass.bak.$(date +%Y%m%d-%H%M%S)
cp -a /etc/pam.d/system-auth /etc/pam.d/system-auth.bak.$(date +%Y%m%d-%H%M%S)
cp -a /etc/pam.d/password-auth /etc/pam.d/password-auth.bak.$(date +%Y%m%d-%H%M%S)
```

Install the biopass-rs RPM and verify the module and helper exist before changing PAM:

```bash
sudo dnf install ./biopass-rs-*.rpm
ls -l /lib64/security/libbiopass_rs_pam.so
ls -l /usr/bin/biopass-rs-helper
```

Then replace upstream biopass lines in the authselect templates:

```bash
sudoedit /etc/authselect/custom/biopass/system-auth
sudoedit /etc/authselect/custom/biopass/password-auth
```

`system-auth` is the common stack used by many local authentication services, such as `sudo`, `su`, or local login, depending on the service files under `/etc/pam.d/`.

`password-auth` is a second common stack used by some password-oriented services, such as display managers, SSH, or other services, depending on the service files under `/etc/pam.d/`.

It is normal for the upstream biopass line to appear only in `system-auth`. Do not add biopass-rs to `password-auth` just because `system-auth` has it. Add it only if the services you want to protect include `password-auth`.

Replace an upstream line such as:

```pam
auth        sufficient        libbiopass_pam.so
```

or:

```pam
auth        sufficient        pam_biopass.so
```

with:

```pam
auth        sufficient        libbiopass_rs_pam.so
```

Apply the authselect template changes:

```bash
sudo authselect apply-changes -b
```

If fingerprint authentication is enabled inside biopass-rs, disable authselect's separate `pam_fprintd` path unless you intentionally want a second fingerprint route:

```bash
sudo authselect disable-feature with-fingerprint
sudo authselect apply-changes -b
```

Verify:

```bash
sudo authselect current
grep "biopass" /etc/pam.d/system-auth
grep "biopass" /etc/pam.d/password-auth
grep -R "pam_fprintd.so" /etc/pam.d/system-auth /etc/pam.d/password-auth /etc/authselect/custom/biopass
sudo -k
sudo true
```

**Arch Linux:**

On Arch Linux or any manually edited PAM setup, remove the upstream module line from the services you want to protect and insert the biopass-rs module before the password fallback, for example:

```bash
sudo vi /etc/pam.d/system-auth
```

Remove or comment upstream, add biopass-rs:
```pam
# auth    sufficient    pam_biopass.so     # REMOVED - conflicts with biopass-rs
auth      sufficient    libbiopass_rs_pam.so
auth      [success=1 default=ignore]    pam_unix.so nullok
auth      requisite     pam_deny.so
```

Verify:
```bash
grep "biopass" /etc/pam.d/system-auth
```

### Additional conflicts

If fingerprint is enabled in biopass-rs, do not also keep a separate `pam_fprintd.so` auth rule for the same service unless you intentionally want a second fingerprint path.

To remove `pam_fprintd` conflicts:

**Debian/Ubuntu:**
```bash
sudo pam-auth-update
# Uncheck "Fingerprint authentication"
```

**Fedora/RHEL/Arch:**
```bash
sudo vi /etc/pam.d/system-auth
# Remove or comment the pam_fprintd.so line
```

## Rollback

To roll back system sign-in, disable the `biopass-rs` PAM profile or remove the `libbiopass_rs_pam.so` line from the affected PAM service, then re-enable the upstream profile if needed.

The per-user upstream data can be preserved by copying instead of moving during manual migration. If the package install already moved the data directory and you need to return to upstream, move it back:

```bash
mv ~/.local/share/biopass-rs ~/.local/share/com.ticklab.biopass
```

The config can be copied back only if the upstream version supports the schema you wrote. If you edited settings in biopass-rs after migration, review the YAML manually before reusing it with upstream.
