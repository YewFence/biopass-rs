# biopass-rs - Unofficial Rust Rewrite of Upstream Biopass

[简体中文](README.zh-CN.md) | English

<p align="center">
    <img src="https://public-r2.ticklab.site/media/tc1oN21KXhMM1B2jOecRhk=" alt="biopass logo" width="120" />
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

<h2 align="center">biopass-rs</h2>
<p align="center"><b>An unofficial Rust rewrite of upstream <a href="https://github.com/TickLabVN/biopass">biopass</a></b></p>
<p align="center">A fast, secure, and privacy-focused biometric recognition module for Linux desktops supporting face and fingerprint.</p>

> **Note**: biopass-rs is a personal, unofficial Rust rewrite of upstream [biopass](https://github.com/TickLabVN/biopass), developed by [@phucvinh57](https://github.com/phucvinh57) and [@thaitran24](https://github.com/thaitran24) at TickLab. The original C++ implementation has been fully replaced with Rust in this repository, and biopass-rs is maintained on a best-effort basis. For the official project, please visit the [upstream biopass repository](https://github.com/TickLabVN/biopass).

---

## Why biopass-rs?

[biopass](https://github.com/TickLabVN/biopass) was developed by TickLab to fill this gap, providing a fast, secure, and modern biometric suite that goes beyond just face ID. biopass-rs is my personal take on the project — translating the C++ implementation into safer and clearer Rust.

## Comparison with upstream biopass

| Feature | [Biopass](https://github.com/TickLabVN/biopass) | [Biopass-rs](https://github.com/YewFence/biopass-rs) |
| :--- | :--- | :--- |
| **AI Model Installation** | Shell script | Native Rust code |
| **Anti-Spoofing Config Structure** | Flat array, ambiguous `ai` and `ir` switch state | Refactored into separate `ai` and `ir` modules for clearer configuration |
| **Anti-Spoofing Retry** | Feature explicitly removed [#94](https://github.com/TickLabVN/biopass/pull/94) | AI and IR anti-spoofing checks support independent retry configuration |
| **Camera Handling** | None | Added image auto-optimization option |
| **IR Camera Capture Frame Quality Detection** | Under optimization, see [#116](https://github.com/TickLabVN/biopass/issues/116) | Automatically skips dark frames |
| **Image Processing Path** | GUI uses browser API for image handling, PAM module during authentication uses OpenCV, [#114](https://github.com/TickLabVN/biopass/issues/114) | Both GUI and PAM module use Rust's jpeg crate for image processing, ensuring consistent image quality |
| **`helper` CLI** | `auth` and `crop-face` commands | Added new subcommands: `config` (config management tree), `install`, `model-download`, `capture-face`, `preview-session`, `clean`, and `completion`; a global `--username` flag with automatic lookup from environment variables |

## Installation

- Download prebuilt packages from the [biopass-rs releases](https://github.com/YewFence/biopass-rs/releases). Debian and RPM packages are published there when available.
- System sign-in setup uses distro-managed PAM configuration when available (for example `pam-auth-update` on Debian/Ubuntu): [docs/PAM.md](docs/PAM.md)
- Migrating from upstream biopass requires both per-user config/data migration and a PAM review so the upstream and biopass-rs PAM modules are not enabled for the same service: [docs/upstream-migration.md](docs/upstream-migration.md)
- Interactive `polkit` authentication setup: [docs/Polkit.md](docs/Polkit.md)
- [IR camera setup guide](docs/IR%20camera.md)
- [`biopass-rs-helper` CLI reference](docs/biopass-rs-helper.md) — authentication, face capture, model install, and shell completion.

## Features

- [x] Authentication: User can register multiple biometrics for authentication. Authentication methods can be executed in parallel or sequentially.
    - [x] Face:
      - [x] Recognition
      - [x] Anti-spoofing
        - [x] With AI model
          - [x] Configurable retry
        - [x] With IR camera
          - [x] Configurable retry
    - [x] Fingerprint

Feel free to request new features or report bugs by opening an issue. For contributing, please read [CONTRIBUTING.md](docs/contributing.md).

## References

Models used in this project (sourced from the upstream project):
- Face Recognition: **[EdgeFace](https://github.com/otroshi/edgeface)**
- Face Detection: **[YOLO-Face](https://github.com/akanametov/yolo-face)**

## Credits

biopass-rs is an unofficial Rust rewrite of upstream [biopass](https://github.com/TickLabVN/biopass).

- **Original design and architecture**: [@phucvinh57](https://github.com/phucvinh57) and [@thaitran24](https://github.com/thaitran24) at TickLab
- **AI models**: EdgeFace and YOLO-Face, as used in upstream biopass
- **C++ → Rust translation**: Maintained on a best-effort basis; updates may lag behind the upstream project

Special thanks to the TickLab team for creating biopass and releasing it as open source. Without their original work, biopass-rs would not exist.

If you find biopass-rs useful, please consider supporting the [upstream biopass project](https://github.com/TickLabVN/biopass) first.
