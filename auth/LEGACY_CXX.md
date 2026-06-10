# Legacy C++ auth sources

This file records the C++ and CMake files that remain under `auth/` after the
default auth product path moved to Rust and Cargo.

## Default product path

The default auth build path is `make build-auth`. It builds:

- `auth/rust/Cargo.toml`
- `auth/rust/pam/Cargo.toml`

The default Tauri bundle path in `app/src-tauri/tauri.conf.json` packages:

- `auth/rust/target/release/biopass-helper`
- `auth/rust/pam/target/release/libbiopass_pam.so`
- model download script from `app/src-tauri/scripts/download_models.sh`
- install scripts from `app/src-tauri/scripts`

`app/src-tauri/build.rs` delegates to `tauri_build::build()` and does not link
`stdc++`, `libbiopass_fingerprint.a`, or any project-owned C++ auth artifact.

## Remaining C++ and CMake files

These files are retained as historical references for behavior comparison while
the Rust auth path is validated on real hardware. They are not used by
`make build-auth`, the Rust PAM crate, the Rust helper, or Tauri packaging.

| Path group | Files | Status |
| --- | --- | --- |
| `auth/core` | `auth_config.cc`, `auth_config.h`, `auth_manager.cc`, `auth_manager.h`, `auth_method.h`, `CMakeLists.txt` | Historical reference for Rust config and orchestration parity |
| `auth/pam` | `helper.cc`, `pam.cc`, `CMakeLists.txt` | Historical reference for Rust helper and PAM parity |
| `auth/fingerprint` | `fingerprint_auth.cc`, `fingerprint_auth.h`, `fingerprint_ffi.cc`, `fingerprint_ffi.h`, `CMakeLists.txt` | Historical reference for fprintd D-Bus behavior parity |
| `auth/face` | `face_auth.cc`, `face_auth.h`, `image_utils.h`, `stb_impl.cc`, `CMakeLists.txt` | Historical reference for face auth behavior parity |
| `auth/face/common` | `camera_capture.cc`, `camera_capture.h`, `debug_image_io.cc`, `debug_image_io.h` | Historical reference for V4L2 capture and debug image behavior parity |
| `auth/face/detection` | `face_detection.cc`, `face_detection.h`, `utils.cc`, `utils.h`, `CMakeLists.txt` | Historical reference for YOLOv8 face detection parity |
| `auth/face/recognition` | `face_recognition.cc`, `face_recognition.h`, `CMakeLists.txt` | Historical reference for face embedding parity |
| `auth/face/antispoofing` | `antispoof_check.cc`, `antispoof_check.h`, `face_as.cc`, `face_as.h`, `ir_camera_as.cc`, `ir_camera_as.h`, `CMakeLists.txt` | Historical reference for AI and IR anti-spoofing parity |
| `auth/test/camera` | `main.cpp`, `CMakeLists.txt` | Legacy manual camera test fixture |
| `auth/test/face_engine` | `main.cpp`, `CMakeLists.txt` | Legacy manual face engine test fixture |

## Rust replacement evidence

The default Rust auth path now provides these replacements:

- PAM entry point: `auth/rust/pam`
- Helper command: `auth/rust/src/bin/biopass-helper.rs`
- Auth orchestration: `auth/rust/src/manager.rs`
- Config migration and reading: `auth/rust/src/config.rs`
- fprintd D-Bus auth path: `auth/rust/src/fingerprint_auth.rs`
- Tauri fprintd management path: `app/src-tauri/src/fingerprint_ffi.rs`
- V4L2 capture path: `auth/rust/src/camera.rs`
- Face detection path: `auth/rust/src/face_detection.rs`
- Face recognition path: `auth/rust/src/face_recognition.rs`
- AI anti-spoofing path: `auth/rust/src/face_antispoofing.rs`
- IR face presence path: `auth/rust/src/face_auth.rs`

## Verification record

Use these checks to prove the retained C++ files are not in the default product
path:

```sh
make -n build-auth
rg "auth/build|cmake|CMake|libbiopass|openpnp|stdc\+\+|biopass_fingerprint|libonnxruntime" \
  Makefile app/src-tauri/tauri.conf.json app/src-tauri/build.rs app/src-tauri/src auth/rust \
  -g'!*target*' -n
cargo test --manifest-path auth/rust/Cargo.toml
cargo test --manifest-path auth/rust/pam/Cargo.toml
cargo build --manifest-path auth/rust/Cargo.toml --release
cargo build --manifest-path auth/rust/pam/Cargo.toml --release
```

Hardware-dependent paths are isolated behind Rust modules and covered by unit
tests for parsers, state mapping, retry and timeout logic, frame decoding, and
model post-processing. Manual validation still requires real devices:

```sh
auth/rust/target/release/biopass-helper auth --username "$USER"
```

For fingerprint validation, enroll at least one finger through the application,
then run the helper command and confirm fprintd emits `VerifyStatus` and the
helper returns success for a matching finger.

For IR validation, configure `methods.face.anti_spoofing.ir_camera` with a
`/dev/video*` device that exposes `GREY`, enable anti-spoofing, then run the
helper command and confirm the Rust V4L2 path captures a `GREY` frame and the
Rust detector finds a face in the IR image.
