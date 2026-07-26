use super::camera::{
    ir_camera_request, IR_LIVENESS_FRAME_COUNT, IR_LIVENESS_FRAME_INTERVAL_MS,
    IR_LIVENESS_REQUIRED_PASSES,
};
use super::outcome::AntiSpoofingCheckOutcome;
use super::session::DefaultFaceAuthRuntime;
use super::storage::save_debug_frame_if_enabled;
use super::FaceAuth;
use crate::{
    emit_log, AuthConfig, CameraSession, FaceBox, FaceMethodConfig, IrLivenessSummary,
    LogComponent, LogLevel, RgbFrame,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

impl FaceAuth {
    pub(super) fn check_anti_spoofing(
        &mut self,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
        rgb_frame: &RgbFrame,
        rgb_face_box: FaceBox,
        face: &RgbFrame,
    ) -> Result<AntiSpoofingCheckOutcome, String> {
        let debug = auth_config.debug;
        let log = |level: LogLevel, msg: &str| {
            emit_log(LogComponent::Auth, level, "FaceAntiSpoofing", msg)
        };

        if !auth_config.antispoof {
            log(LogLevel::Info, "skipped (antispoof disabled at runtime)");
            return Ok(AntiSpoofingCheckOutcome::passed());
        }

        let ai_enabled = self.config.anti_spoofing.rgb.enable;
        let ir_enabled = self.config.anti_spoofing.ir.enable;
        if !ai_enabled && !ir_enabled {
            log(LogLevel::Info, "skipped (no ai or ir sub-check enabled)");
            return Ok(AntiSpoofingCheckOutcome::passed());
        }

        log(
            LogLevel::Info,
            &format!("running checks (ai={ai_enabled}, ir={ir_enabled})"),
        );

        if ai_enabled {
            let model_path = self.config.anti_spoofing.rgb.model.path.clone();
            let max_attempts = self.config.anti_spoofing.rgb.retries.saturating_add(1);
            let retry_delay_ms = self.config.anti_spoofing.rgb.retry_delay_ms;
            if model_path.is_empty() || !Path::new(&model_path).is_file() {
                log(
                    LogLevel::Warn,
                    "ai model not configured or missing on disk, treating as spoof",
                );
                save_debug_frame_if_enabled(debug, username, face, "ai_model_missing");
                return Ok(AntiSpoofingCheckOutcome::failed(
                    "rgb_anti_spoofing_model_missing",
                    "RGB anti-spoofing model is missing",
                ));
            }

            let mut attempt = 0u32;
            let verdict = loop {
                attempt += 1;
                log(
                    LogLevel::Debug,
                    &format!("ai attempt {attempt}/{max_attempts}"),
                );
                let verdict = match self.runtime.detect_rgb_spoof(&self.config, face) {
                    Ok(verdict) => verdict,
                    Err(error) => {
                        save_debug_frame_if_enabled(debug, username, face, "ai_error");
                        return Err(error);
                    }
                };
                log(
                    LogLevel::Debug,
                    &format!("ai model verdict: spoof={}", verdict.spoof),
                );
                if !verdict.spoof {
                    break verdict;
                }
                if attempt >= max_attempts {
                    break verdict;
                }
                if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                    log(LogLevel::Info, "ai check cancelled during retry");
                    return Ok(AntiSpoofingCheckOutcome::failed(
                        "cancelled",
                        "RGB anti-spoofing was cancelled during retry",
                    ));
                }
                if retry_delay_ms > 0 {
                    log(
                        LogLevel::Debug,
                        &format!("ai retry sleeping {retry_delay_ms}ms"),
                    );
                    std::thread::sleep(Duration::from_millis(retry_delay_ms as u64));
                }
            };
            if verdict.spoof {
                save_debug_frame_if_enabled(debug, username, face, "ai_spoof_detected");
                return Ok(AntiSpoofingCheckOutcome::failed(
                    "rgb_spoof_detected",
                    "RGB anti-spoofing classified the candidate as spoof",
                ));
            }
        }

        if ir_enabled {
            log(LogLevel::Info, "running IR face liveness check");
            let ir = self.runtime.run_ir_liveness(
                &self.config,
                username,
                auth_config,
                cancel_signal,
                rgb_frame,
                rgb_face_box,
            )?;
            let passed = ir.passed_frames >= ir.required_passes;
            if !passed {
                return Ok(AntiSpoofingCheckOutcome::failed(
                    ir.last_failure.as_deref().unwrap_or("ir_liveness_failed"),
                    &format!(
                        "IR liveness failed: {}/{} frame(s) passed, required {}",
                        ir.passed_frames, ir.frames, ir.required_passes
                    ),
                )
                .with_ir(ir));
            }
        }

        Ok(AntiSpoofingCheckOutcome::passed())
    }
}

impl DefaultFaceAuthRuntime {
    pub(super) fn run_ir_check_with_retries(
        &mut self,
        config: &FaceMethodConfig,
        username: &str,
        auth_config: &AuthConfig,
        cancel_signal: Option<&AtomicBool>,
        rgb_frame: &RgbFrame,
        rgb_face_box: FaceBox,
    ) -> Result<IrLivenessSummary, String> {
        let debug = auth_config.debug;
        let log = |level: LogLevel, msg: &str| {
            emit_log(LogComponent::Auth, level, "FaceAntiSpoofingIr", msg)
        };

        let Some(camera) = config
            .anti_spoofing
            .ir
            .camera
            .as_deref()
            .filter(|camera| !camera.is_empty())
            .map(str::to_owned)
        else {
            log(
                LogLevel::Error,
                "no IR camera configured, cannot run liveness check",
            );
            return Ok(ir_summary(0, 0, 1, Some("ir_camera_missing"), None));
        };

        if !Path::new(&config.detection.model).is_file() {
            log(
                LogLevel::Error,
                "detection model missing, cannot run IR check. Run 'biopass-rs-helper model-download' to download models.",
            );
            return Ok(ir_summary(
                0,
                0,
                1,
                Some("ir_detection_model_missing"),
                None,
            ));
        }

        let model_path = config.anti_spoofing.ir.model.path.clone();
        if model_path.is_empty() || !Path::new(&model_path).is_file() {
            log(
                LogLevel::Error,
                "IR anti-spoofing model missing, cannot run liveness check. Run 'biopass-rs-helper model-download' to download models.",
            );
            return Ok(ir_summary(
                0,
                0,
                1,
                Some("ir_anti_spoofing_model_missing"),
                None,
            ));
        }

        let max_attempts = config.anti_spoofing.ir.retries.saturating_add(1);
        let retry_delay_ms = config.anti_spoofing.ir.retry_delay_ms;
        if self.ir_camera_session.is_none() {
            let request =
                ir_camera_request(&camera, config.anti_spoofing.ir.auto_optimize_camera, debug);
            let mut ir_session = match CameraSession::open(&request) {
                Ok(session) => session,
                Err(error) => {
                    log(
                        LogLevel::Warn,
                        &format!("IR camera session open failed: {error}"),
                    );
                    return Ok(ir_summary(0, 0, 1, Some("ir_camera_open_failed"), None));
                }
            };

            if config.anti_spoofing.ir.warmup_delay_ms > 0 {
                log(
                    LogLevel::Debug,
                    &format!(
                        "streaming {}ms for IR warmup",
                        config.anti_spoofing.ir.warmup_delay_ms
                    ),
                );
                if let Err(error) = ir_session.warmup_for(Duration::from_millis(
                    config.anti_spoofing.ir.warmup_delay_ms as u64,
                )) {
                    log(LogLevel::Warn, &format!("IR camera warmup failed: {error}"));
                    return Ok(ir_summary(0, 0, 1, Some("ir_camera_warmup_failed"), None));
                }
            }

            if let Err(error) = ir_session.warmup(request.warmup_frames) {
                log(LogLevel::Warn, &format!("IR camera warmup failed: {error}"));
                return Ok(ir_summary(0, 0, 1, Some("ir_camera_warmup_failed"), None));
            }

            self.ir_camera_session = Some(ir_session);
        }

        let mut ir_session = self
            .ir_camera_session
            .take()
            .expect("IR camera session is initialized");
        let mut summary = ir_summary(0, 0, 1, Some("ir_liveness_failed"), None);
        for attempt in 1..=max_attempts {
            if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                summary.last_failure = Some("cancelled".to_string());
                break;
            }
            if attempt > 1 {
                emit_log(
                    LogComponent::Auth,
                    LogLevel::Debug,
                    "FaceAntiSpoofingIr",
                    &format!("attempt {attempt}/{max_attempts}"),
                );
            }
            let result = self.check_ir_liveness(
                config,
                username,
                auth_config,
                &mut ir_session,
                rgb_frame,
                rgb_face_box,
            );
            match result {
                Ok(current) if current.passed_frames >= current.required_passes => {
                    summary = current;
                    break;
                }
                Ok(current) => {
                    summary = current;
                }
                Err(error) => {
                    self.ir_camera_session = Some(ir_session);
                    return Err(error);
                }
            }
            if attempt < max_attempts {
                if cancel_signal.is_some_and(|signal| signal.load(Ordering::SeqCst)) {
                    break;
                }
                if retry_delay_ms > 0 {
                    emit_log(
                        LogComponent::Auth,
                        LogLevel::Debug,
                        "FaceAntiSpoofingIr",
                        &format!("retry sleeping {retry_delay_ms}ms"),
                    );
                    std::thread::sleep(Duration::from_millis(retry_delay_ms as u64));
                }
            }
        }
        self.ir_camera_session = Some(ir_session);
        Ok(summary)
    }

    fn check_ir_liveness(
        &mut self,
        config: &FaceMethodConfig,
        username: &str,
        auth_config: &AuthConfig,
        ir_session: &mut CameraSession,
        rgb_frame: &RgbFrame,
        rgb_face_box: FaceBox,
    ) -> Result<IrLivenessSummary, String> {
        let debug = auth_config.debug;
        let log = |level: LogLevel, msg: &str| {
            emit_log(LogComponent::Auth, level, "FaceAntiSpoofingIr", msg)
        };

        let min_face_area_ratio = config.anti_spoofing.ir.min_face_area_ratio;
        let model_diagnostic = !config.anti_spoofing.ir.ir_model_hard_fail;

        log(
            LogLevel::Info,
            &format!(
                "collecting {IR_LIVENESS_FRAME_COUNT} IR frame(s), requiring >= {IR_LIVENESS_REQUIRED_PASSES} liveness pass(es)"
            ),
        );

        let mut passed_frames: usize = 0;
        let mut last_failure: Option<String> = None;
        let mut highest_detection_confidence: Option<f32> = None;
        for frame_idx in 0..IR_LIVENESS_FRAME_COUNT {
            log(
                LogLevel::Debug,
                &format!(
                    "capturing IR frame {}/{}",
                    frame_idx + 1,
                    IR_LIVENESS_FRAME_COUNT
                ),
            );
            let frame = match ir_session.next_frame() {
                Ok(frame) => frame,
                Err(error) => {
                    log(LogLevel::Warn, &format!("IR frame capture failed: {error}"));
                    last_failure = Some("ir_frame_capture_failed".to_string());
                    continue;
                }
            };
            log(
                LogLevel::Debug,
                &format!("IR frame captured: {}x{}", frame.width, frame.height),
            );
            save_debug_frame_if_enabled(debug, username, &frame, "ir_raw_frame");

            let detections = match self
                .detector(config)
                .and_then(|detector| detector.detect(&frame))
            {
                Ok(detections) => detections,
                Err(error) => {
                    log(LogLevel::Warn, &format!("IR detection error: {error}"));
                    save_debug_frame_if_enabled(debug, username, &frame, "ir_detection_error");
                    last_failure = Some("ir_detection_error".to_string());
                    continue;
                }
            };
            log(
                LogLevel::Debug,
                &format!("IR detection found {} face(s)", detections.len()),
            );
            if detections.is_empty() {
                log(
                    LogLevel::Info,
                    "no face detected in IR frame (highest confidence is 0.0 — nothing above detector threshold)",
                );
                save_debug_frame_if_enabled(debug, username, &frame, "ir_no_face");
                last_failure = Some("ir_no_face".to_string());
                if frame_idx + 1 < IR_LIVENESS_FRAME_COUNT {
                    std::thread::sleep(Duration::from_millis(IR_LIVENESS_FRAME_INTERVAL_MS));
                }
                continue;
            }

            let highest_confidence = detections
                .iter()
                .map(|detection| detection.confidence)
                .fold(0.0_f32, f32::max);
            highest_detection_confidence = Some(
                highest_detection_confidence
                    .map(|current| current.max(highest_confidence))
                    .unwrap_or(highest_confidence),
            );

            let frame_area = (frame.width as f32) * (frame.height as f32);
            let mut max_face_area_ratio = 0.0_f32;
            let mut max_face_area_width = 0_u32;
            let mut max_face_area_height = 0_u32;
            let usable: Vec<_> = detections
                .into_iter()
                .filter(|detection| {
                    if frame_area <= 0.0 || min_face_area_ratio <= 0.0 {
                        return true;
                    }
                    let bbox_area =
                        (detection.bbox.width() as f32) * (detection.bbox.height() as f32);
                    let area_ratio = bbox_area / frame_area;
                    if area_ratio > max_face_area_ratio {
                        max_face_area_ratio = area_ratio;
                        max_face_area_width = detection.bbox.width();
                        max_face_area_height = detection.bbox.height();
                    }
                    area_ratio >= min_face_area_ratio
                })
                .collect();
            if usable.is_empty() {
                log(
                    LogLevel::Info,
                    &format!(
                        "IR face too small for reliable liveness (highest conf={:.4}, max bbox={}x{}, max area ratio={:.4}, ratio threshold={:.4}), treating as spoof",
                        highest_confidence,
                        max_face_area_width,
                        max_face_area_height,
                        max_face_area_ratio,
                        min_face_area_ratio
                    ),
                );
                save_debug_frame_if_enabled(debug, username, &frame, "ir_face_too_small");
                last_failure = Some("ir_face_too_small".to_string());
                if frame_idx + 1 < IR_LIVENESS_FRAME_COUNT {
                    std::thread::sleep(Duration::from_millis(IR_LIVENESS_FRAME_INTERVAL_MS));
                }
                continue;
            }

            let best_detection = match pick_ir_face_match(
                rgb_frame.width,
                rgb_frame.height,
                rgb_face_box,
                frame.width,
                frame.height,
                &usable,
            ) {
                Some(detection) => detection,
                None => {
                    log(
                        LogLevel::Info,
                        &format!(
                            "no IR face matches the RGB face spatially (highest conf={:.4}), treating as spoof",
                            highest_confidence
                        ),
                    );
                    save_debug_frame_if_enabled(debug, username, &frame, "ir_face_mismatch");
                    last_failure = Some("ir_face_mismatch".to_string());
                    if frame_idx + 1 < IR_LIVENESS_FRAME_COUNT {
                        std::thread::sleep(Duration::from_millis(IR_LIVENESS_FRAME_INTERVAL_MS));
                    }
                    continue;
                }
            };
            let selected_bbox_area =
                (best_detection.bbox.width() as f32) * (best_detection.bbox.height() as f32);
            let selected_area_ratio = if frame_area <= 0.0 {
                0.0
            } else {
                selected_bbox_area / frame_area
            };
            log(
                LogLevel::Debug,
                &format!(
                    "selected IR face crop conf={:.4} bbox={}x{}@({},{}), area ratio={:.4}, ratio threshold={:.4}",
                    best_detection.confidence,
                    best_detection.bbox.width(),
                    best_detection.bbox.height(),
                    best_detection.bbox.x1,
                    best_detection.bbox.y1,
                    selected_area_ratio,
                    min_face_area_ratio,
                ),
            );

            let verdict = match self
                .ir_anti_spoofing(config)
                .and_then(|model| model.detect(&best_detection.crop))
            {
                Ok(verdict) => verdict,
                Err(error) => {
                    log(LogLevel::Warn, &format!("IR classifier error: {error}"));
                    save_debug_frame_if_enabled(
                        debug,
                        username,
                        &best_detection.crop,
                        "ir_classifier_error",
                    );
                    if model_diagnostic {
                        passed_frames += 1;
                    } else {
                        last_failure = Some("ir_classifier_error".to_string());
                    }
                    continue;
                }
            };
            log(
                LogLevel::Debug,
                &format!(
                    "IR liveness verdict frame {}/{}: spoof={}",
                    frame_idx + 1,
                    IR_LIVENESS_FRAME_COUNT,
                    verdict.spoof
                ),
            );
            if verdict.spoof {
                if model_diagnostic {
                    log(
                        LogLevel::Debug,
                        &format!(
                            "IR liveness classifier failed frame {}/{} as diagnostic (detection conf={:.4}, classifier score={:.4}, ir_model_hard_fail=false)",
                            frame_idx + 1,
                            IR_LIVENESS_FRAME_COUNT,
                            highest_confidence,
                            verdict.score
                        ),
                    );
                } else {
                    log(
                        LogLevel::Info,
                        &format!(
                            "IR liveness FAILED frame {}/{} (detection conf={:.4}, classifier score={:.4})",
                            frame_idx + 1,
                            IR_LIVENESS_FRAME_COUNT,
                            highest_confidence,
                            verdict.score
                        ),
                    );
                }
                save_debug_frame_if_enabled(debug, username, &best_detection.crop, "ir_spoof");
                if model_diagnostic {
                    passed_frames += 1;
                } else {
                    last_failure = Some("ir_spoof_detected".to_string());
                }
            } else {
                passed_frames += 1;
            }

            if frame_idx + 1 < IR_LIVENESS_FRAME_COUNT {
                std::thread::sleep(Duration::from_millis(IR_LIVENESS_FRAME_INTERVAL_MS));
            }
        }

        let required = IR_LIVENESS_REQUIRED_PASSES.min(IR_LIVENESS_FRAME_COUNT);
        let passed = passed_frames >= required;
        if passed {
            last_failure = None;
        } else if last_failure.is_none() {
            last_failure = Some("ir_liveness_failed".to_string());
        }
        log(
            LogLevel::Info,
            &format!(
                "IR liveness aggregate: {passed_frames}/{IR_LIVENESS_FRAME_COUNT} frame(s) passed, required >= {required}, verdict={passed}"
            ),
        );
        Ok(ir_summary(
            IR_LIVENESS_FRAME_COUNT,
            passed_frames,
            required,
            last_failure.as_deref(),
            highest_detection_confidence,
        ))
    }
}

pub(super) fn ir_summary(
    frames: usize,
    passed_frames: usize,
    required_passes: usize,
    last_failure: Option<&str>,
    highest_detection_confidence: Option<f32>,
) -> IrLivenessSummary {
    IrLivenessSummary {
        frames,
        passed_frames,
        required_passes,
        last_failure: last_failure.map(str::to_string),
        highest_detection_confidence,
    }
}

#[allow(clippy::too_many_arguments)]
fn pick_ir_face_match(
    rgb_width: u32,
    rgb_height: u32,
    rgb_box: FaceBox,
    ir_width: u32,
    ir_height: u32,
    detections: &[crate::FaceDetection],
) -> Option<&crate::FaceDetection> {
    if detections.is_empty() || rgb_width == 0 || rgb_height == 0 || ir_width == 0 || ir_height == 0
    {
        return None;
    }

    let target_rx1 = rgb_box.x1 as f32 / rgb_width as f32;
    let target_ry1 = rgb_box.y1 as f32 / rgb_height as f32;
    let target_rx2 = rgb_box.x2 as f32 / rgb_width as f32;
    let target_ry2 = rgb_box.y2 as f32 / rgb_height as f32;
    let target_cx = (target_rx1 + target_rx2) * 0.5;
    let target_cy = (target_ry1 + target_ry2) * 0.5;

    let mut best: Option<(&crate::FaceDetection, f32, f32)> = None;
    for detection in detections {
        if detection.bbox.width() == 0 || detection.bbox.height() == 0 {
            continue;
        }
        let ir_rx1 = detection.bbox.x1 as f32 / ir_width as f32;
        let ir_ry1 = detection.bbox.y1 as f32 / ir_height as f32;
        let ir_rx2 = detection.bbox.x2 as f32 / ir_width as f32;
        let ir_ry2 = detection.bbox.y2 as f32 / ir_height as f32;
        let iou = bbox_iou(
            target_rx1, target_ry1, target_rx2, target_ry2, ir_rx1, ir_ry1, ir_rx2, ir_ry2,
        );
        let ir_cx = (ir_rx1 + ir_rx2) * 0.5;
        let ir_cy = (ir_ry1 + ir_ry2) * 0.5;
        let dx = ir_cx - target_cx;
        let dy = ir_cy - target_cy;
        let center_distance = (dx * dx + dy * dy).sqrt();
        match best {
            None => best = Some((detection, iou, center_distance)),
            Some((_, best_iou, best_dist)) => {
                if iou > best_iou || (best_iou <= 0.0 && center_distance < best_dist) {
                    best = Some((detection, iou, center_distance));
                }
            }
        }
    }

    let (detection, iou, center_distance) = best?;
    if iou <= 0.0 && center_distance > 0.3 {
        return None;
    }
    Some(detection)
}

#[allow(clippy::too_many_arguments)]
fn bbox_iou(ax1: f32, ay1: f32, ax2: f32, ay2: f32, bx1: f32, by1: f32, bx2: f32, by2: f32) -> f32 {
    let inter_x1 = ax1.max(bx1);
    let inter_y1 = ay1.max(by1);
    let inter_x2 = ax2.min(bx2);
    let inter_y2 = ay2.min(by2);
    let inter_w = (inter_x2 - inter_x1).max(0.0);
    let inter_h = (inter_y2 - inter_y1).max(0.0);
    let inter = inter_w * inter_h;
    let area_a = (ax2 - ax1).max(0.0) * (ay2 - ay1).max(0.0);
    let area_b = (bx2 - bx1).max(0.0) * (by2 - by1).max(0.0);
    let union = area_a + area_b - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ir_summary_records_liveness_vote_details() {
        let summary = ir_summary(3, 1, 2, Some("ir_face_mismatch"), Some(0.76));

        assert_eq!(summary.frames, 3);
        assert_eq!(summary.passed_frames, 1);
        assert_eq!(summary.required_passes, 2);
        assert_eq!(summary.last_failure.as_deref(), Some("ir_face_mismatch"));
        assert_eq!(summary.highest_detection_confidence, Some(0.76));
    }
}
