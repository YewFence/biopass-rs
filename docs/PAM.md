# PAM Setup Guide

[简体中文](PAM.zh-CN.md) | English

If you are migrating from upstream Biopass, read [Migrating from upstream Biopass](upstream-migration.md) first. The upstream PAM module and `libbiopass_rs_pam.so` should not both be enabled for the same PAM service.

## Before You Start

**⚠️ Critical Warning**: Incorrect PAM configuration can lock you out of your system. Always keep a root terminal open and test authentication in a separate terminal before closing the root session.

### Check for Upstream Biopass

Before enabling biopass-rs, check if upstream Biopass is installed and active:

```bash
# Check if upstream Biopass package is installed
dpkg -l | grep biopass          # Debian/Ubuntu
rpm -qa | grep biopass          # Fedora/RHEL
pacman -Q | grep biopass        # Arch Linux

# Check for upstream PAM module
ls /usr/lib/security/pam_biopass.so 2>/dev/null || \
ls /lib/security/pam_biopass.so 2>/dev/null || \
echo "Upstream PAM module not found"

# Check which PAM services reference Biopass
grep -r "pam_biopass\|biopass" /etc/pam.d/ 2>/dev/null
grep -r "biopass" /usr/share/pam-configs/ 2>/dev/null  # Debian/Ubuntu
```

If upstream Biopass is present, see the [Migration section](#migrating-from-upstream-biopass) below.

## Debian/Ubuntu

1. Verify the Biopass PAM profile exists:
    ```bash
    ls /usr/share/pam-configs/biopass-rs
    ```

2. Enable the Biopass PAM profile:
    ```bash
    sudo pam-auth-update
    ```

3. In the dialog:
   - **Enable** `Biopass` from `biopass-rs`
   - **Disable** any upstream Biopass profile if present
   - **Disable** `Fingerprint authentication` if you enabled fingerprint in Biopass (to avoid conflicts with `pam_fprintd`)

4. Verify the PAM configuration was applied:
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```
    
    You should see lines like:
    ```
    /etc/pam.d/common-auth:auth	[success=2 default=ignore]	libbiopass_rs_pam.so
    ```

5. Test in a new terminal (keep the root terminal open):
    ```bash
    sudo -k
    sudo true
    ```
    
    If authentication fails, switch back to the root terminal and run `sudo pam-auth-update` to revert.

## Fedora/RHEL

Fedora-based distributions (Fedora, RHEL, CentOS, Rocky, Alma) use `authselect` instead of `pam-auth-update`.

### Check Current Profile

```bash
# Check the active authselect profile
sudo authselect current

# List available profiles
sudo authselect list
```

### Option 1: Using Custom Profile (Recommended)

1. Create a custom profile based on the current one:
    ```bash
    # If using sssd profile
    sudo authselect create-profile biopass-custom -b sssd
    
    # If using minimal profile
    sudo authselect create-profile biopass-custom -b minimal
    ```

2. Edit the custom profile to add biopass-rs:
    ```bash
    sudo vi /etc/authselect/custom/biopass-custom/system-auth
    ```
    
    Add this line before the `pam_unix.so` auth line:
    ```pam
    auth        sufficient    libbiopass_rs_pam.so
    ```
    
    If upstream Biopass is present, remove or comment out its line:
    ```pam
    # auth        sufficient    pam_biopass.so     # Commented out - using biopass-rs instead
    ```

3. Apply the custom profile:
    ```bash
    sudo authselect select custom/biopass-custom --force
    ```

4. Verify the changes:
    ```bash
    grep -r "biopass" /etc/pam.d/
    cat /etc/pam.d/system-auth
    ```

### Option 2: Direct PAM File Edit (Advanced)

If you prefer to edit PAM files directly without authselect:

1. **Disable authselect** (this makes PAM files editable):
    ```bash
    sudo authselect opt-out
    ```

2. Edit `/etc/pam.d/system-auth`:
    ```bash
    sudo vi /etc/pam.d/system-auth
    ```
    
    Add before `pam_unix.so`:
    ```pam
    auth        sufficient    libbiopass_rs_pam.so
    auth        [success=1 default=ignore]    pam_unix.so nullok
    auth        requisite     pam_deny.so
    ```

3. **Warning**: When authselect is disabled, you must manually maintain PAM configuration. System updates will not automatically update PAM files.

### Test Configuration

Keep a root terminal open and test in a new terminal:
```bash
sudo -k
sudo true
```

If authentication fails, return to the root terminal and revert:
```bash
# If using authselect
sudo authselect select sssd --force

# If you opted out of authselect, restore backup
sudo cp /etc/pam.d/system-auth.bak /etc/pam.d/system-auth
```

## Arch Linux

Arch Linux does not use `pam-auth-update` or `authselect` by default. Configure PAM manually.

**Keep a root terminal open while testing.** Incorrect PAM configuration can lock you out.

1. Verify the PAM module is installed:
    ```bash
    ls /usr/lib/security/libbiopass_rs_pam.so
    ```

2. Back up the PAM configuration:
    ```bash
    sudo cp /etc/pam.d/system-auth /etc/pam.d/system-auth.bak
    ```

3. Check for and remove upstream Biopass module:
    ```bash
    # Check if upstream module exists
    grep "pam_biopass" /etc/pam.d/system-auth
    
    # If found, remove or comment it out
    sudo vi /etc/pam.d/system-auth
    ```

4. Edit the PAM service:
    ```bash
    sudoedit /etc/pam.d/system-auth
    ```
    
    Insert biopass-rs before the existing `pam_unix.so` auth rule:
    ```pam
    auth      sufficient  libbiopass_rs_pam.so
    auth      [success=1 default=ignore]  pam_unix.so nullok
    auth      requisite   pam_deny.so
    ```

5. Test in a new terminal before closing the root terminal:
    ```bash
    sudo -k
    sudo true
    ```
    
    If it fails, restore the backup in the root terminal:
    ```bash
    sudo cp /etc/pam.d/system-auth.bak /etc/pam.d/system-auth
    ```

## Migrating from Upstream Biopass

If you have upstream Biopass installed, you need to disable or remove it to avoid conflicts.

### Option 1: Disable Upstream PAM Module Only

Keep the upstream package installed but disable its PAM module:

**Debian/Ubuntu:**
```bash
sudo pam-auth-update
# Disable the upstream Biopass option, enable biopass-rs
```

**Fedora/RHEL:**
```bash
# Edit the custom profile or system-auth file
sudo vi /etc/pam.d/system-auth
# Comment out or remove the upstream pam_biopass.so line
# Add libbiopass_rs_pam.so line
```

**Arch Linux:**
```bash
sudo vi /etc/pam.d/system-auth
# Comment out or remove the upstream pam_biopass.so line
# Add libbiopass_rs_pam.so line
```

### Option 2: Completely Remove Upstream Biopass

If you want to fully switch to biopass-rs, uninstall the upstream package:

**Debian/Ubuntu:**
```bash
# Find the package name
dpkg -l | grep biopass

# Remove the package (replace with actual package name)
sudo apt remove biopass
sudo apt autoremove

# Verify removal
dpkg -l | grep biopass
ls /usr/lib/security/pam_biopass.so 2>/dev/null
```

**Fedora/RHEL:**
```bash
# Find the package name
rpm -qa | grep biopass

# Remove the package
sudo dnf remove biopass  # or sudo yum remove biopass

# Verify removal
rpm -qa | grep biopass
ls /usr/lib64/security/pam_biopass.so 2>/dev/null
```

**Arch Linux:**
```bash
# Find the package name
pacman -Q | grep biopass

# Remove the package
sudo pacman -R biopass

# Verify removal
pacman -Q | grep biopass
ls /usr/lib/security/pam_biopass.so 2>/dev/null
```

### After Removing Upstream

1. Verify no upstream PAM references remain:
    ```bash
    grep -r "pam_biopass" /etc/pam.d/
    ```

2. Enable biopass-rs following the instructions for your distribution above.

3. Verify biopass-rs is active:
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```

## Troubleshooting

### Authentication Not Triggering

If Biopass doesn't activate during authentication:

1. Check if the PAM module is loaded:
    ```bash
    grep -r "libbiopass_rs_pam.so" /etc/pam.d/
    ```

2. Check PAM module file exists:
    ```bash
    ls -l /usr/lib/security/libbiopass_rs_pam.so    # Most distros
    ls -l /lib/security/libbiopass_rs_pam.so        # Some Debian-based
    ls -l /usr/lib64/security/libbiopass_rs_pam.so  # Some RHEL-based
    ```

3. Check Biopass configuration:
    ```bash
    ls ~/.config/biopass-rs/config.yaml
    cat ~/.config/biopass-rs/config.yaml | grep -A5 "enabled_methods"
    ```

4. Enable debug mode and check logs:
    ```bash
    # Enable debug in config
    vi ~/.config/biopass-rs/config.yaml
    # Set debug: true
    
    # Try authentication and check system logs
    sudo journalctl -f | grep biopass
    ```

### Camera Permission Issues

If authentication fails due to camera access:

```bash
# Check camera permissions
ls -l /dev/video*

# Add user to video group
sudo usermod -aG video $USER

# For systemd services (like polkit), see docs/Polkit.md
```

### Conflicts with pam_fprintd

If you enabled fingerprint in Biopass, disable `pam_fprintd`:

**Debian/Ubuntu:**
```bash
sudo pam-auth-update
# Uncheck "Fingerprint authentication"
```

**Fedora/RHEL/Arch:**
```bash
# Remove or comment pam_fprintd.so lines from /etc/pam.d/system-auth
sudo vi /etc/pam.d/system-auth
```
