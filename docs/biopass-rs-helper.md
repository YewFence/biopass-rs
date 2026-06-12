# `biopass-rs-helper` CLI Reference

`biopass-rs-helper` is the low-level command-line tool that the desktop GUI and the PAM module use to perform authentication, capture and crop face images, install AI models, and more. It is also the primary entry point for scripting and debugging.

The binary is installed to `/usr/bin/biopass-rs-helper` when using the distro package. When developing from this repository, run it via:

```bash
mise run helper
```

…which builds the crate and invokes the binary. Append `--` to forward extra arguments:

```bash
mise run helper -- auth --service sudo
```

## Synopsis

```text
Biopass authentication helper

Usage: biopass-rs-helper [OPTIONS] SUBCOMMAND

Options:
  -h, --help    Print this help message and exit

Subcommands:
  auth             Authenticate user
  migrate          Migrate user configuration
  install          Install models and run setup
  crop-face        Crop a face from an image
  capture-face     Capture face from camera
  preview-session  Start interactive preview session
  completion       Generate shell completion script
```

## `auth`

Authenticate a user against Biopass. This is the subcommand invoked by the PAM module during system sign-in.

```bash
biopass-rs-helper auth --service <SERVICE> [--username <USERNAME>]
```

| Flag         | Required | Description                                                                                  |
| :----------- | :------- | :------------------------------------------------------------------------------------------- |
| `--service`  | Yes      | PAM service name (for example `sudo`, `login`, `gdm-password`). Used to consult the user's `ignored_services` list. |
| `--username` | No       | Target user. If omitted, the helper tries to infer it from `SUDO_USER`, `USER`, `USERNAME`, then `LOGNAME`. |

### Exit codes

| Code | Meaning |
| :--- | :------ |
| `0`  | Authentication succeeded. |
| `1`  | Authentication failed, or an internal error occurred. |
| `2`  | Biopass has nothing to do for this user (no config, no enabled methods, or service is ignored). PAM should fall through to the next module. |

### Examples

```bash
# Test the sudo path as the current user
biopass-rs-helper auth --service sudo

# Authenticate a specific user explicitly
biopass-rs-helper auth --service login --username alice
```

## `migrate`

Run the configuration schema migration for one user. The configuration format has evolved over time; older layouts are upgraded in place.

```bash
biopass-rs-helper migrate --username <USERNAME>
```

| Flag         | Required | Description                |
| :----------- | :------- | :------------------------- |
| `--username` | Yes      | User whose config to migrate. |

This command only touches the current biopass-rs config path, `~/.config/biopass-rs/config.yaml`. It does not copy the upstream config from `~/.config/com.ticklab.biopass/config.yaml`, move the upstream data directory, edit PAM, or disable the upstream Biopass PAM module.

Exits with a non-zero status if the user does not exist or if the migration fails. The `install` subcommand runs migration across **all** users after copying upstream configs into the new path when the new config is absent. See [Migrating from upstream Biopass](upstream-migration.md) for the full migration flow.

## `install`

One-shot setup used by the post-install scripts of the distro package. It runs three steps in order:

1. `ldconfig` to refresh the dynamic linker cache (so the PAM module can be located).
2. `migrate-all` to copy upstream configs into the biopass-rs path when needed, move upstream user data directories when possible, and bring copied configs up to the current schema.
3. `download-models` to fetch the AI models (EdgeFace recognition, YOLO-Face detection) into the user's biopass-rs data directory.

```bash
biopass-rs-helper install
```

Warnings from the first two steps are non-fatal — only the model download step determines the final exit code.

## `crop-face`

Detect the largest face in a JPEG/PNG file and write a cropped, re-encoded JPEG. Useful for preparing training data or previewing the detector on a still image.

```bash
biopass-rs-helper crop-face \
  --input  path/to/photo.jpg \
  --output path/to/cropped.jpg \
  --model  /usr/share/biopass-rs/models/yolo-face.onnx \
  [--quality 90]
```

| Flag        | Required | Default | Description |
| :---------- | :------- | :------ | :---------- |
| `--input`   | Yes      |         | Path to the source image. |
| `--output`  | Yes      |         | Path to write the cropped JPEG. |
| `--model`   | Yes      |         | Path to the YOLO-Face detection model. |
| `--quality` | No       | `90`    | JPEG quality, 1–100. |

Exits with code `2` if no face is detected in the input, so callers can distinguish "no face" from generic errors.

## `capture-face`

Capture a single frame from a camera, detect the largest face, and write the crop to disk. Combines `crop-face` with a V4L2 capture step.

```bash
biopass-rs-helper capture-face \
  [--camera /dev/video0] \
  --output  path/to/captured.jpg \
  --model   /usr/share/biopass-rs/models/yolo-face.onnx \
  [--quality 90]
```

| Flag        | Required | Default | Description |
| :---------- | :------- | :------ | :---------- |
| `--camera`  | No       |         | V4L2 device path. If omitted, the first available camera is used. |
| `--output`  | Yes      |         | Path to write the cropped JPEG. |
| `--model`   | Yes      |         | Path to the YOLO-Face detection model. |
| `--quality` | No       | `90`    | JPEG quality, 1–100. |

Honors the `auto_optimize_camera` setting in the current user's face config (see `helper_auto_optimize_camera` in `biopass-rs-helper.rs`). Exits with code `2` if no face is detected.

## `preview-session`

Start a long-lived interactive session for the desktop preview window. The helper reads newline-delimited commands on **stdin** and writes framed responses to **stdout**; the GUI side drives the protocol.

```bash
biopass-rs-helper preview-session \
  [--camera /dev/video0] \
  [--model  /usr/share/biopass-rs/models/yolo-face.onnx] \
  [--quality 70]
```

| Flag        | Required | Default | Description |
| :---------- | :------- | :------ | :---------- |
| `--camera`  | No       |         | V4L2 device path. |
| `--model`   | No       |         | Path to the YOLO-Face detection model. If omitted, `CAPTURE` commands will fail with `ERR detection model not loaded` (but `FRAME` still works). |
| `--quality` | No       | `70`    | JPEG quality, 1–100. |

### Protocol

The session begins by emitting `READY\n`. It then loops over stdin:

| Command       | Response                                              | Description |
| :------------ | :---------------------------------------------------- | :---------- |
| `FRAME`       | `OK <bytes>\n<jpeg bytes>` or `ERR <message>\n`       | Capture one frame and stream the raw JPEG bytes (no face detection). |
| `CAPTURE <p>` | `OK\n` / `NO_FACE\n` / `ERR <message>\n`              | Detect the largest face and write the crop to path `<p>`. |
| `QUIT`        | (session ends with exit code 0)                       | Graceful shutdown. |

Any other input is answered with `ERR unknown command\n`. The session exits with a non-zero code if stdin closes unexpectedly or a write fails.

### Typical usage

The desktop frontend spawns this helper as a subprocess and pipes commands into it; there is little reason to drive it manually. If you do, use `printf 'FRAME\nCAPTURE /tmp/face.jpg\nQUIT\n' | biopass-rs-helper preview-session --model /path/to/yolo.onnx`.

## `completion`

Generate a shell completion script and print it to stdout. Pair with `eval` to enable tab-completion in the current shell.

```bash
biopass-rs-helper completion bash > /etc/bash_completion.d/biopass-rs-helper
biopass-rs-helper completion zsh  > "${fpath[1]}/_biopass-rs-helper"
biopass-rs-helper completion fish > ~/.config/fish/completions/biopass-rs-helper.fish
biopass-rs-helper completion powershell | Out-String | Invoke-Expression
```

| Argument | Description |
| :------- | :---------- |
| `bash`   | Bash completion script. |
| `zsh`    | Zsh completion script. |
| `fish`   | Fish completion script. |
| `powershell` | PowerShell completion script. |
| `elvish` | Elvish completion script. |

## Environment variables

Several environment variables influence `biopass-rs-helper` behavior:

| Variable       | Used by                            | Purpose |
| :------------- | :--------------------------------- | :------ |
| `SUDO_USER`    | `auth`, `capture-face`             | When `--username` is not given, this is the first variable consulted. Lets `sudo`-invoked auth resolve to the invoking user. |
| `USER`         | `auth`, `capture-face`             | Fallback after `SUDO_USER`. |
| `USERNAME`     | `auth`, `capture-face`             | Fallback after `USER`. |
| `LOGNAME`      | `auth`                             | Last-resort fallback for the user lookup. |
| `BIOPASS_DEBUG`| `auth` and friends (when supported)| Enables verbose logging output for debugging failures. See [the developer docs](contributing.md). |

## See also

- [PAM setup](PAM.md) — how `biopass-rs-helper auth` is wired into system sign-in.
- [Polkit setup](Polkit.md) — for the polkit authentication flow.
- [Contributing](contributing.md) — architecture overview, including how the desktop GUI talks to `preview-session`.
