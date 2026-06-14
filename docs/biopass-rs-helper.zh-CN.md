# `biopass-rs-helper` CLI 参考

简体中文 | [English](biopass-rs-helper.md)

`biopass-rs-helper` 是桌面 GUI 和 PAM 模块用于执行认证、捕获和裁剪面部图像、安装 AI 模型等操作的低级命令行工具。它也是脚本和调试的主要入口点。

使用发行版包时，该二进制文件安装到 `/usr/bin/biopass-rs-helper`。从此仓库进行开发时，通过以下方式运行：

```bash
mise run helper
```

…这会构建 crate 并调用二进制文件。附加 `--` 以转发额外的参数：

```bash
mise run helper -- auth --service sudo
```

## 概要

```text
BioPass 认证 helper

用法：biopass-rs-helper [选项] <命令>

选项：
  -u, --username <USERNAME>  目标用户名。默认为当前用户
                             （SUDO_USER → USER → USERNAME → LOGNAME）。对于不针对特定用户
                             的命令（install、crop-face、completion）会被忽略。
  -c, --config <PATH>        覆盖配置文件路径。为 helper 的其余部分设置 BIOPASS_CONFIG。
                             便于在不触碰真实配置的情况下进行开发和测试。
  -d, --data-dir <DIR>       覆盖数据目录（faces / debugs）。为 helper 的其余部分设置
                             BIOPASS_DATA_DIR。
  -h, --help                 打印此帮助消息并退出

子命令：
  auth             认证用户
  config           管理用户配置文件
  install          安装模型并运行设置
  model-download   仅下载模型
  crop-face        从图像中裁剪面部
  capture-face     从相机捕获面部
  preview-session  启动交互式预览会话
  completion       生成 shell 补全脚本
  clean            移除失败的人脸认证尝试产生的调试帧缓存
```

`--username`、`--config` 和 `--data-dir` 是**全局**标志：它们可以出现在子命令**之前或之后**。例如，PAM 模块以 `biopass-rs-helper --username <user> auth --service <service>` 的方式调用。

## `auth`

针对 biopass-rs 认证用户。这是 PAM 模块在系统登录期间调用的子命令。

```bash
biopass-rs-helper [--username <USERNAME>] auth --service <SERVICE>
```

| 标志 | 必需 | 描述 |
| :----------- | :------- | :------------------------------------------------------------------------------------------- |
| `--service`  | 是 | PAM 服务名称（例如 `sudo`、`login`、`gdm-password`）。用于查询用户的 `ignored_services` 列表。 |
| `--username` | 否 | 目标用户（全局标志）。如果省略，helper 会尝试从 `SUDO_USER`、`USER`、`USERNAME`、然后 `LOGNAME` 推断。 |

### 退出代码

| 代码 | 含义 |
| :--- | :------ |
| `0`  | 认证成功。 |
| `1`  | 认证失败，或发生内部错误。 |
| `2`  | biopass-rs 对此用户无事可做（无配置、无启用的方法，或服务被忽略）。PAM 应穿透到下一个模块。 |

### 示例

```bash
# 以当前用户测试 sudo 路径
biopass-rs-helper auth --service sudo

# 显式认证特定用户
biopass-rs-helper --username alice auth --service login
```

## `config`

管理用户的 `~/.config/biopass-rs/config.yaml`。配置格式随时间演变；此子命令树将引导、重置和迁移操作作为独立的动作暴露出来。

```bash
biopass-rs-helper [--username <USERNAME>] config <动作>
```

| 动作 | 描述 |
| :-------- | :---------- |
| `init`    | 确保配置文件存在。如果不存在，则写入内置默认值。（biopass-rs 不会自动导入上游 `biopass` 配置——其 schema 每个版本都在变；见[从上游 biopass 迁移](upstream-migration.zh-CN.md)。） |
| `reset`   | 将配置文件恢复为内置默认值。 |
| `migrate` | 将现有配置带到当前模式（例如 `anti_spoofing.ai` → `anti_spoofing.rgb`）。 |

### `config init`

```bash
biopass-rs-helper [--username <USERNAME>] config init [--force]
```

| 标志 | 描述 |
| :-------- | :---------- |
| `--force` | 覆盖已有的配置文件，而不是保持不动。 |

### `config migrate`

就地迁移配置模式。这是跨模式变更升级 biopass-rs 后运行的命令。

```bash
biopass-rs-helper --username <USERNAME> config migrate
```

此动作仅将当前 biopass-rs 配置路径 `~/.config/biopass-rs/config.yaml` 重写为 biopass-rs 的当前 schema。它不会导入上游配置、移动任何数据目录、编辑 PAM 或禁用上游 biopass PAM 模块。

如果用户不存在或迁移失败，则以非零状态退出。`install` 子命令的其中一个步骤就是为**当前**用户运行 `config init`。有关完整迁移流程，请参阅[从上游 biopass 迁移](upstream-migration.zh-CN.md)。

## `install`

发行版包的安装后脚本使用的一次性设置。它按顺序运行四个步骤：

1. `ldconfig` 以刷新动态链接器缓存（以便可以定位 PAM 模块）。
2. **`config init`** — 如果当前用户没有配置，则写入默认配置（**不会**导入上游配置）。
3. **复制已注册人脸** — 将上游 `~/.local/share/com.ticklab.biopass/faces` 中已注册的人脸图片复制到 biopass-rs 数据目录（非破坏性）。
4. `download-models` 将 AI 模型（EdgeFace 识别、YOLO-Face 检测）获取到当前用户的 biopass-rs 数据目录。

```bash
biopass-rs-helper install
```

`ldconfig` 的警告是非致命的，复制人脸也不会导致运行失败；`config init` 和模型下载都会决定退出代码。`install` 仅对**当前**用户（通过 `SUDO_USER`/`USER`/… 解析）操作；它不再遍历每个系统用户。

## `model-download`

将 AI 模型（EdgeFace 识别、YOLO-Face 检测）下载到当前用户的 biopass-rs 数据目录，而不运行配置引导或 `ldconfig`。当 `install` 已运行但模型缺失或需要重新获取时（例如重置数据目录后）使用此命令。

```bash
biopass-rs-helper model-download
```

当人脸认证因模型文件缺失而失败时，认证路径会记录一条指向此处的错误。

## `crop-face`

检测 JPEG/PNG 文件中最大的面部并写入裁剪、重新编码的 JPEG。用于准备训练数据或在静止图像上预览检测器。

```bash
biopass-rs-helper crop-face \
  --input  path/to/photo.jpg \
  --output path/to/cropped.jpg \
  --model  /usr/share/biopass-rs/models/yolo-face.onnx \
  [--quality 90]
```

| 标志 | 必需 | 默认值 | 描述 |
| :---------- | :------- | :------ | :---------- |
| `--input`   | 是 |         | 源图像路径。 |
| `--output`  | 是 |         | 写入裁剪的 JPEG 的路径。 |
| `--model`   | 是 |         | YOLO-Face 检测模型的路径。 |
| `--quality` | 否 | `90`    | JPEG 质量，1–100。 |

如果输入中未检测到面部，则以代码 `2` 退出，以便调用者可以区分"无面部"和通用错误。

## `capture-face`

从相机捕获单帧，检测最大的面部，并将裁剪写入磁盘。将 `crop-face` 与 V4L2 捕获步骤结合。

```bash
biopass-rs-helper capture-face \
  [--camera /dev/video0] \
  --output  path/to/captured.jpg \
  --model   /usr/share/biopass-rs/models/yolo-face.onnx \
  [--quality 90]
```

| 标志 | 必需 | 默认值 | 描述 |
| :---------- | :------- | :------ | :---------- |
| `--camera`  | 否 |         | V4L2 设备路径。如果省略，使用第一个可用的相机。 |
| `--output`  | 是 |         | 写入裁剪的 JPEG 的路径。 |
| `--model`   | 是 |         | YOLO-Face 检测模型的路径。 |
| `--quality` | 否 | `90`    | JPEG 质量，1–100。 |

遵守目标用户面部配置中的 `auto_optimize_camera` 设置（参见 `src/bin/biopass_rs_helper/utils.rs` 中的 `helper_auto_optimize_camera`）。如果未检测到面部，则以代码 `2` 退出。

## `preview-session`

为桌面预览窗口启动长期交互式会话。helper 在 **stdin** 上读取换行分隔的命令，并将帧化响应写入 **stdout**；GUI 端驱动协议。

```bash
biopass-rs-helper preview-session \
  [--camera /dev/video0] \
  [--model  /usr/share/biopass-rs/models/yolo-face.onnx] \
  [--quality 70]
```

| 标志 | 必需 | 默认值 | 描述 |
| :---------- | :------- | :------ | :---------- |
| `--camera`  | 否 |         | V4L2 设备路径。 |
| `--model`   | 否 |         | YOLO-Face 检测模型的路径。如果省略，`CAPTURE` 命令将失败并显示 `ERR detection model not loaded`（但 `FRAME` 仍然有效）。 |
| `--quality` | 否 | `70`    | JPEG 质量，1–100。 |

### 协议

会话开始时发出 `READY\n`。然后在 stdin 上循环：

| 命令 | 响应 | 描述 |
| :------------ | :---------------------------------------------------- | :---------- |
| `FRAME`       | `OK <bytes>\n<jpeg bytes>` 或 `ERR <message>\n`       | 捕获一帧并流式传输原始 JPEG 字节（无面部检测）。 |
| `CAPTURE <p>` | `OK\n` / `NO_FACE\n` / `ERR <message>\n`              | 检测最大的面部并将裁剪写入路径 `<p>`。 |
| `QUIT`        | （会话以退出代码 0 结束）                       | 优雅关闭。 |

任何其他输入都以 `ERR unknown command\n` 回答。如果 stdin 意外关闭或写入失败，会话将以非零代码退出。

### 典型用法

桌面前端将此 helper 作为子进程生成并将命令管道化到其中；几乎没有理由手动驱动它。如果您这样做，请使用 `printf 'FRAME\nCAPTURE /tmp/face.jpg\nQUIT\n' | biopass-rs-helper preview-session --model /path/to/yolo.onnx`。

## `completion`

生成 shell 补全脚本并将其打印到 stdout。与 `eval` 配对以在当前 shell 中启用 tab 补全。

```bash
biopass-rs-helper completion bash > /etc/bash_completion.d/biopass-rs-helper
biopass-rs-helper completion zsh  > "${fpath[1]}/_biopass-rs-helper"
biopass-rs-helper completion fish > ~/.config/fish/completions/biopass-rs-helper.fish
biopass-rs-helper completion powershell | Out-String | Invoke-Expression
```

| 参数 | 描述 |
| :------- | :---------- |
| `bash`   | Bash 补全脚本。 |
| `zsh`    | Zsh 补全脚本。 |
| `fish`   | Fish 补全脚本。 |
| `powershell` | PowerShell 补全脚本。 |
| `elvish` | Elvish 补全脚本。 |

## `clean`

移除失败的人脸认证尝试产生的调试帧缓存。启用调试模式时，biopass-rs 会将原始帧和诊断裁切保存到用户的 `debugs` 目录下；此子命令清空它们，并报告删除的文件数量和释放的空间。

```bash
biopass-rs-helper [--username <USERNAME>] clean
```

对目标用户的数据目录（遵循 `--data-dir` / `BIOPASS_DATA_DIR`）操作。

## 环境变量

几个环境变量影响 `biopass-rs-helper` 行为：

| 变量 | 使用者 | 目的 |
| :------------- | :--------------------------------- | :------ |
| `SUDO_USER`    | `auth`、`capture-face`             | 当未给出 `--username` 时，这是第一个被查询的变量。让 `sudo` 调用的 auth 解析为调用用户。 |
| `USER`         | `auth`、`capture-face`             | `SUDO_USER` 之后的回退。 |
| `USERNAME`     | `auth`、`capture-face`             | `USER` 之后的回退。 |
| `LOGNAME`      | `auth`                             | 用户查找的最后回退。 |
| `BIOPASS_CONFIG`   | 所有感知用户的子命令 | 覆盖配置文件路径（同 `--config`）。 |
| `BIOPASS_DATA_DIR` | 所有感知用户的子命令 | 覆盖存放 faces / debugs / models 的数据目录（同 `--data-dir`）。 |

详细的调试日志由配置文件中的 `strategy.debug` 字段控制，而不是环境变量。请参阅[开发者文档](contributing.zh-CN.md)。

## 另请参阅

- [PAM 设置](PAM.zh-CN.md) — `biopass-rs-helper auth` 如何连接到系统登录。
- [Polkit 设置](Polkit.zh-CN.md) — 用于 polkit 认证流程。
- [贡献](contributing.zh-CN.md) — 架构概述，包括桌面 GUI 如何与 `preview-session` 交互。
