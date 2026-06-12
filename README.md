# Biopass-rs - An alternative to Howdy

<p align="center">
    <img src="https://public-r2.ticklab.site/media/tc1oN21KXhMM1B2jOecRhk=" alt="Biopass Logo" width="120" />
</p>

<p align="center">
    <a href="https://github.com/YewFence/biopass-rs/releases/latest">
        <img src="https://img.shields.io/github/v/release/YewFence/biopass-rs?label=Last%20Release&style=flat-square" alt="Latest release" />
    </a>
    <a href="https://github.com/YewFence/biopass-rs/stargazers">
        <img src="https://img.shields.io/github/stars/YewFence/biopass-rs?style=flat-square" alt="GitHub stars" />
    </a>
    <a href="https://github.com/YewFence/biopass-rs/issues">
        <img src="https://img.shields.io/github/issues/YewFence/biopass-rs?style=flat-square" alt="Open Issues" />
    </a>
</p>

<h2 align="center">Biopass-rs</h2>
<p align="center"><b>An unofficial Rust rewrite of <a href="https://github.com/TickLabVN/biopass">Biopass</a> for Linux</b></p>
<p align="center">A fast, secure, and privacy-focused biometric recognition module for Linux desktops supporting face and fingerprint.</p>

> **Note**: This is a personal, unofficial Rust rewrite of the original [Biopass](https://github.com/TickLabVN/biopass) project developed by [@phucvinh57](https://github.com/phucvinh57) and [@thaitran24](https://github.com/thaitran24) at TickLab. The original C++ implementation has been fully replaced with Rust, and the project is maintained on a best-effort basis. For the official version, please visit the [upstream repository](https://github.com/TickLabVN/biopass).

---

## Why Biopass-rs?

While Windows Hello provides a seamless multi-modal biometric experience (Face, Fingerprint, PIN) on Windows 11, Linux has historically lacked a modern, unified equivalent. The most well-known project in this space, [Howdy](https://github.com/boltgolt/howdy), focuses exclusively on facial recognition and has not seen significant updates in recent years.

[Biopass](https://github.com/TickLabVN/biopass) was developed by TickLab to fill this gap, providing a fast, secure, and modern biometric suite that goes beyond just face ID. This Rust rewrite is my personal take on the project — translating the C++ implementation into idiomatic Rust for educational purposes and because I enjoy writing Rust in my spare time.

## Comparison with Upstream

| Area | Improvement | Details |
| :--- | :--- | :--- |
| **AI model installation** | Pure Rust | Model download and install logic migrated from shell scripts to native Rust code |
| **Anti-spoofing config** | Modular structure | Refactored into separate `ai` and `ir` modules for clearer configuration |
| **Retry behavior** | Independent controls | AI and IR anti-spoofing checks now support separate retry configuration |
| **Camera handling** | Quality optimization | Added image quality controls, dark-frame skipping for V4L2 GREY IR cameras, and an auto-optimize option |
| **`biopass-rs-helper` ergonomics** | Expanded CLI surface | New subcommands beyond the upstream `auth` and `crop-face`: `migrate`, `install`, `capture-face`, `preview-session`, and `completion`; the `auth` subcommand's `--username` argument is now optional and falls back to environment variable lookup |

## Installation

- Please refer to the [upstream Biopass releases](https://github.com/TickLabVN/biopass/releases) for prebuilt Debian and RPM packages, or the [AUR package](https://aur.archlinux.org/packages/biopass-bin) for Arch-based distributions.
- System sign-in setup uses distro-managed PAM configuration when available (for example `pam-auth-update` on Debian/Ubuntu): [docs/PAM.md](docs/PAM.md)
- Migrating from upstream Biopass requires both per-user config/data migration and a PAM review so the upstream and Rust rewrite PAM modules are not enabled for the same service: [docs/upstream-migration.md](docs/upstream-migration.md)
- Interactive `polkit` authentication setup: [docs/Polkit.md](docs/Polkit.md)
- [IR camera setup guide](docs/IR%20camera.md)
- [`biopass-rs-helper` CLI reference](docs/biopass-rs-helper.md) — authentication, face capture, model install, and shell completion.

## Features

- [x] Authentication: User can register multiple biometrics for authentication. Authentication methods can be executed in parallel or sequentially.
    - [x] Face:
      - [x] Recognition
      - [x] Anti-spoofing
        - [x] With AI model
        - [x] With IR camera
    - [x] Fingerprint
- [ ] Local AI model management: User can download, update, and delete AI models for supported authentication methods.

Feel free to request new features or report bugs by opening an issue. For contributing, please read [CONTRIBUTING.md](docs/contributing.md).

## References

Models used in this project (sourced from the upstream project):
- Face Recognition: **[EdgeFace](https://github.com/otroshi/edgeface)**
- Face Detection: **[YOLO-Face](https://github.com/akanametov/yolo-face)**

## Credits

This project is an unofficial Rust rewrite of [TickLabVN/biopass](https://github.com/TickLabVN/biopass).

- **Original design and architecture**: [@phucvinh57](https://github.com/phucvinh57) and [@thaitran24](https://github.com/thaitran24) at TickLab
- **AI models**: EdgeFace and YOLO-Face, as used in the upstream project
- **C++ → Rust translation**: Maintained on a best-effort basis; updates may lag behind the upstream project

Special thanks to the TickLab team for creating Biopass and releasing it as open source. Without their original work, this rewrite would not exist.

If you find Biopass useful, please consider supporting the [upstream project](https://github.com/TickLabVN/biopass) first.
