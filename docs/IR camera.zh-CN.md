# IR 相机指南

简体中文 | [English](IR%20camera.md)

Biopass 使用红外（IR）相机进行面部反欺骗，而不是仅依赖 RGB 反欺骗 AI 模型。通常将其配置为 Linux 视频设备路径，例如 `/dev/video2`。如果您的设备支持 IR 相机，可以通过配置 UI 启用此选项。

## 要求

- IR 传感器暴露为 `/dev/video*` 设备的 Linux 系统。
- 在 Biopass 中已设置好面部识别。
- 访问相机设备的权限。

Biopass 仅从配置的 IR 视频设备读取数据。它不管理笔记本电脑或摄像头的硬件 IR 发射器。

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

## 2. 在 Biopass 中启用它

打开 Biopass 桌面应用并进入面部设置。

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
