# IR Camera Guide

[简体中文](IR%20camera.zh-CN.md) | English

biopass-rs uses an infrared (IR) camera for face anti-spoofing, rather than relying only on the RGB anti-spoofing AI model. This is usually configured as a Linux video device path such as `/dev/video2`. If your devices supports IR camera, you can turn on this option by using the configuration UI.

## Requirements

- A Linux system where the IR sensor is exposed as a `/dev/video*` device.
- A working face setup in biopass-rs.
- Permission to access the camera device.

biopass-rs only reads from the configured IR video device. It does not manage the hardware IR emitter for your laptop or webcam.

## How It Works

The IR anti-spoofing pipeline runs as a layered liveness check:

1. **LED / exposure warm-up** — the IR camera may initially return a dark frame, so biopass-rs warms the stream up for `ir.warmup_delay_ms` (default 300ms) by pulling frames until the sensor's exposure/gain converges, before the frames that count are captured.
2. **Frame capture** — three IR frames are captured in sequence.
3. **Face detection** — a YOLO model (`yolov8n-face.onnx`) locates a face in each IR frame.
4. **RGB / IR spatial match** — the IR detection is matched to the RGB-authenticated face by bounding-box IoU. If no IR face overlaps the RGB face, the IR frame is skipped.
5. **Minimum face scale check** — the detected IR face must occupy at least `ir.min_face_area_ratio` (default 0.08) of the frame area. Tiny / distant faces are skipped, not classified as spoof.
6. **Liveness classification** — a MobileNetV3 model (`mobilenetv3_antispoof.onnx`) classifies each accepted crop as **real** or **spoof**. Since the model expects RGB, the single grayscale IR channel is cloned into all 3 color channels.

A frame is accepted as real only when the SPOOF class does not win, the real score meets `anti_spoofing.ir.model.threshold`, and the real score is strictly greater than the spoof score.

For each authentication attempt, biopass-rs collects 3 usable IR face crops and requires at least 2 of them to pass liveness — a strict majority vote. Structural failures (model missing, frame unreadable, face too small, no spatial match) are always treated as a spoof. The IR model's own spoof verdict is configurable: when `ir.ir_model_hard_fail` is `true`, a classifier spoof verdict also fails the check (fail-closed); when `false` (the default), the classifier score is treated as diagnostic only — a high spoof score is logged but does not by itself reject the attempt. The other checks (face presence, scale, spatial match) must still pass regardless.

When the RGB anti-spoofing model is enabled alongside IR (`anti_spoofing.rgb`), **both** must independently pass before authentication is granted. The RGB AI anti-spoofing task and IR liveness task run in sequence.

`ir.auto_optimize_camera` (default `false`) reuses the RGB camera's V4L2 exposure and image-control settings on the IR camera, which helps IR sensors that need matching exposure to produce a usable frame.

When debug mode is enabled, biopass-rs saves additional IR diagnostics under `~/.local/share/biopass-rs/<user>/debugs/`, including:

- `ir_raw_frame.*.jpg` — every raw IR frame
- `ir_spoof.*.jpg` — the cropped face the liveness classifier rejected
- `ir_face_too_small.*.jpg` / `ir_face_mismatch.*.jpg` / `ir_no_face.*.jpg` — the upstream failure labels

These help distinguish between a real model mismatch and insufficient input detail caused by distance, blur, or poor crop scale.

## 1. Find the IR Camera Device

List video devices:

```bash
ls -l /dev/video*
```

If `v4l2-ctl` is available, it is usually easier to identify the correct device with:

```bash
v4l2-ctl --list-devices
```

Look for the device node that belongs to your IR sensor, for example `/dev/video2`.

## 2. Enable It In biopass-rs

Open the biopass-rs desktop app and go to the face settings.

In the anti-spoofing section:

1. Enable face anti-spoofing if you want to use the AI anti-spoofing model too.
2. Set `IR Camera` to the correct `/dev/video*` device.
3. Save your configuration.

If you only want IR-based anti-spoofing, selecting the `IR Camera` device is enough.

## 3. If The IR Emitter Stays Off On Linux

On some Linux systems, the IR camera is detected but the IR light emitter does not turn on automatically. In that case, use [`linux-enable-ir-emitter`](https://github.com/EmixamPP/linux-enable-ir-emitter).

To install it:

```bash
VERSION=6.1.2
DIST=linux-enable-ir-emitter-$VERSION-release.systemd.x86-64.tar.gz
wget https://github.com/EmixamPP/linux-enable-ir-emitter/releases/download/$VERSION/$DIST
sudo tar -C / --no-same-owner -m -h -vxzf $DIST
```

Then, configure your IR emitter: 

```bash
sudo linux-enable-ir-emitter configure
```

Follow instructions printed when it is configuring your camera. After successfully triggering your IR emitter, please run this command:

```bash
sudo systemctl enable --now linux-enable-ir-emitter
```

Thanks @notherealmarco for help me on this https://github.com/TickLabVN/biopass/discussions/60#discussioncomment-16521628.
