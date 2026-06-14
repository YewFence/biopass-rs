# `biopass-rs-helper` CLI Reference

[简体中文](biopass-rs-helper.zh-CN.md) | English

`biopass-rs-helper` is the low-level command-line tool that the desktop GUI and the PAM module use to perform authentication, capture and crop face images, install AI models, and more. It is also the primary entry point for scripting and debugging.

## Synopsis

```text
BioPass authentication helper

Usage: biopass-rs-helper [OPTIONS] <COMMAND>

Options:
  -u, --username <USERNAME>  Target username. Defaults to the current user
                             (SUDO_USER → USER → USERNAME → LOGNAME). Ignored by
                             commands that do not operate on a specific user
                             (install, crop-face, completion).
  -c, --config <PATH>        Override the config file path. Sets BIOPASS_CONFIG
                             for the rest of the helper. Useful for development
                             and testing without touching the real config.
  -d, --data-dir <DIR>       Override the data directory (faces / debugs). Sets
                             BIOPASS_DATA_DIR for the rest of the helper.
  -h, --help                 Print this help message and exit

Subcommands:
  auth             Authenticate user
  config           Manage the user's config file
  install          Install models and run setup
  model-download   Download models only
  crop-face        Crop a face from an image
  capture-face     Capture face from camera
  preview-session  Start interactive preview session
  completion       Generate shell completion script
  clean            Remove cached debug frames written by failed face-auth attempts
```

`--username`, `--config`, and `--data-dir` are **global** flags: they may appear before *or* after the subcommand. The PAM module, for example, invokes `biopass-rs-helper --username <user> auth --service <service>`.

## `auth`

Authenticate a user against biopass-rs. This is the subcommand invoked by the PAM module during system sign-in.

```bash
biopass-rs-helper [--username <USERNAME>] auth --service <SERVICE>
```

| Flag         | Required | Description                                                                                  |
| :----------- | :------- | :------------------------------------------------------------------------------------------- |
| `--service`  | Yes      | PAM service name (for example `sudo`, `login`, `gdm-password`). Used to consult the user's `ignored_services` list. |
| `--username` | No       | Target user (global flag). If omitted, the helper tries to infer it from `SUDO_USER`, `USER`, `USERNAME`, then `LOGNAME`. |

### Exit codes

| Code | Meaning |
| :--- | :------ |
| `0`  | Authentication succeeded. |
| `1`  | Authentication failed, or an internal error occurred. |
| `2`  | biopass-rs has nothing to do for this user (no config, no enabled methods, or service is ignored). PAM should fall through to the next module. |

### Examples

```bash
# Test the sudo path as the current user
biopass-rs-helper auth --service sudo

# Authenticate a specific user explicitly
biopass-rs-helper --username alice auth --service login
```

## `config`

Manage the user's `~/.config/biopass-rs/config.yaml`. The configuration format has evolved over time; this subcommand tree exposes the bootstrap, reset, and migration operations as separate actions.

```bash
biopass-rs-helper [--username <USERNAME>] config <ACTION>
```

| Action    | Description |
| :-------- | :---------- |
| `init`    | Ensure a config file exists. If none is present, write the built-in defaults. (biopass-rs does not auto-import the upstream `biopass` config — its schema drifts every release; see [Migrating from upstream biopass](upstream-migration.md).) |
| `reset`   | Restore the config file to its built-in defaults. |
| `migrate` | Bring the existing config up to the current schema (for example, `anti_spoofing.ai` → `anti_spoofing.rgb`). |

### `config init`

```bash
biopass-rs-helper [--username <USERNAME>] config init [--force]
```

| Flag      | Description |
| :-------- | :---------- |
| `--force` | Overwrite an existing config file instead of leaving it untouched. |

### `config migrate`

Migrate the configuration schema in place. This is what you run after upgrading biopass-rs across a schema change.

```bash
biopass-rs-helper --username <USERNAME> config migrate
```

This action only rewrites the current biopass-rs config path, `~/.config/biopass-rs/config.yaml`, to biopass-rs's current schema. It does not import the upstream config, move any data directory, edit PAM, or disable the upstream biopass PAM module.

Exits with a non-zero status if the user does not exist or if the migration fails. The `install` subcommand runs `config init` for the **current** user as one of its steps. See [Migrating from upstream biopass](upstream-migration.md) for the full migration flow.

## `install`

One-shot setup used by the post-install scripts of the distro package. It runs four steps in order:

1. `ldconfig` to refresh the dynamic linker cache (so the PAM module can be located).
2. **`config init`** — write a default config for the current user if none exists (it does **not** import the upstream config).
3. **Copy enrolled faces** — copy any enrolled face images from the upstream `~/.local/share/com.ticklab.biopass/faces` into the biopass-rs data directory (non-destructive).
4. `download-models` to fetch the AI models (EdgeFace recognition, YOLO-Face detection) into the current user's biopass-rs data directory.

```bash
biopass-rs-helper install
```

The `ldconfig` warning is non-fatal and face copying never fails the run; `config init` and the model download both determine the exit code. `install` operates on the **current** user only (resolved via `SUDO_USER`/`USER`/…); it no longer iterates over every system user.

## `model-download`

Download the AI models (EdgeFace recognition, YOLO-Face detection) into the current user's biopass-rs data directory, without running the config bootstrap or `ldconfig`. Use this when `install` already ran but the models are missing or need re-fetching (for example, after a data directory reset).

```bash
biopass-rs-helper model-download
```

When face authentication fails because a model file is missing, the auth path logs an error pointing here.

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

Honors the `auto_optimize_camera` setting in the target user's face config (see `helper_auto_optimize_camera` in `src/bin/biopass_rs_helper/utils.rs`). Exits with code `2` if no face is detected.

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

## `clean`

Remove the cached debug frames written by failed face-auth attempts. When debug mode is enabled, biopass-rs saves raw frames and diagnostic crops under the user's `debugs` directory; this subcommand clears them out and reports how many files were removed and how much space was freed.

```bash
biopass-rs-helper [--username <USERNAME>] clean
```

Operates on the target user's data directory (respecting `--data-dir` / `BIOPASS_DATA_DIR`).

## Environment variables

Several environment variables influence `biopass-rs-helper` behavior:

| Variable            | Used by                            | Purpose |
| :------------------ | :--------------------------------- | :------ |
| `SUDO_USER`         | `auth`, `capture-face`             | When `--username` is not given, this is the first variable consulted. Lets `sudo`-invoked auth resolve to the invoking user. |
| `USER`              | `auth`, `capture-face`             | Fallback after `SUDO_USER`. |
| `USERNAME`          | `auth`, `capture-face`             | Fallback after `USER`. |
| `LOGNAME`           | `auth`                             | Last-resort fallback for the user lookup. |
| `BIOPASS_CONFIG`    | all user-aware subcommands         | Override the config file path (same as `--config`). |
| `BIOPASS_DATA_DIR`  | all user-aware subcommands         | Override the data directory holding faces / debugs / models (same as `--data-dir`). |

Verbose debug logging is toggled by the `strategy.debug` field in the config file rather than an environment variable. See [the developer docs](contributing.md).

## See also

- [PAM setup](PAM.md) — how `biopass-rs-helper auth` is wired into system sign-in.
- [Polkit setup](Polkit.md) — for the polkit authentication flow.
- [Contributing](contributing.md) — architecture overview, including how the desktop GUI talks to `preview-session`.
