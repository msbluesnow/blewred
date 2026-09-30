use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use crate::lexical::LexicalEngine;
use crate::obs::OBSClient;
use crate::vision::{VisionEngine, SpecificTestResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct SubtitleCue {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

// Active Censor Sources Mask
pub const CENSOR_SRC_SCREEN_NSFW: u32    = 1 << 0; // AI Vision NSFW on screen (Modes 0, 2)
pub const CENSOR_SRC_SCREEN_OCR: u32     = 1 << 1; // OCR Stop-words on screen (Modes 0, 2)
pub const CENSOR_SRC_SCHEDULED_CUE: u32  = 1 << 2; // Scheduled Cue Timeline (Modes 0, 1)
pub const CENSOR_SRC_LOOKAHEAD_DUE: u32  = 1 << 3; // Lookahead Buffer Queue (Modes 0, 1)
pub const CENSOR_SRC_MANUAL: u32         = 1 << 4; // Manual Emergency Hotkey / Panic Button

static ACTIVE_CENSOR_SOURCES: AtomicU32 = AtomicU32::new(0);
static IS_CENSOR_ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST_ACTIVE_CENSOR_REASON: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static LAST_ACTIVE_CENSOR_SCORE: AtomicU32 = AtomicU32::new(0);

pub fn get_current_censor_reason() -> String {
    LAST_ACTIVE_CENSOR_REASON.lock().map(|g| g.clone()).unwrap_or_default()
}

pub fn get_current_censor_score() -> f32 {
    LAST_ACTIVE_CENSOR_SCORE.load(Ordering::Relaxed) as f32 / 100.0
}

// CensorShield feature enabled by default:
static CENSOR_SHIELD_ENABLED: AtomicBool = AtomicBool::new(true);
static SHIELD_DONATE_ENABLED: AtomicBool = AtomicBool::new(true);

// Operational Mode:
// 0 = Hybrid (Default: Screen Guard + Browser Player Lookahead)
// 1 = PlayerOnly (Browser Lookahead only. Desktop screen capture and OCR loop are paused. CPU ~0%!)
// 2 = ScreenOnly (Desktop Screen Guard only. Browser extension lookahead disabled)
// 3 = Standby / Disabled (All protection modes paused. 0% overhead, clean unmuted outputs)
static OPERATION_MODE: AtomicU8 = AtomicU8::new(0);
pub static MODE_GENERATION: AtomicU64 = AtomicU64::new(0);
pub static LAST_BROWSER_EXTENSION_PULSE: AtomicU64 = AtomicU64::new(0);
pub static REAL_ANALYSIS_FPS_X10: AtomicU32 = AtomicU32::new(0);
pub static DYNAMIC_BOOST_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn get_real_analysis_fps() -> f32 {
    (REAL_ANALYSIS_FPS_X10.load(Ordering::Relaxed) as f32) / 10.0
}

pub fn make_incident_json(
    id: &str,
    timestamp: &str,
    monitor: &str,
    violation_type: &str,
    target: &str,
    score: f32,
    circumstances: &str,
    status: &str,
    action: &str,
) -> serde_json::Value {
    WSBridge::make_incident_json(id, timestamp, monitor, violation_type, target, score, circumstances, status, action)
}

pub fn is_dynamic_boost_active() -> bool {
    DYNAMIC_BOOST_ACTIVE.load(Ordering::Relaxed)
}

pub fn get_analysis_fps_and_status(
    op_mode: u8,
    guard_active: bool,
    boosted: bool,
    is_dynamic: bool,
) -> (u32, &'static str) {
    if op_mode == 1 {
        (0, "0 FPS (Paused - Player Analysis)")
    } else if op_mode >= 3 {
        (0, "0 FPS (Paused - Standby)")
    } else if !guard_active {
        (0, "0 FPS (Paused - Shield Off)")
    } else if is_dynamic {
        (60, "60 FPS (Dynamic Boost)")
    } else if boosted {
        (60, "60 FPS (Boosted Mode)")
    } else {
        (5, "5 FPS (Background Scan)")
    }
}

pub fn get_operation_mode() -> u8 {
    OPERATION_MODE.load(Ordering::Relaxed)
}

pub fn set_operation_mode(val: u8) {
    OPERATION_MODE.store(val, Ordering::Relaxed);
    println!("[WSBridge] Operational mode updated to: {} (0=Hybrid, 1=PlayerOnly, 2=ScreenOnly, 3=Standby)", val);
}

pub fn record_browser_extension_pulse() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    LAST_BROWSER_EXTENSION_PULSE.store(now, Ordering::Relaxed);
}

pub fn is_browser_extension_connected() -> bool {
    let last = LAST_BROWSER_EXTENSION_PULSE.load(Ordering::Relaxed);
    if last == 0 {
        return false;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    now.saturating_sub(last) < 10000 // 10s tolerance prevents disconnect jitter during background tab throttling
}

pub async fn safe_set_operation_mode(
    mode: u8,
    obs: &OBSClient,
    tx_bcast: &broadcast::Sender<String>,
    lookahead: &crate::lookahead::LookaheadEngine,
    vision: &crate::vision::VisionEngine,
) {
    let prev_mode = OPERATION_MODE.swap(mode, Ordering::SeqCst);
    let gen = MODE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    println!("[Orchestrator] Mode transition: {} -> {} (Gen: {})", prev_mode, mode, gen);
    crate::update_tray_mode(mode);

    // Defensive safeguards per transition target:
    if mode == 1 {
        // Mode 1: PlayerOnly. Screen capture & OCR loop are paused.
        ACTIVE_CENSOR_SOURCES.fetch_and(!(CENSOR_SRC_SCREEN_NSFW | CENSOR_SRC_SCREEN_OCR), Ordering::SeqCst);
        vision.clear_obs_censor();
        REAL_ANALYSIS_FPS_X10.store(0, Ordering::Relaxed);
        DYNAMIC_BOOST_ACTIVE.store(false, Ordering::Relaxed);
    } else if mode == 2 {
        // Mode 2: ScreenOnly. Browser player cues & lookahead are paused.
        ACTIVE_CENSOR_SOURCES.fetch_and(!(CENSOR_SRC_SCHEDULED_CUE | CENSOR_SRC_LOOKAHEAD_DUE), Ordering::SeqCst);
        crate::hide_lookahead_hud();
        lookahead.clear_active_tickets();
    } else if mode >= 3 {
        // Mode 3: Standby / Disabled (all modes off).
        ACTIVE_CENSOR_SOURCES.store(0, Ordering::SeqCst);
        vision.clear_obs_censor();
        crate::hide_lookahead_hud();
        lookahead.clear_active_tickets();
        obs.mute_all_audio(false).await;
        REAL_ANALYSIS_FPS_X10.store(0, Ordering::Relaxed);
        DYNAMIC_BOOST_ACTIVE.store(false, Ordering::Relaxed);
    }

    // Synchronize OBS Censor Shield visibility
    let any_active = ACTIVE_CENSOR_SOURCES.load(Ordering::Relaxed) != 0;
    let shield_enabled = CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed);
    if !(any_active && shield_enabled) {
        obs.set_censor_shield_enabled(false).await;
    }

    crate::settings::update_cached_settings(|s| s.operation_mode = mode);

    let resp = json!({
        "type": "operation_mode_changed",
        "operation_mode": mode,
        "generation": gen
    });
    let _ = tx_bcast.send(resp.to_string());
}

pub fn is_censor_active() -> bool {
    ACTIVE_CENSOR_SOURCES.load(Ordering::Relaxed) != 0 || IS_CENSOR_ACTIVE.load(Ordering::Relaxed)
}

pub fn is_censor_shield_enabled() -> bool {
    CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed)
}

pub fn set_censor_shield_enabled(val: bool) {
    CENSOR_SHIELD_ENABLED.store(val, Ordering::Relaxed);
}

pub fn is_shield_donate_enabled() -> bool {
    SHIELD_DONATE_ENABLED.load(Ordering::Relaxed)
}

pub fn set_shield_donate_enabled(val: bool) {
    SHIELD_DONATE_ENABLED.store(val, Ordering::Relaxed);
    crate::settings::update_cached_settings(|s| s.shield_donate_enabled = val);
}

/// Centralized state coordinator: Updates a specific censor source and synchronizes OBS Censor Shield visibility
pub async fn update_censor_source(
    obs: &OBSClient,
    tx_bcast: &broadcast::Sender<String>,
    source: u32,
    active: bool,
    reason: &str,
) {
    let prev_mask = if active {
        ACTIVE_CENSOR_SOURCES.fetch_or(source, Ordering::SeqCst)
    } else {
        ACTIVE_CENSOR_SOURCES.fetch_and(!source, Ordering::SeqCst)
    };
    let new_mask = if active {
        prev_mask | source
    } else {
        prev_mask & !source
    };

    let prev_any = prev_mask != 0;
    let new_any = new_mask != 0;

    IS_CENSOR_ACTIVE.store(new_any, Ordering::Relaxed);

    if active && !reason.trim().is_empty() && reason != "STREAM SAFE" {
        if let Ok(mut g) = LAST_ACTIVE_CENSOR_REASON.lock() {
            *g = reason.to_string();
        }
    } else if !new_any {
        if let Ok(mut g) = LAST_ACTIVE_CENSOR_REASON.lock() {
            g.clear();
        }
    }

    let shield_enabled = CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed);
    let should_show_shield = new_any && shield_enabled;

    let currently_shield_visible = obs.is_shield_visible.load(Ordering::Relaxed);

    if should_show_shield != currently_shield_visible {
        if should_show_shield {
            println!("[CensorManager] Violation active (sources: 0x{:X}, reason: '{}'). Showing Censor Shield in OBS.", new_mask, reason);
            obs.set_censor_shield_enabled(true).await;
            obs.send_mute_command("Mic/Aux", true).await;
            obs.send_mute_command("Desktop Audio", true).await;
        } else {
            println!("[CensorManager] Censor sources cleared or shield disabled (sources: 0x{:X}). Hiding Censor Shield in OBS.", new_mask);
            obs.set_censor_shield_enabled(false).await;
            if !new_any {
                obs.send_mute_command("Mic/Aux", false).await;
                obs.send_mute_command("Desktop Audio", false).await;
            }
        }
    } else if !new_any && prev_any {
        // When all violations are cleared, ensure audio is unmuted even if shield was not shown
        obs.send_mute_command("Mic/Aux", false).await;
        obs.send_mute_command("Desktop Audio", false).await;
    }

    let effective_reason = if new_any {
        if active && !reason.trim().is_empty() && reason != "STREAM SAFE" {
            reason.to_string()
        } else {
            let cached = LAST_ACTIVE_CENSOR_REASON.lock().map(|g| g.clone()).unwrap_or_default();
            if !cached.is_empty() {
                cached
            } else {
                reason.to_string()
            }
        }
    } else {
        "STREAM SAFE".to_string()
    };

    let shield_state = json!({
        "type": "shield_state_update",
        "is_nsfw": new_any,
        "reason": effective_reason
    });
    let _ = tx_bcast.send(shield_state.to_string());

    let shield_changed = json!({
        "type": "censor_shield_changed",
        "enabled": shield_enabled,
        "is_shield_visible": should_show_shield
    });
    let _ = tx_bcast.send(shield_changed.to_string());
}

/// Changes the Censor Shield armed preference and synchronizes OBS visibility accordingly
pub async fn set_censor_shield_enabled_with_sync(
    obs: &OBSClient,
    tx_bcast: &broadcast::Sender<String>,
    enabled: bool,
) {
    CENSOR_SHIELD_ENABLED.store(enabled, Ordering::SeqCst);
    crate::settings::update_cached_settings(|s| s.censor_shield_enabled = enabled);
    let any_active = is_censor_active();
    let should_show_shield = any_active && enabled;
    let currently_shield_visible = obs.is_shield_visible.load(Ordering::Relaxed);

    if should_show_shield != currently_shield_visible {
        if should_show_shield {
            println!("[CensorManager] Censor Shield armed and violation currently active. Showing Censor Shield in OBS.");
            obs.set_censor_shield_enabled(true).await;
            obs.send_mute_command("Mic/Aux", true).await;
            obs.send_mute_command("Desktop Audio", true).await;
        } else {
            println!("[CensorManager] Censor Shield disarmed or no violation. Hiding Censor Shield in OBS.");
            obs.set_censor_shield_enabled(false).await;
        }
    } else if !enabled && currently_shield_visible {
        obs.set_censor_shield_enabled(false).await;
    }

    println!(
        "[CensorManager] Censor Shield setting = {}, any_violation_active = {}, visible in OBS = {}",
        enabled, any_active, should_show_shield
    );

    let msg = json!({
        "type": "censor_shield_changed",
        "enabled": enabled,
        "is_shield_visible": should_show_shield
    });
    let _ = tx_bcast.send(msg.to_string());
}

/// Synchronizes the Censor Shield HTML file(s) on disk so `#shield-donate-box`
/// is hidden immediately if disabled, even without active WebSocket or script execution.
pub fn sync_censor_shield_html_donate(enabled: bool) {
    let mut candidate_paths = Vec::new();
    let root = crate::paths::PathResolver::find_project_root();
    candidate_paths.push(root.join("ui").join("censor_shield.html"));
    candidate_paths.push(root.join("censor_shield.html"));

    if let Ok(cwd) = std::env::current_dir() {
        candidate_paths.push(cwd.join("ui").join("censor_shield.html"));
        candidate_paths.push(cwd.join("censor_shield.html"));
    }

    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let appdata_dir = std::path::PathBuf::from(local_appdata).join("blewred");
        candidate_paths.push(appdata_dir.join("ui").join("censor_shield.html"));
        candidate_paths.push(appdata_dir.join("censor_shield.html"));
    }

    candidate_paths.sort();
    candidate_paths.dedup();

    let re_footer = match regex::Regex::new(r#"<div\s+class="shield-docked-footer"[^>]*id="shield-donate-box"[^>]*>|<div[^>]*id="shield-donate-box"[^>]*class="shield-docked-footer"[^>]*>"#) {
        Ok(re) => re,
        Err(_) => return,
    };

    for path in candidate_paths {
        if path.exists() && path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let new_tag = if enabled {
                    r#"<div class="shield-docked-footer" id="shield-donate-box">"#
                } else {
                    r#"<div class="shield-docked-footer" id="shield-donate-box" style="display: none;">"#
                };

                let updated = re_footer.replace_all(&content, new_tag).to_string();
                if updated != content {
                    let _ = std::fs::write(&path, updated);
                    println!("[WSBridge] Synchronized censor_shield.html donate display (enabled={}) at: {:?}", enabled, path);
                }
            }
        }
    }
}

/// Changes the Censor Shield support banner preference, persists it, updates disk HTML,
/// broadcasts live update to all WebSocket clients and refreshes OBS browser source cache.
pub async fn set_shield_donate_and_sync(
    obs: &OBSClient,
    tx_bcast: &broadcast::Sender<String>,
    enabled: bool,
) {
    set_shield_donate_enabled(enabled);
    crate::settings::update_cached_settings(|s| s.shield_donate_enabled = enabled);
    let cached = crate::settings::get_cached_settings();
    let _ = crate::settings::save_settings(&cached);

    // Synchronize HTML markup on disk immediately
    sync_censor_shield_html_donate(enabled);

    // Broadcast live event to WebSocket clients (UI and Censor Shield browser source)
    let msg = json!({
        "type": "shield_donate_changed",
        "enabled": enabled
    });
    let _ = tx_bcast.send(msg.to_string());

    // Tell OBS Studio browser source to refresh without cache
    obs.refresh_censor_shield().await;

    println!("[WSBridge] Censor Shield support banner set to: {} (HTML synced and OBS refreshed)", enabled);
}

/// Generates a realistic 320x180 JPEG thumbnail of a streaming video scene with target zone
pub fn get_realistic_test_thumbnail() -> String {
    let width = 320u32;
    let height = 180u32;
    let mut img = image::RgbImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            // Dark cinematic gradient background (navy/slate)
            let mut r = (18.0 + (x as f32 / width as f32) * 12.0) as u8;
            let mut g = (22.0 + (y as f32 / height as f32) * 16.0) as u8;
            let mut b = (32.0 + (x as f32 / width as f32) * 20.0) as u8;

            // Player bottom control bar (dark bar at y >= 164)
            if y >= 164 {
                r = 12; g = 14; b = 18;
                // Red playback progress line at y == 164
                if y == 164 && x < 130 {
                    r = 230; g = 30; b = 30;
                }
            }

            // Central test subject silhouette (x: 100..220, y: 35..150)
            let cx = x as f32 - 160.0;
            let cy = y as f32 - 92.0;
            let dist_sq = (cx * cx) / (45.0 * 45.0) + (cy * cy) / (55.0 * 55.0);
            if dist_sq <= 1.0 {
                let factor = (1.0 - dist_sq).sqrt();
                r = (185.0 * factor + r as f32 * (1.0 - factor)) as u8;
                g = (130.0 * factor + g as f32 * (1.0 - factor)) as u8;
                b = (115.0 * factor + b as f32 * (1.0 - factor)) as u8;
            }

            // Top player header bar (y <= 18)
            if y <= 20 {
                r = (r as f32 * 0.45) as u8;
                g = (g as f32 * 0.45) as u8;
                b = (b as f32 * 0.45) as u8;
            }

            img.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }

    let mut buf = std::io::Cursor::new(Vec::new());
    if img.write_to(&mut buf, image::ImageFormat::Jpeg).is_ok() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
        format!("data:image/jpeg;base64,{}", b64)
    } else {
        String::new()
    }
}

pub struct WSBridge {
    lexical: Arc<Mutex<LexicalEngine>>,
    obs: Arc<OBSClient>,
    vision: Arc<VisionEngine>,
    fps_boosted: Arc<AtomicBool>,
    pub realtime_guard_active: Arc<AtomicBool>,
    pub lookahead: Arc<crate::lookahead::LookaheadEngine>,
    port: u16,
    broadcast_tx: broadcast::Sender<String>,
}

impl WSBridge {
    pub fn new(
        lexical: Arc<Mutex<LexicalEngine>>,
        obs: Arc<OBSClient>,
        vision: Arc<VisionEngine>,
        fps_boosted: Arc<AtomicBool>,
        port: u16,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(100);
        let realtime_guard_active = Arc::new(AtomicBool::new(true));
        let lookahead = Arc::new(crate::lookahead::LookaheadEngine::new(vision.cascade.clone()));
        Self {
            lexical,
            obs,
            vision,
            fps_boosted,
            realtime_guard_active,
            lookahead,
            port,
            broadcast_tx,
        }
    }

    pub fn broadcast_alert(&self, source: &str, word: &str, rule: &str) {
        let payload = json!({
            "type": "banned_violation_spotted",
            "source": source,
            "word": word,
            "rule": rule,
            "is_banned": true,
            "matches": [word]
        });
        let _ = self.broadcast_tx.send(payload.to_string());
    }

    pub fn broadcast_emergency_mute(&self, duration_ms: u64) {
        let payload = json!({
            "type": "emergency_mute_triggered",
            "duration_ms": duration_ms
        });
        let _ = self.broadcast_tx.send(payload.to_string());
    }

    pub fn broadcast_shield_state(&self, is_nsfw: bool, reason: &str) {
        let payload = json!({
            "type": "shield_state_update",
            "is_nsfw": is_nsfw,
            "reason": reason
        });
        let _ = self.broadcast_tx.send(payload.to_string());
    }

    pub fn broadcast_fps_mode(&self, boosted: bool) {
        crate::settings::update_cached_settings(|s| s.fps_boosted = boosted);
        let op_mode = get_operation_mode();
        let guard_active = self.realtime_guard_active.load(Ordering::Relaxed);
        let is_dynamic = is_dynamic_boost_active();
        let real_fps = get_real_analysis_fps();
        let (video_fps, video_status) = get_analysis_fps_and_status(op_mode, guard_active, boosted, is_dynamic);
        let payload = json!({
            "type": "vision_mode_changed",
            "boosted": boosted,
            "fps_boosted": boosted,
            "dynamic_boost_active": is_dynamic,
            "real_fps": real_fps,
            "video_fps": video_fps,
            "video_status": video_status,
            "analysis_paused": op_mode == 1 || op_mode >= 3 || !guard_active
        });
        let _ = self.broadcast_tx.send(payload.to_string());
    }

    pub fn broadcast_tx(&self) -> broadcast::Sender<String> {
        self.broadcast_tx.clone()
    }

    pub fn make_incident_json(
        id: &str,
        timestamp: &str,
        monitor: &str,
        violation_type: &str,
        target: &str,
        score: f32,
        circumstances: &str,
        status: &str,
        action: &str,
    ) -> serde_json::Value {
        json!({
            "type": "new_incident",
            "incident": {
                "id": id,
                "timestamp": timestamp,
                "monitor": monitor,
                "violation_type": violation_type,
                "target": target,
                "score": score,
                "circumstances": circumstances,
                "status": status,
                "action": action
            }
        })
    }

    pub async fn toggle_emergency_shield(
        obs: Arc<OBSClient>,
        tx_bcast: broadcast::Sender<String>,
        reason: &str,
    ) -> bool {
        let is_currently_active = IS_CENSOR_ACTIVE.load(Ordering::Relaxed)
            || ACTIVE_CENSOR_SOURCES.load(Ordering::Relaxed) != 0
            || obs.is_shield_visible.load(Ordering::Relaxed);

        if is_currently_active {
            println!("[Hotkeys/Panic] Shield is active -> Streamer toggled OFF. Hiding Censor Shield.");
            ACTIVE_CENSOR_SOURCES.store(0, Ordering::SeqCst);
            update_censor_source(&obs, &tx_bcast, CENSOR_SRC_MANUAL, false, "STREAM SAFE").await;

            let res_msg = json!({
                "type": "incident_resolved",
                "incident_type": "PANIC",
                "timestamp": VisionEngine::get_current_timestamp(),
                "message": "Streamer toggled shield off: stream restored safe"
            });
            let _ = tx_bcast.send(res_msg.to_string());
            false
        } else {
            println!("[Hotkeys/Panic] Shield is hidden -> Streamer toggled ON ({}). Showing Censor Shield.", reason);
            set_censor_shield_enabled(true);
            update_censor_source(&obs, &tx_bcast, CENSOR_SRC_MANUAL, true, reason).await;

            let timestamp = VisionEngine::get_current_timestamp();
            let inc_id = format!("manual-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
            let inc_msg = Self::make_incident_json(
                &inc_id,
                &timestamp,
                "Manual / Hotkey",
                "PANIC",
                reason,
                1.0,
                "Emergency shield toggled by user hotkey",
                "ACTIVE",
                "Censor Shield + Mute",
            );
            let _ = tx_bcast.send(inc_msg.to_string());

            let mute_payload = json!({
                "type": "emergency_mute_triggered",
                "duration_ms": 10000
            });
            let _ = tx_bcast.send(mute_payload.to_string());
            true
        }
    }

    pub async fn trigger_censor_with_continuous_guard(
        obs: Arc<OBSClient>,
        vision: Arc<VisionEngine>,
        tx_bcast: broadcast::Sender<String>,
        initial_duration_ms: u64,
        reason: &str,
    ) {
        println!("[CensorGuard] Shield activated: {}. Initial duration: {}ms", reason, initial_duration_ms);
        update_censor_source(&obs, &tx_bcast, CENSOR_SRC_MANUAL, true, reason).await;

        let timestamp = VisionEngine::get_current_timestamp();
        let mon_name = vision.get_selected_monitor_name();
        let inc_id = format!("inc-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
        let inc_msg = Self::make_incident_json(
            &inc_id,
            &timestamp,
            &mon_name,
            if reason.to_uppercase().contains("OCR") || reason.to_uppercase().contains("STOPWORD") || reason.to_uppercase().contains("BANNED") { "OCR" } else { "NSFW" },
            reason,
            0.99,
            &format!("Detected: {}. Protection shield activated in OBS.", reason),
            "ACTIVE (BLUR)",
            "Screen hidden, audio muted",
        );
        let _ = tx_bcast.send(inc_msg.to_string());

        let shield_state = json!({
            "type": "shield_state_update",
            "is_nsfw": true,
            "reason": reason
        });
        let _ = tx_bcast.send(shield_state.to_string());

        let mute_payload = json!({
            "type": "emergency_mute_triggered",
            "duration_ms": initial_duration_ms
        });
        let _ = tx_bcast.send(mute_payload.to_string());

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(initial_duration_ms)).await;

            // Continuous Guard Loop:
            // Since VisionEngine captures the physical Windows screen via Win32 GDI GetDC(0),
            // it is NEVER blinded by the OBS Censor Shield overlay on the broadcast canvas!
            let mut guard_checks = 0;
            const MAX_GUARD_CHECKS: usize = 20; // 10s extra safety limit

            loop {
                let check = vision.analyze_screen_raw();
                if !check.should_censor || guard_checks >= MAX_GUARD_CHECKS {
                    if check.should_censor {
                        println!("[CensorGuard] Max safety guard reached (13s total). Restoring stream.");
                    } else {
                        println!("[CensorGuard] Screen verified clean via physical GDI! Deactivating shield and restoring stream.");
                    }
                    break;
                }

                guard_checks += 1;
                println!(
                    "[CensorGuard] Screen STILL contains explicit content ({})! Extending shield for 500ms (check {}/{})",
                    check.status_message, guard_checks, MAX_GUARD_CHECKS
                );

                let ext_update = json!({
                    "type": "shield_extended",
                    "reason": format!("Screen still contains: {}. Shield extended.", check.status_message),
                    "check_index": guard_checks
                });
                let _ = tx_bcast.send(ext_update.to_string());

                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            // Restore stream
            update_censor_source(&obs, &tx_bcast, CENSOR_SRC_MANUAL, false, "STREAM SAFE").await;

            let res_msg = json!({
                "type": "incident_resolved",
                "timestamp": VisionEngine::get_current_timestamp(),
                "reason": "Screen clean, OBS shield removed"
            });
            let _ = tx_bcast.send(res_msg.to_string());

            println!("[CensorGuard] Visual Censor Shield deactivated, stream restored clean");
        });
    }

    pub async fn start(&self) {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => {
                println!("[WSBridge] Native Rust WebSocket Bridge running on ws://{}", addr);
                l
            }
            Err(e) => {
                eprintln!("[WSBridge] Failed to bind to {}: {}", addr, e);
                return;
            }
        };

        // 1. Telemetry loop (500ms) to broadcast system, GPU and video status
        let tx_telemetry = self.broadcast_tx.clone();
        let obs_ref = self.obs.clone();
        let lex_ref = self.lexical.clone();
        let vis_ref = self.vision.clone();
        let fps_ref = self.fps_boosted.clone();
        let guard_telemetry = self.realtime_guard_active.clone();
        let lookahead_telemetry = self.lookahead.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if tx_telemetry.receiver_count() > 0 {
                    let obs_connected = *obs_ref.is_connected.lock().await;
                    if obs_connected {
                        obs_ref.update_capture_and_video_telemetry().await;
                    }
                    let rules_count = {
                        let eng = lex_ref.lock().await;
                        eng.rules.len()
                    };
                    let boosted = fps_ref.load(Ordering::Relaxed);
                    let op_mode = get_operation_mode();
                    let guard_active = guard_telemetry.load(Ordering::Relaxed);
                    let is_dynamic = is_dynamic_boost_active();
                    let real_fps = get_real_analysis_fps();
                    let (video_fps, video_status) = get_analysis_fps_and_status(op_mode, guard_active, boosted, is_dynamic);
                    let resolution = obs_ref.video_resolution.lock().await.clone();
                    let capture_active = *obs_ref.capture_active.lock().await;
                    let capture_source = obs_ref.capture_source_name.lock().await.clone();
                    let current_scene = obs_ref.selected_scene.lock().await.clone().unwrap_or_default();
                    let scenes = obs_ref.cached_scenes.lock().await.clone();
                    let gpu_info = vis_ref.gpu_name.clone();
                    let directml_ready = vis_ref.directml_available.load(Ordering::Relaxed);

                    let monitors = VisionEngine::enumerate_monitors();
                    let mut selected_monitor = vis_ref.get_selected_monitor();
                    if !monitors.iter().any(|m| m.index == selected_monitor) {
                        let fallback_idx = monitors.iter().find(|m| m.is_primary).map(|m| m.index).unwrap_or(0);
                        vis_ref.set_selected_monitor(fallback_idx);
                        crate::settings::update_cached_settings(|s| s.selected_monitor = fallback_idx);
                        crate::update_tray_screen_monitor(fallback_idx);
                        selected_monitor = fallback_idx;
                    }
                    let cues_count = lookahead_telemetry.get_cues().len();
                    let (p_name, p_time, p_play) = lookahead_telemetry.get_player_sync_state();

                    let obs_plugin_active = vis_ref.cascade.is_obs_plugin_active();
                    let data = json!({
                        "type": "telemetry",
                        "obs_connected": obs_connected,
                        "obs_plugin_active": obs_plugin_active,
                        "rules_count": rules_count,
                        "fps_boosted": boosted,
                        "dynamic_boost_active": is_dynamic,
                        "real_fps": real_fps,
                        "video_fps": video_fps,
                        "video_status": video_status,
                        "analysis_paused": op_mode == 1 || op_mode >= 3 || !guard_active,
                        "obs_resolution": resolution,
                        "capture_active": capture_active,
                        "capture_source": capture_source,
                        "current_scene": current_scene,
                        "scenes": scenes,
                        "gpu_info": gpu_info,
                        "directml_ready": directml_ready,
                        "cuda_ready": directml_ready,
                        "monitors": monitors,
                        "selected_monitor": selected_monitor,
                        "hud_monitor": crate::get_hud_monitor_index(),
                        "realtime_guard": guard_active,
                        "censor_shield_enabled": CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed),
                        "shield_donate_enabled": SHIELD_DONATE_ENABLED.load(Ordering::Relaxed),
                        "ocr_enabled": vis_ref.is_ocr_enabled(),
                        "is_shield_visible": obs_ref.is_shield_visible.load(Ordering::Relaxed),
                        "operation_mode": op_mode,
                        "nsfw_threshold": vis_ref.get_nsfw_threshold(),
                        "nsfw_model_ready": vis_ref.is_model_ready(),
                        "cues_count": cues_count,
                        "player_sync": {
                            "name": p_name,
                            "time": p_time,
                            "is_playing": p_play,
                            "detected": lookahead_telemetry.is_player_active()
                        },
                        "ru_ocr_installed": crate::vision::VisionEngine::is_russian_ocr_installed(),
                        "en_ocr_installed": crate::vision::VisionEngine::is_english_ocr_installed(),
                        "extension_connected": is_browser_extension_connected()
                    });
                    let _ = tx_telemetry.send(data.to_string());
                }
            }
        });

        // 2. Continuous Real-time Stream Surveillance Loop
        let guard_worker_active = self.realtime_guard_active.clone();
        let fps_ref_guard = self.fps_boosted.clone();
        let vis_guard = self.vision.clone();
        let obs_guard = self.obs.clone();
        let tx_guard = self.broadcast_tx.clone();

        tokio::spawn(async move {
            let mut shield_currently_active = false;
            let mut filter_currently_active = false;
            let mut dynamic_escalation_frames: u32 = 0;
            let mut fps_window_start = std::time::Instant::now();
            let mut frames_in_window: u32 = 0;
            let mut last_violation_instant: Option<std::time::Instant> = None;
            const CENSOR_RELEASE_HOLD_SECS: f32 = 2.0;
            println!("[RealtimeGuard] Live neural & OCR stream surveillance worker online");
            obs_guard.set_censor_shield_enabled(false).await;

            loop {
                // If mode is PlayerOnly (1) or Standby/Disabled (>=3), desktop screen capture and OCR loop are completely paused!
                // This drops CPU load to ~0% while streamer is watching player content or has protection disabled!
                let op_mode = OPERATION_MODE.load(Ordering::Relaxed);
                let guard_active = guard_worker_active.load(Ordering::Relaxed);
                if op_mode == 1 || op_mode >= 3 || !guard_active {
                    dynamic_escalation_frames = 0;
                    DYNAMIC_BOOST_ACTIVE.store(false, Ordering::Relaxed);
                    REAL_ANALYSIS_FPS_X10.store(0, Ordering::Relaxed);
                    fps_window_start = std::time::Instant::now();
                    frames_in_window = 0;
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    continue;
                }

                let boosted = fps_ref_guard.load(Ordering::Relaxed);
                let is_dynamic = dynamic_escalation_frames > 0;
                DYNAMIC_BOOST_ACTIVE.store(is_dynamic, Ordering::Relaxed);

                // Safe and optimal frame pacing: 12ms (~50-60 FPS) in danger/boosted, 200ms (~5 FPS) in idle scan
                let sleep_ms = if boosted || is_dynamic { 12 } else { 200 };
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;

                let m_idx = vis_guard.get_selected_monitor();
                let vis_clone = vis_guard.clone();
                let gen_before = MODE_GENERATION.load(Ordering::Relaxed);

                // Run screen capture, neural ViT & NudeNet NSFW inference and OCR in spawn_blocking
                // so it NEVER blocks the Tokio event loop!
                let frame_res = tokio::task::spawn_blocking(move || {
                    vis_clone.analyze_frame_realtime(m_idx)
                }).await;

                let frame_data = match frame_res {
                    Ok(Some(d)) => d,
                    _ => continue,
                };

                let gen_after = MODE_GENERATION.load(Ordering::Relaxed);
                let cur_mode = OPERATION_MODE.load(Ordering::Relaxed);
                // DEFENSIVE RACE CONDITION GUARD: If mode changed while frame was processing, discard immediately!
                if gen_after != gen_before || cur_mode == 1 || cur_mode >= 3 {
                    continue;
                }

                // Calculate real measured FPS across rolling window
                frames_in_window += 1;
                let elapsed_window = fps_window_start.elapsed().as_secs_f32();
                if elapsed_window >= 1.0 {
                    let measured_fps = (frames_in_window as f32) / elapsed_window;
                    REAL_ANALYSIS_FPS_X10.store((measured_fps * 10.0).round() as u32, Ordering::Relaxed);
                    fps_window_start = std::time::Instant::now();
                    frames_in_window = 0;
                }

                // Dynamic 60 FPS Escalation:
                // When Gatekeeper score >= 0.15, or boxes are actively tracked, or censorship is active,
                // escalate instantly to 60 FPS (12ms sleep).
                // Once screen returns to clean, count down 30 frames then return to 200ms idle scan.
                if frame_data.stage1_score >= 0.15 || frame_data.box_count > 0 || frame_data.should_censor || shield_currently_active {
                    dynamic_escalation_frames = 30;
                } else if dynamic_escalation_frames > 0 {
                    dynamic_escalation_frames -= 1;
                }
                let is_dynamic_now = dynamic_escalation_frames > 0;
                DYNAMIC_BOOST_ACTIVE.store(is_dynamic_now, Ordering::Relaxed);

                let real_fps = get_real_analysis_fps();
                let (target_fps, video_status) = get_analysis_fps_and_status(cur_mode, true, boosted, is_dynamic_now);

                // 1. Broadcast live metrics to UI on EVERY single frame
                let live_msg = json!({
                    "type": "live_frame_metrics",
                    "monitor_index": frame_data.monitor_index,
                    "monitor_name": frame_data.monitor_name,
                    "nsfw_category": frame_data.nsfw_category,
                    "nsfw_score": frame_data.nsfw_score,
                    "nsfw_pct": format!("{:.1}%", frame_data.nsfw_score * 100.0),
                    "nsfw_label": frame_data.nsfw_label,
                    "nsfw_violation": frame_data.nsfw_violation,
                    "stage1_score": frame_data.stage1_score,
                    "stage1_pct": format!("{:.1}%", frame_data.stage1_score * 100.0),
                    "stage1_label": frame_data.stage1_label,
                    "stage2_score": frame_data.stage2_score,
                    "stage2_pct": format!("{:.1}%", frame_data.stage2_score * 100.0),
                    "obs_plugin_active": frame_data.obs_plugin_active,
                    "box_count": frame_data.box_count,
                    "ocr_snippet": frame_data.ocr_snippet,
                    "ocr_violation": frame_data.ocr_violation,
                    "banned_words": frame_data.banned_words,
                    "censor_active": shield_currently_active || frame_data.should_censor,
                    "censor_reason": frame_data.reason,
                    "timestamp": frame_data.timestamp,
                    "real_fps": real_fps,
                    "video_fps": target_fps,
                    "video_status": video_status,
                    "fps_boosted": boosted,
                    "dynamic_boost_active": is_dynamic_now
                });
                let _ = tx_guard.send(live_msg.to_string());

                // 2. Dynamic OBS Source Control (Calm vs Violation with Hysteresis Hold-Time)
                if frame_data.should_censor {
                    last_violation_instant = Some(std::time::Instant::now());
                    LAST_ACTIVE_CENSOR_SCORE.store((frame_data.nsfw_score * 100.0).round() as u32, Ordering::Relaxed);
                    if frame_data.nsfw_violation {
                        update_censor_source(&obs_guard, &tx_guard, CENSOR_SRC_SCREEN_NSFW, true, &frame_data.reason).await;
                    }
                    if frame_data.ocr_violation {
                        update_censor_source(&obs_guard, &tx_guard, CENSOR_SRC_SCREEN_OCR, true, &frame_data.reason).await;
                    }

                    if !shield_currently_active {
                        shield_currently_active = true;
                        let inc_id = format!("inc-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis());
                        let inc_msg = Self::make_incident_json(
                            &inc_id,
                            &frame_data.timestamp,
                            &frame_data.monitor_name,
                            if frame_data.nsfw_violation { "NSFW" } else { "OCR" },
                            &frame_data.reason,
                            frame_data.nsfw_score,
                            &format!("Detected: {}. Stream protection active.", frame_data.reason),
                            "ACTIVE",
                            if CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed) { "Censor Shield + Mute" } else { "Selective Blur" },
                        );
                        let _ = tx_guard.send(inc_msg.to_string());
                    }

                    // Native OBS GPU Blur Plugin (runs on source):
                    if frame_data.obs_plugin_active {
                        if !filter_currently_active {
                            filter_currently_active = true;
                        }
                        if frame_data.box_count == 0 {
                            vis_guard.cascade.trigger_obs_full_censor(&frame_data.reason);
                        }
                    }
                } else {
                    // CALM / SAFE SCREEN
                    // Hysteresis release check: only release if screen has remained continuously clean for CENSOR_RELEASE_HOLD_SECS
                    let hold_elapsed = last_violation_instant.map(|t| t.elapsed().as_secs_f32()).unwrap_or(999.0);
                    if shield_currently_active && hold_elapsed >= CENSOR_RELEASE_HOLD_SECS {
                        shield_currently_active = false;
                        last_violation_instant = None;
                        update_censor_source(&obs_guard, &tx_guard, CENSOR_SRC_SCREEN_NSFW, false, "STREAM SAFE").await;
                        update_censor_source(&obs_guard, &tx_guard, CENSOR_SRC_SCREEN_OCR, false, "STREAM SAFE").await;

                        let res_msg = json!({
                            "type": "incident_resolved",
                            "timestamp": frame_data.timestamp,
                            "message": "Screen clean: stream safe, shield removed"
                        });
                        let _ = tx_guard.send(res_msg.to_string());

                        if filter_currently_active {
                            filter_currently_active = false;
                            vis_guard.clear_obs_censor();
                        }
                    }
                }
            }
        });

        // 3. Lookahead Scheduled Cues Polling Worker (25ms precision)
        let lookahead_poll = self.lookahead.clone();
        let obs_lookahead = self.obs.clone();
        let vis_lookahead = self.vision.clone();
        let tx_lookahead = self.broadcast_tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(25)).await;
                let op_mode = OPERATION_MODE.load(Ordering::Relaxed);
                if op_mode == 2 || op_mode >= 3 {
                    // ScreenOnly or Standby/Disabled: skip lookahead due cues!
                    continue;
                }
                let gen_before = MODE_GENERATION.load(Ordering::Relaxed);
                let due_tickets = lookahead_poll.poll_due_cues();
                for ticket in due_tickets {
                    if ticket.ticket_id.starts_with("test_") {
                        // Test ticket expired naturally - hide HUD only if this exact test ticket is still active
                        crate::hide_lookahead_hud_if_ticket(&ticket.ticket_id);
                        let notify = json!({
                            "type": "lookahead_cue_executed",
                            "ticket_id": ticket.ticket_id,
                            "reason": ticket.reason
                        });
                        let _ = tx_lookahead.send(notify.to_string());
                        continue;
                    }

                    if ticket.ticket_id.starts_with("cue-warn-") {
                        // Scheduled cue warning countdown finished in HUD.
                        // Hide HUD if this warning ticket is still active.
                        // Actual censorship is managed by handle_player_sync with CENSOR_SRC_SCHEDULED_CUE.
                        crate::hide_lookahead_hud_if_ticket(&ticket.ticket_id);
                        let notify = json!({
                            "type": "lookahead_ticket_resolved",
                            "ticket_id": ticket.ticket_id,
                            "action": "due"
                        });
                        let _ = tx_lookahead.send(notify.to_string());
                        continue;
                    }

                    println!("[LookaheadScheduler] Scheduled cue reached playback time: {}. Triggering OBS protection!", ticket.reason);
                    let obs_l = obs_lookahead.clone();
                    let tx_l = tx_lookahead.clone();
                    let reason = ticket.reason.clone();
                    tokio::spawn(async move {
                        let cur_gen = MODE_GENERATION.load(Ordering::Relaxed);
                        let cur_mode = OPERATION_MODE.load(Ordering::Relaxed);
                        if cur_gen == gen_before && (cur_mode == 0 || cur_mode == 1) {
                            update_censor_source(&obs_l, &tx_l, CENSOR_SRC_LOOKAHEAD_DUE, true, &reason).await;
                            tokio::time::sleep(Duration::from_millis(3500)).await;
                            update_censor_source(&obs_l, &tx_l, CENSOR_SRC_LOOKAHEAD_DUE, false, "STREAM SAFE").await;
                        }
                    });
                    if ticket.boxes.is_empty() {
                        vis_lookahead.cascade.trigger_obs_full_censor(&ticket.reason);
                    }

                    crate::hide_lookahead_hud_if_ticket(&ticket.ticket_id);
                    let notify = json!({
                        "type": "lookahead_cue_executed",
                        "ticket_id": ticket.ticket_id,
                        "reason": ticket.reason
                    });
                    let _ = tx_lookahead.send(notify.to_string());
                }
            }
        });

        // 4. Accept incoming WebSocket connections
        let lexical_shared = self.lexical.clone();
        let obs_shared = self.obs.clone();
        let vision_shared = self.vision.clone();
        let fps_boosted_shared = self.fps_boosted.clone();
        let guard_shared_main = self.realtime_guard_active.clone();
        let lookahead_shared_main = self.lookahead.clone();
        let tx_main = self.broadcast_tx.clone();

        while let Ok((stream, _)) = listener.accept().await {
            let lex = lexical_shared.clone();
            let obs_client = obs_shared.clone();
            let vision_client = vision_shared.clone();
            let fps_shared = fps_boosted_shared.clone();
            let guard_shared = guard_shared_main.clone();
            let lookahead_client = lookahead_shared_main.clone();
            let mut bcast_rx = tx_main.subscribe();
            let tx_bcast = tx_main.clone();

            tokio::spawn(async move {
                if let Ok(ws_stream) = accept_async(stream).await {
                    let (sink, mut reader) = ws_stream.split();
                    let sink_mutex = Arc::new(Mutex::new(sink));

                    // Forward broadcast channel to WebSocket sink with resilient lag recovery
                    let sink_for_bcast = sink_mutex.clone();
                    tokio::spawn(async move {
                        loop {
                            match bcast_rx.recv().await {
                                Ok(msg_str) => {
                                    let mut s = sink_for_bcast.lock().await;
                                    if s.send(Message::Text(msg_str.into())).await.is_err() {
                                        break;
                                    }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                    // Telemetry or events lagged; recover and keep streaming
                                    continue;
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                    break;
                                }
                            }
                        }
                    });

                    // Process incoming client messages
                    while let Some(Ok(msg)) = reader.next().await {
                        if let Message::Text(text) = msg {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                let msg_type = val["type"].as_str().unwrap_or("");
                                let client_type = val["client"].as_str().unwrap_or("");
                                if client_type == "browser" || msg_type == "captions" || msg_type == "player_sync" || msg_type == "lookahead_frame" || msg_type == "heartbeat" {
                                    record_browser_extension_pulse();
                                }

                                if msg_type == "register" {
                                    let rules_text = {
                                        let engine = lex.lock().await;
                                        engine.export_words_to_text()
                                    };
                                    let rules_count = {
                                        let engine = lex.lock().await;
                                        engine.rules.len()
                                    };
                                    let obs_conn = *obs_client.is_connected.lock().await;
                                    let monitors = VisionEngine::enumerate_monitors();
                                    let mut selected_monitor = vision_client.get_selected_monitor();
                                    if !monitors.iter().any(|m| m.index == selected_monitor) {
                                        let fallback_idx = monitors.iter().find(|m| m.is_primary).map(|m| m.index).unwrap_or(0);
                                        vision_client.set_selected_monitor(fallback_idx);
                                        crate::settings::update_cached_settings(|s| s.selected_monitor = fallback_idx);
                                        crate::update_tray_screen_monitor(fallback_idx);
                                        selected_monitor = fallback_idx;
                                    }
                                    let (p_name, p_time, p_play) = lookahead_client.get_player_sync_state();
                                    let resp = json!({
                                        "type": "init_state",
                                        "status": "ok",
                                        "rules_text": rules_text,
                                        "rules_count": rules_count,
                                        "obs_connected": obs_conn,
                                        "gpu_info": vision_client.gpu_name.clone(),
                                        "directml_ready": vision_client.directml_available.load(Ordering::Relaxed),
                                        "cuda_ready": vision_client.directml_available.load(Ordering::Relaxed),
                                        "monitors": monitors,
                                        "selected_monitor": selected_monitor,
                                        "hud_monitor": crate::get_hud_monitor_index(),
                                        "realtime_guard": guard_shared.load(Ordering::Relaxed),
                                        "censor_shield_enabled": CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed),
                                        "shield_donate_enabled": SHIELD_DONATE_ENABLED.load(Ordering::Relaxed),
                                        "ocr_enabled": vision_client.is_ocr_enabled(),
                                        "is_shield_visible": obs_client.is_shield_visible.load(Ordering::Relaxed),
                                        "is_censor_active": is_censor_active(),
                                        "active_censor_reason": get_current_censor_reason(),
                                        "active_censor_score": get_current_censor_score(),
                                        "active_cue": lookahead_client.get_active_cue_info(),
                                        "operation_mode": get_operation_mode(),
                                        "extension_connected": is_browser_extension_connected(),
                                        "lookahead_preview_visible": crate::is_lookahead_preview_window_visible(),
                                        "player_sync": {
                                            "name": p_name,
                                            "time": p_time,
                                            "is_playing": p_play,
                                            "detected": lookahead_client.is_player_active()
                                        },
                                        "nsfw_threshold": vision_client.get_nsfw_threshold(),
                                        "nsfw_model_ready": vision_client.is_model_ready(),
                                        "ru_ocr_installed": crate::vision::VisionEngine::is_russian_ocr_installed(),
                                        "en_ocr_installed": crate::vision::VisionEngine::is_english_ocr_installed(),
                                        "hotkey_scope": crate::settings::get_cached_settings().hotkey_scope,
                                        "hotkey_panic": crate::settings::get_cached_settings().hotkey_panic,
                                        "hotkey_threat": crate::settings::get_cached_settings().hotkey_threat,
                                        "models_status": crate::downloader::ModelDownloader::check_models_status(),
                                        "censor_categories": vision_client.get_censor_categories(),
                                        "cues_count": lookahead_client.get_cues().len(),
                                        "scheduled_cues": lookahead_client.get_cues(),
                                        "cues_config": {
                                            "pre_warning_seconds": lookahead_client.get_pre_warning_seconds(),
                                            "auto_censor": lookahead_client.get_auto_censor(),
                                            "append_mode": lookahead_client.get_append_cues_mode()
                                        },
                                        "ocr_status": crate::downloader::ModelDownloader::check_ocr_status(),
                                        "hide_setup_guide": crate::settings::get_cached_settings().hide_setup_guide,
                                        "language": crate::settings::get_cached_settings().language,
                                        "gpu_info": vision_client.gpu_name.clone(),
                                        "events": [
                                            {
                                                "type": "system",
                                                "timestamp": "00:00:00",
                                                "message": format!("blewred Rust Core active • Accelerator: {}", vision_client.gpu_name)
                                            }
                                        ]
                                    });
                                    let mut s = sink_mutex.lock().await;
                                    let _ = s.send(Message::Text(resp.to_string().into())).await;
                                } else if msg_type == "set_language" {
                                    if let Some(lang) = val["language"].as_str() {
                                        let clean_lang = if lang == "en" { "en" } else { "ru" };
                                        crate::settings::update_cached_settings(|s| {
                                            s.language = clean_lang.to_string();
                                        });
                                        let bcast = json!({
                                            "type": "language_changed",
                                            "language": clean_lang
                                        });
                                        let _ = tx_bcast.send(bcast.to_string());
                                        println!("[WSBridge] Language updated and broadcasted to clients: {}", clean_lang);
                                    }
                                } else if msg_type == "toggle_censor_shield" {
                                    let enabled = val["enabled"].as_bool().unwrap_or(true);
                                    let obs_ref = obs_client.clone();
                                    let tx_b = tx_bcast.clone();
                                    tokio::spawn(async move {
                                        set_censor_shield_enabled_with_sync(&obs_ref, &tx_b, enabled).await;
                                    });
                                } else if msg_type == "toggle_shield_donate" {
                                    let enabled = val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                                    let obs_ref = obs_client.clone();
                                    let tx_b = tx_bcast.clone();
                                    tokio::spawn(async move {
                                        set_shield_donate_and_sync(&obs_ref, &tx_b, enabled).await;
                                    });
                                } else if msg_type == "toggle_ocr" {
                                    let enabled = val["enabled"].as_bool().unwrap_or(true);
                                    vision_client.set_ocr_enabled(enabled);
                                    crate::settings::update_cached_settings(|s| s.ocr_enabled = enabled);
                                    println!("[WSBridge] OCR text surveillance toggled: enabled = {}", enabled);
                                    let resp = json!({
                                        "type": "ocr_state_changed",
                                        "enabled": enabled
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "toggle_realtime_guard" {
                                    let enabled = val["enabled"].as_bool().unwrap_or(true);
                                    guard_shared.store(enabled, Ordering::Relaxed);
                                    println!("[WSBridge] Realtime Guard state changed: enabled = {}", enabled);
                                    let resp = json!({
                                        "type": "realtime_guard_changed",
                                        "enabled": enabled
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "set_nsfw_threshold" {
                                    let threshold = val["threshold"].as_u64().unwrap_or(65) as u32;
                                    vision_client.set_nsfw_threshold(threshold);
                                    let resp = json!({
                                        "type": "nsfw_threshold_changed",
                                        "threshold": threshold
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "set_censor_categories" {
                                    let cats = if let Some(c) = val.get("categories") {
                                        serde_json::from_value::<crate::cascade::CensorCategories>(c.clone())
                                            .unwrap_or_default()
                                    } else {
                                        crate::cascade::CensorCategories::default()
                                    };
                                    vision_client.set_censor_categories(cats.clone());
                                    crate::settings::update_cached_settings(|s| s.censor_categories = cats.clone());
                                    let resp = json!({
                                        "type": "censor_categories_changed",
                                        "categories": cats
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "set_monitor" {
                                    let raw_idx = val["monitor_index"].as_i64().unwrap_or(0) as i32;
                                    let monitors = VisionEngine::enumerate_monitors();
                                    let monitor_idx = if monitors.iter().any(|m| m.index == raw_idx) {
                                        raw_idx
                                    } else {
                                        monitors.iter().find(|m| m.is_primary).map(|m| m.index).unwrap_or(0)
                                    };
                                    vision_client.set_selected_monitor(monitor_idx);
                                    crate::settings::update_cached_settings(|s| s.selected_monitor = monitor_idx);
                                    crate::update_tray_screen_monitor(monitor_idx);
                                    let resp = json!({
                                        "type": "monitor_changed",
                                        "selected_monitor": monitor_idx
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "set_hud_monitor" {
                                    let raw_idx = val["monitor_index"].as_i64().unwrap_or(-1) as i32;
                                    let monitors = VisionEngine::enumerate_monitors();
                                    let mon_idx = if raw_idx == -1 || monitors.iter().any(|m| m.index == raw_idx) {
                                        raw_idx
                                    } else {
                                        -1
                                    };
                                    crate::set_hud_monitor_index(mon_idx);
                                    crate::settings::update_cached_settings(|s| s.hud_monitor = mon_idx);
                                    crate::update_tray_hud_monitor(mon_idx);
                                    let resp = json!({
                                        "type": "hud_monitor_changed",
                                        "hud_monitor": mon_idx
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "set_operation_mode" {
                                    let mode = val["mode"].as_u64()
                                        .or_else(|| val["mode"].as_str().and_then(|s| s.parse::<u64>().ok()))
                                        .unwrap_or(0) as u8;
                                    safe_set_operation_mode(mode, &obs_client, &tx_bcast, &lookahead_client, &vision_client).await;
                                } else if msg_type == "get_operation_mode" {
                                    let resp = json!({
                                        "type": "operation_mode_changed",
                                        "operation_mode": get_operation_mode()
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "test_lookahead_alert" {
                                    // Reset previous test state completely
                                    lookahead_client.clear_test_tickets();
                                    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
                                    let mock_ticket = crate::lookahead::LookaheadIncidentTicket {
                                        ticket_id: format!("test_{}", now_ms),
                                        player_type: "Alloha (Demo)".to_string(),
                                        eta_seconds: 8.4,
                                        severity: "high".to_string(),
                                        reason: "Exposed female breast (Test)".to_string(),
                                        score: 0.89,
                                        raw_image_base64: get_realistic_test_thumbnail(),
                                        boxes: vec![crate::cascade::NormalizedBox {
                                            x1: 0.28,
                                            y1: 0.20,
                                            x2: 0.72,
                                            y2: 0.75,
                                            class_id: 3,
                                            label: "FEMALE_BREAST_EXPOSED".to_string(),
                                            score: 0.89,
                                        }],
                                        created_at_ms: now_ms,
                                        execute_at_ms: now_ms + 8400,
                                        status: "pending".to_string(),
                                    };
                                    lookahead_client.record_ticket(mock_ticket.clone());
                                    crate::show_lookahead_hud(mock_ticket.clone());
                                    let resp = json!({
                                        "type": "lookahead_ticket",
                                        "ticket": mock_ticket
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "hide_hud" {
                                    lookahead_client.clear_test_tickets();
                                    crate::hide_lookahead_hud();
                                } else if msg_type == "update_rules" {
                                    let text = val["text"].as_str().unwrap_or("");
                                    let count = {
                                        let mut engine = lex.lock().await;
                                        engine.clear();
                                        engine.load_words_from_text(text);
                                        engine.rules.len()
                                    };
                                    println!("[WSBridge] Stopwords updated via UI: {} rules active", count);
                                    let resp = json!({
                                        "type": "rules_updated",
                                        "count": count
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "captions" {
                                    let cur_mode = get_operation_mode();
                                    if cur_mode == 2 || cur_mode >= 3 {
                                        continue;
                                    }
                                    let current_time = val["current_time"].as_f64().unwrap_or(0.0);
                                    let platform = val["platform"].as_str().unwrap_or("browser");

                                    if let Some(cues) = val["cues"].as_array() {
                                        for c in cues {
                                            let cue_text = c["text"].as_str().unwrap_or("");
                                            let cue_start = c["start"].as_f64().unwrap_or(current_time);
                                            let cue_end = c["end"].as_f64().unwrap_or(cue_start + 2.0);

                                            let matches = {
                                                let engine = lex.lock().await;
                                                engine.check_text(cue_text)
                                            };

                                            if !matches.is_empty() {
                                                let word = matches[0].word.clone();
                                                let rule = matches[0].rule.clone();
                                                let lead_time = (cue_start - current_time).max(0.0);
                                                let duration_ms = ((cue_end - cue_start + 0.4) * 1000.0).max(1200.0) as u64;

                                                println!(
                                                    "[WSBridge] [{}] Banned word '{}' in future captions! Muting in {:.2}s for {}ms",
                                                    platform, word, lead_time, duration_ms
                                                );

                                                let alert_payload = json!({
                                                    "type": "banned_audio_spotted",
                                                    "source": "desktop",
                                                    "word": word,
                                                    "rule": rule,
                                                    "is_banned": true,
                                                    "matches": [word]
                                                });
                                                let _ = tx_bcast.send(alert_payload.to_string());

                                                let gen_before = MODE_GENERATION.load(Ordering::Relaxed);
                                                let obs_ref = obs_client.clone();
                                                tokio::spawn(async move {
                                                    if lead_time > 0.05 {
                                                        tokio::time::sleep(Duration::from_secs_f64(lead_time)).await;
                                                    }
                                                    let cur_gen = MODE_GENERATION.load(Ordering::Relaxed);
                                                    let cur_mode = get_operation_mode();
                                                    if cur_gen == gen_before && (cur_mode == 0 || cur_mode == 1) {
                                                        obs_ref.mute_input("Desktop Audio", duration_ms).await;
                                                    }
                                                });
                                            }
                                        }
                                    }
                                } else if msg_type == "player_sync" {
                                    let player_name = val["player_name"].as_str().unwrap_or("Web Player");
                                    let current_time = val["current_time"].as_f64().unwrap_or(0.0);
                                    let duration = val["duration"].as_f64().unwrap_or(0.0);
                                    let is_playing = val["is_playing"].as_bool().unwrap_or(false);

                                    let sync_res = lookahead_client.handle_player_sync(player_name, current_time, duration, is_playing);

                                    let cur_mode = get_operation_mode();
                                    if cur_mode == 2 || cur_mode >= 3 {
                                        continue;
                                    }

                                    // 1. Warning window crossed: show HUD alert
                                    if let Some(ticket) = sync_res.warning_ticket {
                                        println!("[WSBridge] Scheduled Cue Warning: {} (ETA: {:.1}s)", ticket.reason, ticket.eta_seconds);
                                        if lookahead_client.is_notifications_enabled() {
                                            crate::show_lookahead_hud(ticket.clone());
                                        }
                                        let ticket_msg = json!({
                                            "type": "lookahead_ticket",
                                            "ticket": ticket
                                        });
                                        let _ = tx_bcast.send(ticket_msg.to_string());
                                    }

                                    // 2. Playhead entered active cue: trigger OBS censor
                                    if sync_res.should_censor_now {
                                        println!("[WSBridge] Scheduled Cue started! Triggering OBS censorship for: {:?}", sync_res.active_cue_reason);
                                        let obs_c = obs_client.clone();
                                        let tx_b = tx_bcast.clone();
                                        let reason = sync_res.active_cue_reason.clone();
                                        tokio::spawn(async move {
                                            update_censor_source(&obs_c, &tx_b, CENSOR_SRC_SCHEDULED_CUE, true, &reason).await;
                                        });
                                        crate::hide_lookahead_hud();

                                        let cue_id = sync_res.active_cue_id.clone().unwrap_or_else(|| "cue".to_string());
                                        let range = sync_res.active_cue_range.clone();
                                        let reason_text = sync_res.active_cue_reason.clone();
                                        let display_target = if !range.is_empty() {
                                            format!("{} • {}", range, reason_text)
                                        } else {
                                            reason_text.clone()
                                        };
                                        let display_circ = if !range.is_empty() {
                                            format!("Scheduled player censor ({}): {}. Stream protection active.", range, reason_text)
                                        } else {
                                            format!("Scheduled player censor: {}. Stream protection active.", reason_text)
                                        };
                                        let now_ts = {
                                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                                            let secs = now.as_secs();
                                            let h = (secs / 3600) % 24;
                                            let m = (secs / 60) % 60;
                                            let s = secs % 60;
                                            format!("{:02}:{:02}:{:02}", h, m, s)
                                        };

                                        let inc_data = json!({
                                            "id": format!("cue-{}", cue_id),
                                            "timestamp": now_ts,
                                            "monitor": format!("Player ({})", player_name),
                                            "violation_type": "CUE",
                                            "target": display_target,
                                            "score": 1.0,
                                            "circumstances": display_circ,
                                            "status": "ACTIVE",
                                            "action": if CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed) { "Censor Shield + Mute" } else { "Screen Blocking + Mute" }
                                        });

                                        let rem = sync_res.remaining_sec.unwrap_or(0.0);
                                        let total = sync_res.total_duration_sec.unwrap_or(0.0);
                                        let bcast = json!({
                                            "type": "scheduled_cue_started",
                                            "cue_id": sync_res.active_cue_id,
                                            "reason": sync_res.active_cue_reason,
                                            "range": sync_res.active_cue_range,
                                            "player_name": player_name,
                                            "remaining_sec": rem,
                                            "total_sec": total,
                                            "incident": inc_data
                                        });
                                        let _ = tx_bcast.send(bcast.to_string());
                                    }

                                    // 2.5 Broadcast live progress whenever playhead is within an active scheduled cue
                                    if let Some(ref active_id) = sync_res.active_cue_id {
                                        // Ensure OBS censorship is active for scheduled cues
                                        if (ACTIVE_CENSOR_SOURCES.load(Ordering::Relaxed) & CENSOR_SRC_SCHEDULED_CUE) == 0 {
                                            let obs_c = obs_client.clone();
                                            let tx_b = tx_bcast.clone();
                                            let reason = sync_res.active_cue_reason.clone();
                                            tokio::spawn(async move {
                                                update_censor_source(&obs_c, &tx_b, CENSOR_SRC_SCHEDULED_CUE, true, &reason).await;
                                            });
                                        }

                                        let rem = sync_res.remaining_sec.unwrap_or(0.0);
                                        let total = sync_res.total_duration_sec.unwrap_or(0.0);
                                        let prog_bcast = json!({
                                            "type": "scheduled_cue_progress",
                                            "cue_id": active_id,
                                            "reason": sync_res.active_cue_reason,
                                            "range": sync_res.active_cue_range,
                                            "player_name": player_name,
                                            "remaining_sec": rem,
                                            "total_sec": total,
                                            "current_time": current_time,
                                            "is_playing": is_playing
                                        });
                                        let _ = tx_bcast.send(prog_bcast.to_string());
                                    }

                                    // 3. Playhead exited active cue: uncensor OBS
                                    if sync_res.should_uncensor_now {
                                        println!("[WSBridge] Scheduled Cue ended. Releasing OBS censorship.");
                                        let obs_c = obs_client.clone();
                                        let tx_b = tx_bcast.clone();
                                        tokio::spawn(async move {
                                            update_censor_source(&obs_c, &tx_b, CENSOR_SRC_SCHEDULED_CUE, false, "STREAM SAFE").await;
                                        });
                                        let now_ts = {
                                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                                            let secs = now.as_secs();
                                            let h = (secs / 3600) % 24;
                                            let m = (secs / 60) % 60;
                                            let s = secs % 60;
                                            format!("{:02}:{:02}:{:02}", h, m, s)
                                        };
                                        let bcast = json!({
                                            "type": "scheduled_cue_ended",
                                            "cue_id": sync_res.ended_cue_id.or(sync_res.active_cue_id),
                                            "timestamp": now_ts
                                        });
                                        let _ = tx_bcast.send(bcast.to_string());
                                    }
                                } else if msg_type == "lookahead_frame" {
                                    let cur_mode = get_operation_mode();
                                    if cur_mode == 2 || cur_mode >= 3 {
                                        // Mode is ScreenOnly or Standby/Disabled: ignore browser lookahead frames
                                        continue;
                                    }
                                    let player_id = val["player_id"].as_str().unwrap_or("player").to_string();
                                    let player_type = val["player_type"].as_str().unwrap_or("Web Player").to_string();
                                    let cur_time = val["current_time"].as_f64().unwrap_or(0.0);
                                    let look_time = val["lookahead_time"].as_f64().unwrap_or(cur_time + 8.0);
                                    let w = val["width"].as_u64().unwrap_or(640) as u32;
                                    let h = val["height"].as_u64().unwrap_or(360) as u32;
                                    let img_b64 = val["image"].as_str().unwrap_or("").to_string();

                                    if !img_b64.is_empty() {
                                        let packet = crate::lookahead::LookaheadFramePacket {
                                            player_id,
                                            player_type,
                                            current_time_sec: cur_time,
                                            lookahead_time_sec: look_time,
                                            width: w,
                                            height: h,
                                            image_base64: img_b64,
                                        };
                                        let lookahead_ref = lookahead_client.clone();
                                        let tx_ticket = tx_bcast.clone();
                                        tokio::task::spawn_blocking(move || {
                                            let (maybe_ticket, preview_opt) = lookahead_ref.process_frame_with_preview(packet);
                                            if let Some(preview) = preview_opt {
                                                if crate::is_lookahead_preview_window_visible() {
                                                    crate::emit_lookahead_preview_frame(&preview);
                                                    let prev_msg = json!({
                                                        "type": "lookahead_preview_frame",
                                                        "preview": preview
                                                    });
                                                    let _ = tx_ticket.send(prev_msg.to_string());
                                                }
                                            }
                                            if let Some(ticket) = maybe_ticket {
                                                println!("[WSBridge] Lookahead Warning Ticket generated: {} (ETA: {:.1}s) - Reason: {}", 
                                                    ticket.severity, ticket.eta_seconds, ticket.reason);
                                                if lookahead_ref.is_notifications_enabled() {
                                                    crate::show_lookahead_hud(ticket.clone());
                                                }
                                                let ticket_msg = json!({
                                                    "type": "lookahead_ticket",
                                                    "ticket": ticket
                                                });
                                                let _ = tx_ticket.send(ticket_msg.to_string());
                                            }
                                        });
                                    }
                                } else if msg_type == "lookahead_action" {
                                    let ticket_id = val["ticket_id"].as_str().unwrap_or("").to_string();
                                    let action = val["action"].as_str().unwrap_or("block");
                                    if let Some(_t) = lookahead_client.resolve_ticket(&ticket_id, action) {
                                        crate::hide_lookahead_hud_if_ticket(&ticket_id);
                                        println!("[WSBridge] Streamer decided on {}: action = {}", ticket_id, action);
                                        if action == "block" && !ticket_id.starts_with("test_") {
                                            let obs_c = obs_client.clone();
                                            let tx_b = tx_bcast.clone();
                                            let reason = _t.reason.clone();
                                            let player_type = _t.player_type.clone();
                                            let now_ts = VisionEngine::get_current_timestamp_hms();
                                            if ticket_id.starts_with("cue-warn-") {
                                                let cue_id = ticket_id["cue-warn-".len()..].to_string();
                                                println!("[WSBridge] Scheduled Cue {} blocked by user! Activating continuous OBS protection.", cue_id);
                                                let reason_c = reason.clone();
                                                tokio::spawn(async move {
                                                    update_censor_source(&obs_c, &tx_b, CENSOR_SRC_SCHEDULED_CUE, true, &reason_c).await;
                                                });
                                                let inc_id = format!("cue-{}", cue_id);
                                                let inc_msg = Self::make_incident_json(
                                                    &inc_id,
                                                    &now_ts,
                                                    &format!("Player ({})", player_type),
                                                    "CUE",
                                                    &reason,
                                                    1.0,
                                                    &format!("Blocked by user: {}. Stream protection active.", reason),
                                                    "ACTIVE",
                                                    if CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed) { "Censor Shield + Mute" } else { "Screen Blocking + Mute" },
                                                );
                                                let _ = tx_bcast.send(inc_msg.to_string());
                                                let active_info = lookahead_client.get_active_cue_info();
                                                let (cue_range, rem_sec, tot_sec) = if let Some(info) = active_info {
                                                    (info.range, info.remaining_sec, info.total_duration_sec)
                                                 } else if let Some(cue) = lookahead_client.get_cues().iter().find(|c| c.id == cue_id) {
                                                     let cur_t = lookahead_client.get_player_sync_state().1;
                                                     let rem = (cue.end_sec - cur_t).max(0.0);
                                                     let tot = (cue.end_sec - cue.start_sec).max(0.1);
                                                     (cue.formatted_range.clone(), rem, tot)
                                                 } else {
                                                     ("".to_string(), 0.0, 0.0)
                                                 };

                                                 let bcast = json!({
                                                     "type": "scheduled_cue_started",
                                                     "cue_id": cue_id,
                                                     "reason": reason,
                                                     "range": cue_range,
                                                     "player_name": player_type,
                                                     "remaining_sec": rem_sec,
                                                     "total_sec": tot_sec,
                                                     "incident": inc_msg["incident"]
                                                 });
                                                 let _ = tx_bcast.send(bcast.to_string());
                                             } else {
                                                 let inc_id = format!("lookahead-{}", ticket_id);
                                                 let inc_msg = Self::make_incident_json(
                                                     &inc_id,
                                                     &now_ts,
                                                     &format!("Player ({})", player_type),
                                                     "LOOKAHEAD",
                                                     &reason,
                                                     _t.score,
                                                     &format!("Preemptive blocking by streamer: {}. Stream protection active.", reason),
                                                     "ACTIVE",
                                                     if CENSOR_SHIELD_ENABLED.load(Ordering::Relaxed) { "Censor Shield + Mute" } else { "Screen Blocking + Mute" },
                                                 );
                                                 let _ = tx_bcast.send(inc_msg.to_string());

                                                let ticket_id_res = ticket_id.clone();
                                                tokio::spawn(async move {
                                                    update_censor_source(&obs_c, &tx_b, CENSOR_SRC_LOOKAHEAD_DUE, true, &reason).await;
                                                    tokio::time::sleep(Duration::from_millis(3500)).await;
                                                    update_censor_source(&obs_c, &tx_b, CENSOR_SRC_LOOKAHEAD_DUE, false, "STREAM SAFE").await;
                                                    let res_msg = json!({
                                                        "type": "incident_resolved",
                                                        "incident_id": format!("lookahead-{}", ticket_id_res),
                                                        "incident_type": "LOOKAHEAD",
                                                        "message": "Preemptive blocking completed, screen restored"
                                                    });
                                                    let _ = tx_b.send(res_msg.to_string());
                                                });
                                            }
                                        }
                                        let update = json!({
                                            "type": "lookahead_ticket_resolved",
                                            "ticket_id": ticket_id,
                                            "action": action
                                        });
                                        let _ = tx_bcast.send(update.to_string());
                                    }
                                } else if msg_type == "toggle_lookahead_preview" {
                                    let is_open = crate::toggle_lookahead_preview_window();
                                    let bcast = json!({
                                        "type": "lookahead_preview_state",
                                        "is_open": is_open
                                    });
                                    let _ = tx_bcast.send(bcast.to_string());
                                } else if msg_type == "open_lookahead_preview" {
                                    crate::show_lookahead_preview_window();
                                    let bcast = json!({
                                        "type": "lookahead_preview_state",
                                        "is_open": true
                                    });
                                    let _ = tx_bcast.send(bcast.to_string());
                                } else if msg_type == "close_lookahead_preview" {
                                    crate::hide_lookahead_preview_window();
                                    let bcast = json!({
                                        "type": "lookahead_preview_state",
                                        "is_open": false
                                    });
                                    let _ = tx_bcast.send(bcast.to_string());
                                } else if msg_type == "get_lookahead_preview" {
                                    if let Some(prev) = lookahead_client.get_latest_preview() {
                                        let msg = json!({
                                            "type": "lookahead_preview_frame",
                                            "preview": prev
                                        });
                                        let mut s = sink_mutex.lock().await;
                                        let _ = s.send(Message::Text(msg.to_string().into())).await;
                                    }
                                } else if msg_type == "recognize_screenshot_cues" {
                                    let img_b64 = val["image"].as_str().or_else(|| val["image_base64"].as_str()).unwrap_or("");
                                    let ocr_eng = crate::ocr::get_ocr_engine();
                                    let (parsed_cues, ocr_text) = ocr_eng.recognize_cues_from_image_base64(img_b64);

                                    let resp = json!({
                                        "type": "recognized_cues_result",
                                        "raw_text": ocr_text,
                                        "cues": parsed_cues,
                                        "count": parsed_cues.len()
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                    let mut s = sink_mutex.lock().await;
                                    let _ = s.send(Message::Text(resp.to_string().into())).await;
                                } else if msg_type == "add_scheduled_cues" {
                                    let cues_val = val["cues"].clone();
                                    let append = val["append"].as_bool().unwrap_or(true);
                                    if let Ok(parsed) = serde_json::from_value::<Vec<crate::cues::ScheduledCue>>(cues_val) {
                                        let updated = lookahead_client.add_cues(parsed, append);
                                        let bcast = json!({
                                            "type": "scheduled_cues_updated",
                                            "cues": updated
                                        });
                                        let _ = tx_bcast.send(bcast.to_string());
                                    }
                                } else if msg_type == "delete_scheduled_cue" {
                                    let id = val["id"].as_str().unwrap_or("");
                                    if lookahead_client.delete_cue(id) {
                                        let bcast = json!({
                                            "type": "scheduled_cues_updated",
                                            "cues": lookahead_client.get_cues()
                                        });
                                        let _ = tx_bcast.send(bcast.to_string());
                                    }
                                } else if msg_type == "clear_scheduled_cues" {
                                    lookahead_client.clear_cues();
                                    let bcast = json!({
                                        "type": "scheduled_cues_updated",
                                        "cues": []
                                    });
                                    let _ = tx_bcast.send(bcast.to_string());
                                } else if msg_type == "set_cues_config" {
                                    if let Some(sec) = val["pre_warning_seconds"].as_f64() {
                                        lookahead_client.set_pre_warning_seconds(sec);
                                    }
                                    if let Some(ac) = val["auto_censor"].as_bool() {
                                        lookahead_client.set_auto_censor(ac);
                                    }
                                    if let Some(ap) = val["append_mode"].as_bool() {
                                        lookahead_client.set_append_cues_mode(ap);
                                    }
                                    if let Some(cn) = val["cue_notifications"].as_bool() {
                                        lookahead_client.set_notifications_enabled(cn);
                                    }
                                    crate::settings::update_cached_settings(|s| {
                                        if let Some(sec) = val["pre_warning_seconds"].as_f64() { s.pre_warning_seconds = sec; }
                                        if let Some(ac) = val["auto_censor"].as_bool() { s.auto_censor = ac; }
                                        if let Some(ap) = val["append_mode"].as_bool() { s.append_mode = ap; }
                                        if let Some(cn) = val["cue_notifications"].as_bool() { s.cue_notifications = cn; }
                                    });
                                    let bcast = json!({
                                        "type": "cues_config_updated",
                                        "config": {
                                            "pre_warning_seconds": lookahead_client.get_pre_warning_seconds(),
                                            "auto_censor": lookahead_client.get_auto_censor(),
                                            "cue_notifications": lookahead_client.get_notifications_enabled(),
                                            "append_mode": lookahead_client.get_append_cues_mode()
                                        }
                                    });
                                    let _ = tx_bcast.send(bcast.to_string());
                                } else if msg_type == "get_cues_state" {
                                    let (pname, ptime, is_play) = lookahead_client.get_player_sync_state();
                                    let resp = json!({
                                        "type": "cues_state_result",
                                        "cues": lookahead_client.get_cues(),
                                        "config": {
                                            "pre_warning_seconds": lookahead_client.get_pre_warning_seconds(),
                                            "auto_censor": lookahead_client.get_auto_censor(),
                                            "cue_notifications": lookahead_client.get_notifications_enabled(),
                                            "append_mode": lookahead_client.get_append_cues_mode()
                                        },
                                        "player_sync": {
                                            "name": pname,
                                            "time": ptime,
                                            "is_playing": is_play
                                        }
                                    });
                                    let mut s = sink_mutex.lock().await;
                                    let _ = s.send(Message::Text(resp.to_string().into())).await;
                                } else if msg_type == "check_ocr_status" {
                                    let status = crate::downloader::ModelDownloader::check_ocr_status();
                                    let resp = json!({
                                        "type": "ocr_status_result",
                                        "status": status
                                    });
                                    let mut s = sink_mutex.lock().await;
                                    let _ = s.send(Message::Text(resp.to_string().into())).await;
                                } else if msg_type == "start_ocr_models_download" {
                                    let tx_dl = tx_bcast.clone();
                                    tokio::spawn(async move {
                                        let final_status = crate::downloader::ModelDownloader::check_ocr_status();
                                        let complete_msg = json!({
                                            "type": "ocr_model_download_complete",
                                            "status": final_status
                                        });
                                        let _ = tx_dl.send(complete_msg.to_string());
                                    });
                                } else if msg_type == "test_text" {
                                    let query = val["text"].as_str().unwrap_or("");
                                    let matches = {
                                        let engine = lex.lock().await;
                                        engine.check_text(query)
                                    };
                                    let resp = json!({
                                        "type": "test_result",
                                        "is_banned": !matches.is_empty(),
                                        "matches": matches
                                    });
                                    let mut s = sink_mutex.lock().await;
                                    let _ = s.send(Message::Text(resp.to_string().into())).await;
                                } else if msg_type == "run_ocr_test" {
                                    if let Some(idx) = val["monitor_index"].as_i64() {
                                        vision_client.set_selected_monitor(idx as i32);
                                    }
                                    let text_opt = val["text"].as_str().filter(|s| !s.trim().is_empty()).map(|s| s.to_string());
                                    let obs_c = obs_client.clone();
                                    let vis_c = vision_client.clone();
                                    let tx_c = tx_bcast.clone();
                                    let sink_c = sink_mutex.clone();

                                    tokio::spawn(async move {
                                        let vis_for_blocking = vis_c.clone();
                                        let res = tokio::task::spawn_blocking(move || {
                                            vis_for_blocking.run_ocr_test(text_opt)
                                        }).await.unwrap_or_else(|e| SpecificTestResult {
                                            test_type: "ocr".to_string(),
                                            activated: false,
                                            score: 0.0,
                                            matched_rule: String::new(),
                                            matched_item: String::new(),
                                            circumstances: format!("Error in spawn_blocking OCR: {:?}", e),
                                            gpu_info: String::new(),
                                        });

                                        if res.activated {
                                            let obs_guard = obs_c.clone();
                                            let vis_guard = vis_c.clone();
                                            let tx_guard = tx_c.clone();
                                            let reason = if !res.matched_item.is_empty() {
                                                format!("BANNED-WORD: {}", res.matched_item.to_uppercase())
                                            } else {
                                                "BANNED-WORD".to_string()
                                            };
                                            tokio::spawn(async move {
                                                WSBridge::trigger_censor_with_continuous_guard(obs_guard, vis_guard, tx_guard, 3000, &reason).await;
                                            });
                                        } else {
                                            let safe_state = json!({
                                                "type": "shield_state_update",
                                                "is_nsfw": false,
                                                "reason": "TEXT SAFE"
                                            });
                                            let _ = tx_c.send(safe_state.to_string());
                                        }

                                        let resp = json!({
                                            "type": "ocr_test_result",
                                            "result": res
                                        });
                                        let _ = tx_c.send(resp.to_string());
                                        let mut s = sink_c.lock().await;
                                        let _ = s.send(Message::Text(resp.to_string().into())).await;
                                    });
                                } else if msg_type == "run_nsfw_test" {
                                    if let Some(idx) = val["monitor_index"].as_i64() {
                                        vision_client.set_selected_monitor(idx as i32);
                                    }
                                    let simulate = val["simulate"].as_bool().unwrap_or(false)
                                        || val["force"].as_bool().unwrap_or(false);
                                    let obs_c = obs_client.clone();
                                    let vis_c = vision_client.clone();
                                    let tx_c = tx_bcast.clone();
                                    let sink_c = sink_mutex.clone();

                                    tokio::spawn(async move {
                                        let vis_for_blocking = vis_c.clone();
                                        let res = tokio::task::spawn_blocking(move || {
                                            vis_for_blocking.run_nsfw_test(simulate)
                                        }).await.unwrap_or_else(|_| SpecificTestResult {
                                            test_type: "nsfw".to_string(),
                                            activated: false,
                                            score: 0.0,
                                            matched_rule: String::new(),
                                            matched_item: String::new(),
                                            circumstances: "Error executing NSFW test".to_string(),
                                            gpu_info: String::new(),
                                        });

                                        if res.activated {
                                            let obs_guard = obs_c.clone();
                                            let vis_guard = vis_c.clone();
                                            let tx_guard = tx_c.clone();
                                            let reason = if !res.matched_item.is_empty() {
                                                res.matched_item.to_uppercase()
                                            } else {
                                                "NSFW".to_string()
                                            };
                                            tokio::spawn(async move {
                                                WSBridge::trigger_censor_with_continuous_guard(obs_guard, vis_guard, tx_guard, 3000, &reason).await;
                                            });
                                        } else {
                                            let safe_state = json!({
                                                "type": "shield_state_update",
                                                "is_nsfw": false,
                                                "reason": if !res.matched_item.is_empty() {
                                                    res.matched_item.to_uppercase()
                                                } else {
                                                    "STREAM SAFE".to_string()
                                                }
                                            });
                                            let _ = tx_c.send(safe_state.to_string());
                                        }

                                        let resp = json!({
                                            "type": "nsfw_test_result",
                                            "result": res
                                        });
                                        let _ = tx_c.send(resp.to_string());
                                        let mut s = sink_c.lock().await;
                                        let _ = s.send(Message::Text(resp.to_string().into())).await;
                                    });
                                } else if msg_type == "test_censor" || msg_type == "conditional_censor_test" || msg_type == "manual_action" {
                                    if let Some(idx) = val["monitor_index"].as_i64() {
                                        vision_client.set_selected_monitor(idx as i32);
                                    }
                                    if let Some(idx) = val["monitor_index"].as_i64() {
                                        vision_client.set_selected_monitor(idx as i32);
                                    }
                                    // Conditional Screen Censor Check:
                                    // Screen is covered ONLY if NSFW or banned OCR words are detected!
                                    let force = val["force"].as_bool().unwrap_or(false)
                                        || val["action"].as_str() == Some("force_nsfw");
                                    let duration_ms = val["duration_ms"].as_u64().unwrap_or(3000);

                                    let analysis = vision_client.analyze_screen(force).await;

                                    if analysis.should_censor {
                                        println!(
                                            "[WSBridge] CENSOR TRIGGERED: {}. Covering screen in OBS for {}ms with continuous guard",
                                            analysis.status_message, duration_ms
                                        );

                                        let obs_c = obs_client.clone();
                                        let vis_c = vision_client.clone();
                                        let tx_c = tx_bcast.clone();
                                        let reason = if analysis.nsfw_score >= 0.5 {
                                            analysis.nsfw_label.to_uppercase()
                                        } else if !analysis.banned_matches.is_empty() {
                                            format!("BANNED-WORD: {}", analysis.banned_matches[0].to_uppercase())
                                        } else {
                                            "INAPPROPRIATE CONTENT".to_string()
                                        };
                                        tokio::spawn(async move {
                                            WSBridge::trigger_censor_with_continuous_guard(obs_c, vis_c, tx_c, duration_ms, &reason).await;
                                        });

                                        let test_resp = json!({
                                            "type": "censor_test_result",
                                            "should_censor": true,
                                            "analysis": analysis
                                        });
                                        let _ = tx_bcast.send(test_resp.to_string());
                                    } else {
                                        println!(
                                            "[WSBridge] CENSOR SKIPPED (NO NSFW): {}. Screen will NOT be covered in OBS.",
                                            analysis.status_message
                                        );

                                        let shield_state = json!({
                                            "type": "shield_state_update",
                                            "is_nsfw": false,
                                            "reason": "NO NSFW DETECTED — Stream Safe"
                                        });
                                        let _ = tx_bcast.send(shield_state.to_string());

                                        let test_resp = json!({
                                            "type": "censor_test_result",
                                            "should_censor": false,
                                            "analysis": analysis
                                        });
                                        let _ = tx_bcast.send(test_resp.to_string());
                                    }
                                } else if msg_type == "set_vision_boost" {
                                    let enabled = val["enabled"].as_bool().unwrap_or(false);
                                    fps_shared.store(enabled, Ordering::Relaxed);
                                    vision_client.cascade.set_boost_mode(enabled);
                                    println!("[WSBridge] Vision boost set to: {} (Cascade direct dual-scan synchronized)", enabled);
                                    let op_mode = get_operation_mode();
                                    let is_dynamic = is_dynamic_boost_active();
                                    let real_fps = get_real_analysis_fps();
                                    let (video_fps, video_status) = get_analysis_fps_and_status(op_mode, true, enabled, is_dynamic);
                                    let resp = json!({
                                        "type": "vision_mode_changed",
                                        "boosted": enabled,
                                        "fps_boosted": enabled,
                                        "dynamic_boost_active": is_dynamic,
                                        "real_fps": real_fps,
                                        "video_fps": video_fps,
                                        "video_status": video_status,
                                        "analysis_paused": op_mode == 1 || op_mode >= 3
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "auto_setup_obs" {
                                    let obs_ref = obs_client.clone();
                                    let vis_ref = vision_client.clone();
                                    let tx_clone = tx_bcast.clone();
                                    tokio::spawn(async move {
                                        let res = obs_ref.auto_setup().await;
                                        let (success, install_ok, injection_ok, message) = match res {
                                            Ok(rep) => (rep.success, rep.install_ok, rep.injection_ok, rep.message),
                                            Err(err) => (false, false, false, err),
                                        };
                                        // Give the freshly attached filter a moment to send its first
                                        // UDP heartbeat, then verify the plugin is actually ALIVE in OBS.
                                        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                                        let plugin_running = vis_ref.cascade.is_obs_plugin_active();
                                        let setup_resp = json!({
                                            "type": "obs_setup_result",
                                            "success": success,
                                            "install_ok": install_ok,
                                            "injection_ok": injection_ok,
                                            "plugin_running": plugin_running,
                                            "message": message
                                        });
                                        let _ = tx_clone.send(setup_resp.to_string());
                                    });
                                } else if msg_type == "check_obs_status" {
                                    let is_connected = *obs_client.is_connected.lock().await;
                                    let is_running = crate::injector::ObsInjector::find_obs_process().is_some();
                                    let exe_found = crate::paths::PathResolver::find_obs_executable().is_some();
                                    let resp = json!({
                                        "type": "obs_status_report",
                                        "is_running": is_running,
                                        "is_connected": is_connected,
                                        "exe_found": exe_found
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "launch_obs" {
                                    if let Some(obs_exe) = crate::paths::PathResolver::find_obs_executable() {
                                        let working_dir = obs_exe.parent().unwrap_or(std::path::Path::new("C:\\"));
                                        let _ = std::process::Command::new(&obs_exe).current_dir(working_dir).spawn();
                                    }
                                } else if msg_type == "set_obs_scene" {
                                    let scene_name = val["scene_name"].as_str().unwrap_or("");
                                    if !scene_name.is_empty() {
                                        *obs_client.selected_scene.lock().await = Some(scene_name.to_string());
                                        let obs_c = obs_client.clone();
                                        let sc = scene_name.to_string();
                                        tokio::spawn(async move {
                                            let _ = obs_c.ensure_censor_shield_in_scene(&sc).await;
                                            obs_c.update_capture_and_video_telemetry().await;
                                        });
                                        println!("[WSBridge] Active target OBS scene set to: {}", scene_name);
                                    }
                                } else if msg_type == "refresh_obs_scenes" {
                                    let obs_c = obs_client.clone();
                                    let tx_clone = tx_bcast.clone();
                                    tokio::spawn(async move {
                                        let _ = obs_c.refresh_scenes().await;
                                        obs_c.update_capture_and_video_telemetry().await;
                                        let cur = obs_c.selected_scene.lock().await.clone().unwrap_or_default();
                                        let list = obs_c.cached_scenes.lock().await.clone();
                                        let resp = json!({
                                            "type": "obs_scenes_updated",
                                            "current_scene": cur,
                                            "scenes": list
                                        });
                                        let _ = tx_clone.send(resp.to_string());
                                    });
                                } else if msg_type == "check_models_status" {
                                    let status = crate::downloader::ModelDownloader::check_models_status();
                                    let resp = json!({
                                        "type": "models_status_result",
                                        "status": status
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                    let mut s = sink_mutex.lock().await;
                                    let _ = s.send(Message::Text(resp.to_string().into())).await;
                                } else if msg_type == "start_models_download" {
                                    let tx_dl = tx_bcast.clone();
                                    tokio::spawn(async move {
                                        let cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                                        for spec in &crate::downloader::REQUIRED_MODELS {
                                            let tx_prog = tx_dl.clone();
                                            let res = crate::downloader::ModelDownloader::download_model(
                                                spec,
                                                cancel_token.clone(),
                                                move |event| {
                                                    let stage = if event.filename.contains("vit") { 1 } else { 2 };
                                                    let msg = json!({
                                                        "type": "model_download_progress",
                                                        "progress": event,
                                                        "stage": stage,
                                                        "filename": event.filename,
                                                        "bytes_downloaded": event.downloaded_bytes,
                                                        "total_bytes": event.total_bytes,
                                                        "speed_bytes_per_sec": event.speed_bytes_per_sec,
                                                        "percent": event.percent,
                                                        "status": event.status
                                                    });
                                                    let _ = tx_prog.send(msg.to_string());
                                                },
                                            ).await;

                                            if let Err(e) = res {
                                                let err_msg = json!({
                                                    "type": "model_download_error",
                                                    "filename": spec.filename,
                                                    "error": e,
                                                    "message": e
                                                });
                                                let _ = tx_dl.send(err_msg.to_string());
                                                return;
                                            }
                                        }

                                        let final_status = crate::downloader::ModelDownloader::check_models_status();
                                        let complete_msg = json!({
                                            "type": "models_download_complete",
                                            "success": final_status.all_ready,
                                            "models_ready": final_status.all_ready,
                                            "status": final_status
                                        });
                                        let _ = tx_dl.send(complete_msg.to_string());
                                    });
                                } else if msg_type == "install_ru_ocr" || msg_type == "install_en_ocr" {
                                    let lang_tag = if msg_type == "install_en_ocr" { "en-US" } else { "ru-RU" };
                                    let cap_name = format!("Language.OCR~~~{}~0.0.1.0", lang_tag);
                                    let ps_cmd = format!(
                                        "$Host.UI.RawUI.WindowTitle = 'blewred — Windows OCR Package Installation ({lang_tag})'; \
                                        Write-Host '=====================================================' -ForegroundColor Cyan; \
                                        Write-Host '[blewred] Installing Windows OCR Language Pack for {lang_tag}...' -ForegroundColor Yellow; \
                                        Write-Host 'Running: Add-WindowsCapability -Online -Name {cap_name}' -ForegroundColor Gray; \
                                        Write-Host '=====================================================' -ForegroundColor Cyan; \
                                        try {{ \
                                            $res = Add-WindowsCapability -Online -Name '{cap_name}' -ErrorAction Stop; \
                                            Write-Host '[blewred] OCR language pack for {lang_tag} installed successfully!' -ForegroundColor Green; \
                                            Write-Host 'You can close this window. blewred will detect it automatically.' -ForegroundColor Green; \
                                        }} catch {{ \
                                            Write-Host \"[blewred] DISM command failed: $($_.Exception.Message)\" -ForegroundColor Red; \
                                            Write-Host '[blewred] Opening Windows Language Settings as alternative method...' -ForegroundColor Yellow; \
                                            Start-Process ms-settings:regionlanguage; \
                                        }} \
                                        Write-Host ''; \
                                        Write-Host 'Press Enter to exit...' -ForegroundColor DarkGray; \
                                        [Console]::ReadLine() | Out-Null;"
                                    );
                                    tokio::task::spawn_blocking(move || {
                                        let mut cmd = std::process::Command::new("powershell.exe");
                                        cmd.args([
                                            "-NoProfile",
                                            "-Command",
                                            &format!("Start-Process powershell.exe -Verb RunAs -ArgumentList '-NoProfile', '-Command', \"{}\"", ps_cmd.replace('"', "\\\"")),
                                        ]);
                                        let _ = cmd.status();
                                    });
                                    let resp = json!({
                                        "type": "ocr_install_triggered",
                                        "lang": lang_tag
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "open_windows_language_settings" {
                                    tokio::task::spawn_blocking(|| {
                                        let _ = std::process::Command::new("powershell.exe")
                                            .args(["-NoProfile", "-Command", "Start-Process ms-settings:regionlanguage"])
                                            .spawn();
                                    });
                                } else if msg_type == "set_hotkey_settings" {
                                    if let Some(scope) = val.get("hotkey_scope").and_then(|v| v.as_str()) {
                                        crate::settings::update_cached_settings(|s| s.hotkey_scope = scope.to_string());
                                    }
                                    if let Some(panic_key) = val.get("hotkey_panic").and_then(|v| v.as_str()) {
                                        crate::settings::update_cached_settings(|s| s.hotkey_panic = panic_key.to_string());
                                    }
                                    if let Some(threat_key) = val.get("hotkey_threat").and_then(|v| v.as_str()) {
                                        crate::settings::update_cached_settings(|s| s.hotkey_threat = threat_key.to_string());
                                    }
                                    let cached = crate::settings::get_cached_settings();
                                    let _ = crate::settings::save_settings(&cached);
                                    crate::reload_global_hotkeys();
                                    let resp = json!({
                                        "type": "hotkey_settings_changed",
                                        "hotkey_scope": cached.hotkey_scope,
                                        "hotkey_panic": cached.hotkey_panic,
                                        "hotkey_threat": cached.hotkey_threat,
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                } else if msg_type == "toggle_emergency_shield" {
                                    let obs_c = obs_client.clone();
                                    let tx_c = tx_bcast.clone();
                                    let reason = val.get("reason").and_then(|r| r.as_str()).unwrap_or("EMERGENCY BLOCK").to_string();
                                    tokio::spawn(async move {
                                        WSBridge::toggle_emergency_shield(obs_c, tx_c, &reason).await;
                                    });
                                } else if msg_type == "open_browser_extension_dir" {
                                    let ext_path = crate::paths::PathResolver::find_extension_dir();
                                    let _ = std::fs::create_dir_all(&ext_path);
                                    let _ = std::process::Command::new("explorer.exe").arg(&ext_path).spawn();
                                } else if msg_type == "launch_browser_with_extension" {
                                    let root = crate::paths::PathResolver::find_project_root();
                                    let bat_candidates = [
                                        root.join("launch_browser_with_extension.bat"),
                                        root.join("scripts").join("launch_browser_with_extension.bat"),
                                    ];
                                    let bat_path = bat_candidates.iter().find(|p| p.exists()).cloned().unwrap_or_else(|| root.join("launch_browser_with_extension.bat"));
                                    let mut cmd = std::process::Command::new("cmd.exe");
                                    cmd.args(["/c", &bat_path.to_string_lossy()]);
                                    #[cfg(windows)]
                                    {
                                        use std::os::windows::process::CommandExt;
                                        cmd.creation_flags(0x08000000);
                                    }
                                    let _ = cmd.spawn();
                                    let resp = json!({
                                        "type": "browser_launched_with_extension"
                                    });
                                    let _ = tx_bcast.send(resp.to_string());
                                }
                            }
                        }
                    }
                }
            });
        }
    }
}
