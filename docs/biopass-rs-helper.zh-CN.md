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
Biopass 认证 helper

用法：biopass-rs-helper [选项] 子命令

选项：
  -h, --help    打印此帮助消息并退出

子命令：
  auth             认证用户
  migrate          迁移用户配置
  install          安装模型并运行设置
  crop-face        从图像中裁剪面部
  capture-face     从相机捕获面部
  preview-session  启动交互式预览会话
  completion       生成 shell 补全脚本
```

## `auth`

针对 biopass-rs 认证用户。这是 PAM 模块在系统登录期间调用的子命令。

```bash
biopass-rs-helper auth --service <SERVICE> [--username <USERNAME>]
```

| 标志 | 必需 | 描述 |
| :----------- | :------- | :------------------------------------------------------------------------------------------- |
| `--service`  | 是 | PAM 服务名称（例如 `sudo`、`login`、`gdm-password`）。用于查询用户的 `ignored_services` 列表。 |
| `--username` | 否 | 目标用户。如果省略，helper 会尝试从 `SUDO_USER`、`USER`、`USERNAME`、然后 `LOGNAME` 推断。 |

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
biopass-rs-helper auth --service login --username alice
```

## `migrate`

为一个用户运行配置模式迁移。配置格式随时间演变；旧布局会就地升级。

```bash
biopass-rs-helper migrate --username <USERNAME>
```

| 标志 | 必需 | 描述 |
| :----------- | :------- | :------------------------- |
| `--username` | 是 | 要迁移配置的用户。 |

此命令仅触及当前 biopass-rs 配置路径 `~/.config/biopass-rs/config.yaml`。它不会从 `~/.config/com.ticklab.biopass/config.yaml` 复制上游配置、移动上游数据目录、编辑 PAM 或禁用上游 biopass PAM 模块。

如果用户不存在或迁移失败，则以非零状态退出。当新配置不存在时，`install` 子命令在将上游配置复制到新路径后对**所有**用户运行迁移。有关完整迁移流程，请参阅[从上游 biopass 迁移](upstream-migration.zh-CN.md)。

## `install`

发行版包的安装后脚本使用的一次性设置。它按顺序运行三个步骤：

1. `ldconfig` 以刷新动态链接器缓存（以便可以定位 PAM 模块）。
2. `migrate-all` 在需要时将上游配置复制到 biopass-rs 路径，在可能时移动上游用户数据目录，并将复制的配置带到当前模式。
3. `download-models` 将 AI 模型（EdgeFace 识别、YOLO-Face 检测）获取到用户的 biopass-rs 数据目录。

```bash
biopass-rs-helper install
```

前两个步骤的警告是非致命的——只有模型下载步骤决定最终退出代码。

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

遵守当前用户面部配置中的 `auto_optimize_camera` 设置（参见 `biopass-rs-helper.rs` 中的 `helper_auto_optimize_camera`）。如果未检测到面部，则以代码 `2` 退出。

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

## 环境变量

几个环境变量影响 `biopass-rs-helper` 行为：

| 变量 | 使用者 | 目的 |
| :------------- | :--------------------------------- | :------ |
| `SUDO_USER`    | `auth`、`capture-face`             | 当未给出 `--username` 时，这是第一个被查询的变量。让 `sudo` 调用的 auth 解析为调用用户。 |
| `USER`         | `auth`、`capture-face`             | `SUDO_USER` 之后的回退。 |
| `USERNAME`     | `auth`、`capture-face`             | `USER` 之后的回退。 |
| `LOGNAME`      | `auth`                             | 用户查找的最后回退。 |
| `BIOPASS_DEBUG`| `auth` 和朋友（支持时）| 启用详细日志输出以调试失败。请参阅[开发者文档](contributing.zh-CN.md)。 |

## 另请参阅

- [PAM 设置](PAM.zh-CN.md) — `biopass-rs-helper auth` 如何连接到系统登录。
- [Polkit 设置](Polkit.zh-CN.md) — 用于 polkit 认证流程。
- [贡献](contributing.zh-CN.md) — 架构概述，包括桌面 GUI 如何与 `preview-session` 交互。
