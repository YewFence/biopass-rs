# IR 相机指南

简体中文 | [English](IR%20camera.md)

biopass-rs 使用红外（IR）相机进行面部反欺骗，而不是仅依赖 RGB 反欺骗 AI 模型。通常将其配置为 Linux 视频设备路径，例如 `/dev/video2`。如果您的设备支持 IR 相机，可以通过配置 UI 启用此选项。

## 要求

- IR 传感器暴露为 `/dev/video*` 设备的 Linux 系统。
- 在 biopass-rs 中已设置好面部识别。
- 访问相机设备的权限。

biopass-rs 仅从配置的 IR 视频设备读取数据。它不管理笔记本电脑或摄像头的硬件 IR 发射器。

## 工作原理

IR 反欺骗管线是一个分层活体检查：

1. **LED / 曝光预热** — IR 相机最初可能返回暗帧，因此 biopass-rs 在采集前等待 `ir.warmup_delay_ms`（默认 300ms）。
2. **帧采集** — 连续采集 3 帧 IR 图像。
3. **人脸检测** — YOLO 模型（`yolov8n-face.onnx`）在每帧 IR 图像中定位人脸。
4. **RGB / IR 空间匹配** — 通过边框 IoU 将 IR 检测结果与 RGB 认证的人脸匹配。如果没有任何 IR 人脸与 RGB 人脸重叠，则跳过该 IR 帧。
5. **最小人脸尺寸检查** — 检测到的 IR 人脸必须至少占帧面积的 `ir.min_face_area_ratio`（默认 0.08）。过小 / 偏远的人脸被跳过，而不是被归类为 spoof。
6. **活体分类** — MobileNetV3 模型（`mobilenetv3_antispoof.onnx`）将每个被接受的人脸裁切分类为 **real** 或 **spoof**。由于模型期望 RGB 输入，单通道灰度 IR 数据被复制到 3 个颜色通道中。

当且仅当 SPOOF 类没有胜出、real 分数达到 `anti_spoofing.ai.model.threshold`、且 real 分数严格大于 spoof 分数时，该帧才被接受为真。

每次认证尝试，biopass-rs 收集 3 张可用的 IR 人脸裁切图，要求其中至少 2 张通过活体检查 — 严格的多数投票。检查是 fail-closed：任何失败（模型缺失、帧不可读、人脸过小、空间不匹配、分类器判定 spoof）都被视为 spoof。

当 IR 管线与 RGB AI 模型同时启用时，**两者都必须独立通过**才会授予认证。RGB AI 反欺骗任务与 IR 活体任务串行执行。

启用调试模式后，biopass-rs 会在 `~/.local/share/biopass-rs/<user>/debugs/` 下保存额外的 IR 诊断信息，包括：

- `ir_raw_frame.*.jpg` — 每张原始 IR 帧
- `ir_spoof.*.jpg` — 活体分类器拒绝的人脸裁切
- `ir_face_too_small.*.jpg` / `ir_face_mismatch.*.jpg` / `ir_no_face.*.jpg` — 失败原因标签

这些信息有助于区分真正的模型不匹配和由于距离、模糊或裁切尺寸不足导致的输入细节不足。

## 1. 查找 IR 相机设备

列出视频设备：

```bash
ls -l /dev/video*
```

如果 `v4l2-ctl` 可用，通常更容易用它识别正确的设备：

```bash
v4l2-ctl --list-devices
```

查找属于您的 IR 传感器的设备节点，例如 `/dev/video2`。

## 2. 在 biopass-rs 中启用它

打开 biopass-rs 桌面应用并进入面部设置。

在反欺骗部分：

1. 如果您也想使用 AI 反欺骗模型，请启用面部反欺骗。
2. 将 `IR Camera` 设置为正确的 `/dev/video*` 设备。
3. 保存您的配置。

如果您只想使用基于 IR 的反欺骗，选择 `IR Camera` 设备就足够了。

## 3. 如果 IR 发射器在 Linux 上保持关闭状态

在某些 Linux 系统上，IR 相机被检测到但 IR 灯发射器不会自动打开。在这种情况下，使用 [`linux-enable-ir-emitter`](https://github.com/EmixamPP/linux-enable-ir-emitter)。

安装它：

```bash
VERSION=6.1.2
DIST=linux-enable-ir-emitter-$VERSION-release.systemd.x86-64.tar.gz
wget https://github.com/EmixamPP/linux-enable-ir-emitter/releases/download/$VERSION/$DIST
sudo tar -C / --no-same-owner -m -h -vxzf $DIST
```

然后，配置您的 IR 发射器：

```bash
sudo linux-enable-ir-emitter configure
```

按照配置相机时打印的说明进行操作。成功触发 IR 发射器后，请运行此命令：

```bash
sudo systemctl enable --now linux-enable-ir-emitter
```

感谢 @notherealmarco 在这方面的帮助 https://github.com/TickLabVN/biopass/discussions/60#discussioncomment-16521628。
