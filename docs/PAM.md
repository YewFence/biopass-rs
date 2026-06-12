# PAM Setup Guide

[简体中文](PAM.zh-CN.md) | English

If you are migrating from upstream Biopass, read [Migrating from upstream Biopass](upstream-migration.md) first. The upstream PAM module and `libbiopass_rs_pam.so` should not both be enabled for the same PAM service.

## Debian/Ubuntu

1. Verify the Biopass PAM profile exists:
    ```bash
    ls /usr/share/pam-configs/biopass-rs
    ```
2. Enable the Biopass PAM profile.
    ```bash
    sudo pam-auth-update
    ```
3. Enable the `Biopass` option provided by `biopass-rs`. Disable any upstream Biopass profile if it is still present. If `Fingerprint authentication` is selected, please disable it if you have enabled fingerprint auth in Biopass.
4. Test in a new terminal:
    ```bash
    sudo -k
    sudo true
    ```

## Fedora/RHEL

Fedora-based OS (Fedora, RHEL, CentOS, Rocky, Alma ...) use `authselect`, not `pam-auth-update`.

Follow your distro's `authselect` workflow, keep only one Biopass PAM module active for each service, and test in a new terminal before closing the current root session.

## Arch Linux

Arch Linux does not use `pam-auth-update` nor `authselect` by default. Configure PAM manually.

Keep a root terminal open while testing. Incorrect PAM configuration can lock you out. If an upstream Biopass line is already present in the same service, remove or comment it before adding `libbiopass_rs_pam.so`.

1. Verify the PAM module is installed:
    ```bash
    ls /usr/lib/security/libbiopass_rs_pam.so
    ```
2. Edit the PAM service you want to protect, for example:
    ```bash
    sudoedit /etc/pam.d/system-auth
    ```
3. Insert Biopass before the existing `pam_unix.so` auth rule:
    ```pam
    auth sufficient libbiopass_rs_pam.so
    auth [success=1 default=ignore] pam_unix.so nullok
    auth requisite pam_deny.so
    ```
4. Test in a new terminal before closing the root terminal:
    ```bash
    sudo -k
    sudo true
    ```
