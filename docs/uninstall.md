# Uninstall and reinstall

English | [简体中文](uninstall.zh-CN.md)

## TL;DR

Uninstalling the biopass-rs package does **not** delete your user data — your config and enrolled faces stay in `~/.config/biopass-rs/` and `~/.local/share/biopass-rs/`. Remove those two paths manually to wipe everything. As long as you leave the data directory alone when reinstalling, your enrolled faces are kept.

## Where data lives

All of biopass-rs's user-level data lives under your home directory. None of it is owned by the package, so `apt` / `dnf` / `pacman` will not touch it on removal:

| What | Path |
| :--- | :--- |
| Config file | `~/.config/biopass-rs/config.yaml` |
| Data directory | `~/.local/share/biopass-rs/` |

Inside the data directory:

| Subdir | Contents | If you delete it |
| :--- | :--- | :--- |
| `faces/` | Your enrolled face images | You must re-enroll faces before auth works again |
| `models/` | ONNX inference models | `install` re-downloads them automatically |
| `debugs/` | Diagnostic frames saved from failed auths under debug mode | No impact; clear with `biopass-rs-helper clean` |

## Uninstalling

### 1. Disable biopass-rs in PAM first

Before removing the package, take biopass-rs out of the system sign-in chain — otherwise login can break on the missing PAM module after removal. The exact command depends on your distro (uncheck the `Biopass` profile in `pam-auth-update`, run the reverse `authselect` command, or hand-edit the `/etc/pam.d/` files you changed). See the [PAM setup guide](PAM.md) and reverse the "enable" step for your distro.

### 2. Remove the package

**Debian/Ubuntu:**

```bash
sudo apt remove biopass-rs
sudo apt autoremove   # clean up dependencies that are no longer needed
```

**Fedora/RHEL:**

```bash
sudo dnf remove biopass-rs
```

**Arch Linux:**

```bash
sudo pacman -R biopass-rs
```

This removes only the **packaged files** — `/usr/bin/biopass-rs-helper`, the `libbiopass_rs_pam.so` PAM module, the desktop app, etc. Your config and data directory are left in place.

### 3. (Optional) Wipe user data

If you're sure you're done and want a clean slate:

```bash
rm -rf ~/.local/share/biopass-rs
rm -f  ~/.config/biopass-rs/config.yaml
# the config dir is now empty; remove it too if you like:
rmdir ~/.config/biopass-rs 2>/dev/null || true
```

> **Multi-user machines**: every user who enrolled a face needs their own directory cleaned. As root, pass `--username <user>` to the helper, or delete the path under that user's home directly.

## Reinstalling

- **To keep your enrolled faces**: just reinstall, and **do not** delete the data directory. The package's post-install script runs `biopass-rs-helper install`, which re-downloads any missing models and ensures the default config exists; your `faces/` is untouched and works again immediately.
- **To start fresh**: wipe the data directory and config as described above, then reinstall. You'll need to re-capture and enroll your face afterward (`biopass-rs-helper capture-face`, or the enrollment flow in the desktop app).

> Reinstalling does **not** restore `faces/`. `install` only copies upstream biopass faces when that upstream data directory exists (a migration scenario); a plain reinstall has no such source, so once `faces/` is gone you must re-enroll.

## See also

- [PAM setup](PAM.md) — how to take biopass-rs out of the system sign-in chain before removing it.
- [`biopass-rs-helper` CLI reference](biopass-rs-helper.md) — `clean`, `install`, `config reset`, and friends.
- [Migrating from upstream biopass](upstream-migration.md)
