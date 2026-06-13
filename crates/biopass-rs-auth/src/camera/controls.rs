use crate::{emit_log, LogLevel};
use v4l::control::{Control, Value as ControlValue};
use v4l::prelude::Device;

/// 应用相机控制参数以优化图像质量
///
/// 此函数会尽力启用以下功能:
/// - 自动白平衡 (AWB)
/// - 自动曝光 (光圈优先)
/// - 防闪烁 (50Hz)
/// - 宽动态范围 (背光补偿)
/// - 曝光优先 (动态帧率)
///
/// Apply camera control parameters to improve image quality.
///
/// Controls are best-effort: when the device does not expose a particular
/// control (typical for IR cameras which lack white-balance / exposure
/// controls) the EINVAL from the kernel is treated as a no-op so we don't
/// spam warnings for a perfectly normal configuration.
pub(super) fn apply_camera_optimizations(device: &mut Device, debug: bool) -> Result<(), String> {
    // V4L2 control constants
    const WHITE_BALANCE_AUTOMATIC: u32 = 0x0098_090c;
    const POWER_LINE_FREQUENCY: u32 = 0x0098_0918;
    const BACKLIGHT_COMPENSATION: u32 = 0x0098_091c;
    const AUTO_EXPOSURE: u32 = 0x009a_0901;
    const EXPOSURE_DYNAMIC_FRAMERATE: u32 = 0x009a_0903;
    // ENOTTY is returned when the device is not a V4L2 device at all
    // (some virtual devices /dev/videoN entries). Treat it the same way
    // as EINVAL: silently skip the control.
    const ENOTTY: i32 = 25;
    const EINVAL: i32 = 22;

    let try_control = |control: Control, label: &str| {
        match device.set_control(control) {
            Ok(()) => {}
            Err(error) if matches!(error.raw_os_error(), Some(EINVAL) | Some(ENOTTY)) => {
                // Device doesn't expose this control — expected for IR / cheap webcams.
            }
            Err(error) => {
                emit_log(
                    LogLevel::Warn,
                    debug,
                    "camera:controls",
                    &format!("failed to set {label}: {error}"),
                );
            }
        }
    };

    // Auto white balance
    try_control(
        Control {
            id: WHITE_BALANCE_AUTOMATIC,
            value: ControlValue::Boolean(true),
        },
        "enable auto white balance",
    );

    // Anti-flicker - 50Hz (China/Europe)
    try_control(
        Control {
            id: POWER_LINE_FREQUENCY,
            value: ControlValue::Integer(1),
        },
        "set anti-flicker (50Hz)",
    );

    // Backlight compensation (≈ wide dynamic range)
    try_control(
        Control {
            id: BACKLIGHT_COMPENSATION,
            value: ControlValue::Integer(2),
        },
        "set backlight compensation",
    );

    // Auto exposure - aperture priority mode
    try_control(
        Control {
            id: AUTO_EXPOSURE,
            value: ControlValue::Integer(3),
        },
        "set auto exposure (aperture priority)",
    );

    // Enable dynamic framerate (exposure priority)
    try_control(
        Control {
            id: EXPOSURE_DYNAMIC_FRAMERATE,
            value: ControlValue::Boolean(true),
        },
        "enable exposure dynamic framerate",
    );

    Ok(())
}
