#![recursion_limit = "256"]

pub mod cascade;
pub mod cues;
pub mod downloader;
pub mod injector;
pub mod lexical;
pub mod lookahead;
pub mod obs;
pub mod ocr;
pub mod paths;
pub mod settings;
pub mod tracker;
pub mod vision;
pub mod ws_bridge;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;

use lexical::LexicalEngine;
use lookahead::LookaheadIncidentTicket;
use obs::OBSClient;
use vision::{VisionEngine, ScreenAnalysisResult, SpecificTestResult};
use ws_bridge::WSBridge;

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();
static HUD_MONITOR_INDEX: AtomicI32 = AtomicI32::new(-1);
static CURRENT_HUD_TICKET_ID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn get_app_handle() -> Option<&'static tauri::AppHandle> {
    APP_HANDLE.get()
}

pub fn set_hud_monitor_index(idx: i32) {
    HUD_MONITOR_INDEX.store(idx, Ordering::Relaxed);
    println!("[HUD] Streamer alert monitor set to: {}", idx);
}

pub fn get_hud_monitor_index() -> i32 {
    HUD_MONITOR_INDEX.load(Ordering::Relaxed)
}

pub struct TrayMenuState {
    pub mode_items: Vec<(u8, CheckMenuItem<tauri::Wry>)>,
    pub screen_mon_items: Vec<(i32, CheckMenuItem<tauri::Wry>)>,
    pub hud_mon_items: Vec<(i32, CheckMenuItem<tauri::Wry>)>,
}

static TRAY_STATE: OnceLock<std::sync::Mutex<TrayMenuState>> = OnceLock::new();
static TRAY_HOLDER: OnceLock<tauri::tray::TrayIcon> = OnceLock::new();

pub fn update_tray_mode(mode: u8) {
    if let Some(state_lock) = TRAY_STATE.get() {
        if let Ok(state) = state_lock.lock() {
            for (m, item) in &state.mode_items {
                let _ = item.set_checked(*m == mode);
            }
        }
    }
}

pub fn update_tray_screen_monitor(selected: i32) {
    if let Some(state_lock) = TRAY_STATE.get() {
        if let Ok(state) = state_lock.lock() {
            for (idx, item) in &state.screen_mon_items {
                let _ = item.set_checked(*idx == selected);
            }
        }
    }
}

pub fn update_tray_hud_monitor(selected: i32) {
    if let Some(state_lock) = TRAY_STATE.get() {
        if let Ok(state) = state_lock.lock() {
            for (idx, item) in &state.hud_mon_items {
                let _ = item.set_checked(*idx == selected);
            }
        }
    }
}

pub fn place_window_on_monitor_by_index(window: &tauri::WebviewWindow, mon_idx: i32) {
    let monitors = vision::VisionEngine::enumerate_monitors();
    let target_mon = if mon_idx >= 0 {
        monitors.iter().find(|m| m.index == mon_idx).or_else(|| monitors.iter().find(|m| m.is_primary)).or_else(|| monitors.first())
    } else {
        monitors.iter().find(|m| m.is_primary).or_else(|| monitors.first())
    };

    if let Some(m) = target_mon {
        let scale = if m.width >= 1920 { 1.25 } else { 1.0 };
        let target_w = (m.width as f64 * 0.72 / scale).clamp(860.0, 1040.0);
        let target_h = (m.height as f64 * 0.78 / scale).clamp(520.0, 650.0);
        let target_phys_w = (target_w * scale).round() as i32;
        let target_phys_h = (target_h * scale).round() as i32;

        let center_x = m.x + ((m.width - target_phys_w) / 2).max(0);
        let center_y = m.y + ((m.height - target_phys_h) / 2).max(0);

        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: target_w,
            height: target_h,
        }));
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: center_x,
            y: center_y,
        }));
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        println!(
            "[Monitor] Placed main window on Monitor {} ('{}') at ({}, {}) (scale: {:.2}, target: {:.0}x{:.0})",
            m.index + 1,
            m.name,
            center_x,
            center_y,
            scale,
            target_w,
            target_h
        );
    }
}

pub fn ensure_window_on_primary_monitor(window: &tauri::WebviewWindow) {
    let user_settings = crate::settings::get_cached_settings();
    place_window_on_monitor_by_index(window, user_settings.selected_monitor);
}

pub fn show_lookahead_hud(ticket: LookaheadIncidentTicket) {
    *CURRENT_HUD_TICKET_ID.lock().unwrap() = Some(ticket.ticket_id.clone());
    if let Some(app) = get_app_handle() {
        let hud = match app.get_webview_window("lookahead_hud") {
            Some(w) => w,
            None => {
                println!("[HUD] 'lookahead_hud' window not found, building dynamically...");
                match tauri::WebviewWindowBuilder::new(app, "lookahead_hud", tauri::WebviewUrl::App("lookahead_hud.html".into()))
                    .title("blewred")
                    .inner_size(580.0, 530.0)
                    .resizable(false)
                    .always_on_top(true)
                    .decorations(false)
                    .transparent(true)
                    .skip_taskbar(true)
                    .visible(false)
                    .build()
                {
                    Ok(w) => w,
                    Err(e) => {
                        eprintln!("[HUD] Failed to dynamically build lookahead_hud: {}", e);
                        return;
                    }
                }
            }
        };

        let hud_w = 580.0;
        let hud_h = 530.0;
        let _ = hud.set_size(tauri::Size::Logical(tauri::LogicalSize::new(hud_w, hud_h)));

        let monitors = VisionEngine::enumerate_monitors();
        let target_idx = get_hud_monitor_index();

        let target_mon = if target_idx >= 0 {
            monitors.iter().find(|m| m.index == target_idx).or_else(|| monitors.iter().find(|m| m.is_primary)).or_else(|| monitors.first())
        } else {
            // Auto mode: If multi-monitor, place on streamer's primary display
            monitors.iter().find(|m| m.is_primary).or_else(|| monitors.first())
        };

        if let Some(mon) = target_mon {
            let target_x = mon.x + ((mon.width - hud_w as i32) / 2).max(20);
            let target_y = mon.y + 40;
            let _ = hud.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: target_x,
                y: target_y,
            }));
            println!("[HUD] Placed alert on monitor '{}' (index: {}) at ({}, {})", mon.name, mon.index, target_x, target_y);
        }

        let _ = hud.unminimize();
        let _ = hud.show();
        let _ = hud.set_always_on_top(true);
        let _ = hud.set_focus();
        let _ = hud.emit("lookahead_ticket", &ticket);
    } else {
        eprintln!("[HUD] Application handle not available for lookahead HUD");
    }
}

pub fn hide_lookahead_hud_if_ticket(ticket_id: &str) {
    let should_hide = {
        let mut cur = CURRENT_HUD_TICKET_ID.lock().unwrap();
        if cur.as_deref() == Some(ticket_id) {
            *cur = None;
            true
        } else {
            false
        }
    };
    if should_hide {
        if let Some(app) = get_app_handle() {
            if let Some(hud) = app.get_webview_window("lookahead_hud") {
                let _ = hud.hide();
            }
        }
    }
}

pub fn hide_lookahead_hud() {
    *CURRENT_HUD_TICKET_ID.lock().unwrap() = None;
    if let Some(app) = get_app_handle() {
        if let Some(hud) = app.get_webview_window("lookahead_hud") {
            let _ = hud.hide();
        }
    }
}

static LOOKAHEAD_PREVIEW_VISIBLE: AtomicBool = AtomicBool::new(false);
static LAST_TOGGLE_TIME_MS: AtomicU64 = AtomicU64::new(0);

pub fn get_or_create_lookahead_preview_window(app: &tauri::AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window("lookahead_preview") {
        return Some(w);
    }
    println!("[Preview] 'lookahead_preview' window not found, building dynamically...");
    match tauri::WebviewWindowBuilder::new(app, "lookahead_preview", tauri::WebviewUrl::App("lookahead_preview.html".into()))
        .title("blewred")
        .inner_size(560.0, 420.0)
        .min_inner_size(360.0, 260.0)
        .center()
        .resizable(true)
        .always_on_top(true)
        .decorations(false)
        .transparent(false)
        .skip_taskbar(false)
        .visible(false)
        .build()
    {
        Ok(w) => {
            if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png")) {
                let _ = w.set_icon(icon);
            }
            Some(w)
        }
        Err(e) => {
            eprintln!("[Preview] Failed to dynamically build lookahead_preview window: {}", e);
            None
        }
    }
}

pub fn show_lookahead_preview_window() {
    LOOKAHEAD_PREVIEW_VISIBLE.store(true, Ordering::SeqCst);
    if let Some(app) = get_app_handle() {
        if let Some(w) = get_or_create_lookahead_preview_window(app) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_always_on_top(true);
            let _ = w.set_focus();
        }
    }
}

pub fn hide_lookahead_preview_window() {
    LOOKAHEAD_PREVIEW_VISIBLE.store(false, Ordering::SeqCst);
    if let Some(app) = get_app_handle() {
        if let Some(w) = app.get_webview_window("lookahead_preview") {
            let _ = w.hide();
        }
    }
}

pub fn toggle_lookahead_preview_window() -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let prev = LAST_TOGGLE_TIME_MS.load(Ordering::SeqCst);
    if now.saturating_sub(prev) < 350 {
        println!("[Preview] Debounced rapid toggle within {}ms", now.saturating_sub(prev));
        return is_lookahead_preview_window_visible();
    }
    LAST_TOGGLE_TIME_MS.store(now, Ordering::SeqCst);

    if let Some(app) = get_app_handle() {
        if let Some(w) = get_or_create_lookahead_preview_window(app) {
            let is_vis = is_lookahead_preview_window_visible();
            if is_vis {
                let _ = w.hide();
                LOOKAHEAD_PREVIEW_VISIBLE.store(false, Ordering::SeqCst);
                false
            } else {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_always_on_top(true);
                let _ = w.set_focus();
                LOOKAHEAD_PREVIEW_VISIBLE.store(true, Ordering::SeqCst);
                true
            }
        } else {
            false
        }
    } else {
        false
    }
}

pub fn is_lookahead_preview_window_visible() -> bool {
    LOOKAHEAD_PREVIEW_VISIBLE.load(Ordering::SeqCst)
}

pub fn emit_lookahead_preview_frame(data: &crate::lookahead::LookaheadPreviewData) {
    if let Some(app) = get_app_handle() {
        if let Some(w) = app.get_webview_window("lookahead_preview") {
            let _ = w.emit("lookahead_preview_frame", data);
        }
    }
}

pub struct AppState {
    pub lexical: Arc<Mutex<LexicalEngine>>,
    pub obs: Arc<OBSClient>,
    pub vision: Arc<VisionEngine>,
    pub ws: Arc<WSBridge>,
    pub fps_boosted: Arc<AtomicBool>,
}

#[tauri::command]
async fn get_telemetry(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let obs_conn = *state.obs.is_connected.lock().await;
    let rules_count = {
        let engine = state.lexical.lock().await;
        engine.rules.len()
    };
    let boosted = state.fps_boosted.load(Ordering::Relaxed);
    let is_dynamic = crate::ws_bridge::is_dynamic_boost_active();
    let real_fps = crate::ws_bridge::get_real_analysis_fps();
    let realtime_guard = state.ws.realtime_guard_active.load(Ordering::Relaxed);
    let operation_mode = crate::ws_bridge::get_operation_mode();
    let (video_fps, video_status) = crate::ws_bridge::get_analysis_fps_and_status(
        operation_mode,
        realtime_guard,
        boosted,
        is_dynamic,
    );
    let gpu_info = state.vision.gpu_name.clone();
    let directml_ready = state.vision.directml_available.load(Ordering::Relaxed);
    let monitors = vision::VisionEngine::enumerate_monitors();
    let mut selected_monitor = state.vision.get_selected_monitor();
    if !monitors.iter().any(|m| m.index == selected_monitor) {
        let fallback_idx = monitors.iter().find(|m| m.is_primary).map(|m| m.index).unwrap_or(0);
        state.vision.set_selected_monitor(fallback_idx);
        crate::settings::update_cached_settings(|s| s.selected_monitor = fallback_idx);
        update_tray_screen_monitor(fallback_idx);
        selected_monitor = fallback_idx;
    }

    let mut hud_monitor = crate::get_hud_monitor_index();
    if hud_monitor != -1 && !monitors.iter().any(|m| m.index == hud_monitor) {
        crate::set_hud_monitor_index(-1);
        crate::settings::update_cached_settings(|s| s.hud_monitor = -1);
        update_tray_hud_monitor(-1);
        hud_monitor = -1;
    }

    let resolution = state.obs.video_resolution.lock().await.clone();
    let capture_active = *state.obs.capture_active.lock().await;
    let capture_source = state.obs.capture_source_name.lock().await.clone();
    let current_scene = state.obs.selected_scene.lock().await.clone().unwrap_or_default();
    let scenes = state.obs.cached_scenes.lock().await.clone();
    let nsfw_threshold = state.vision.get_nsfw_threshold();
    let censor_categories = state.vision.get_censor_categories();
    let censor_shield_enabled = crate::ws_bridge::is_censor_shield_enabled();

    Ok(serde_json::json!({
        "obs_connected": obs_conn,
        "obs_plugin_active": state.vision.cascade.is_obs_plugin_active(),
        "rules_count": rules_count,
        "fps_boosted": boosted,
        "dynamic_boost_active": is_dynamic,
        "real_fps": real_fps,
        "video_fps": video_fps,
        "video_status": video_status,
        "analysis_paused": operation_mode == 1 || operation_mode >= 3 || !realtime_guard,
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
        "hud_monitor": hud_monitor,
        "operation_mode": operation_mode,
        "realtime_guard": realtime_guard,
        "nsfw_threshold": nsfw_threshold,
        "censor_categories": censor_categories,
        "censor_shield_enabled": censor_shield_enabled,
        "is_shield_visible": state.obs.is_shield_visible.load(Ordering::Relaxed),
        "extension_connected": crate::ws_bridge::is_browser_extension_connected(),
        "cues_count": state.ws.lookahead.get_cues().len(),
        "pre_warning_seconds": state.ws.lookahead.get_pre_warning_seconds(),
        "auto_censor": state.ws.lookahead.get_auto_censor(),
        "append_cues_mode": state.ws.lookahead.get_append_cues_mode(),
        "player_sync": {
            "name": state.ws.lookahead.get_player_sync_state().0,
            "time": state.ws.lookahead.get_player_sync_state().1,
            "is_playing": state.ws.lookahead.get_player_sync_state().2,
            "detected": state.ws.lookahead.is_player_active(),
        }
    }))
}

#[tauri::command]
async fn get_monitors() -> Result<Vec<vision::MonitorInfo>, String> {
    Ok(vision::VisionEngine::enumerate_monitors())
}

#[tauri::command]
async fn set_monitor(state: State<'_, AppState>, index: i32) -> Result<(), String> {
    let monitors = vision::VisionEngine::enumerate_monitors();
    let valid_idx = if monitors.iter().any(|m| m.index == index) {
        index
    } else {
        monitors.iter().find(|m| m.is_primary).map(|m| m.index).unwrap_or(0)
    };
    state.vision.set_selected_monitor(valid_idx);
    crate::settings::update_cached_settings(|s| s.selected_monitor = valid_idx);
    update_tray_screen_monitor(valid_idx);
    Ok(())
}

#[tauri::command]
async fn set_hud_monitor(index: i32) -> Result<(), String> {
    let monitors = vision::VisionEngine::enumerate_monitors();
    let valid_idx = if index == -1 || monitors.iter().any(|m| m.index == index) {
        index
    } else {
        -1
    };
    set_hud_monitor_index(valid_idx);
    crate::settings::update_cached_settings(|s| s.hud_monitor = valid_idx);
    update_tray_hud_monitor(valid_idx);
    Ok(())
}

#[tauri::command]
async fn get_hud_monitor() -> Result<i32, String> {
    Ok(get_hud_monitor_index())
}

#[tauri::command]
async fn reposition_to_primary_monitor(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        ensure_window_on_primary_monitor(&w);
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(())
}

#[tauri::command]
async fn get_operation_mode() -> Result<u8, String> {
    Ok(crate::ws_bridge::get_operation_mode())
}

#[tauri::command]
async fn set_operation_mode(state: State<'_, AppState>, mode: u8) -> Result<(), String> {
    crate::ws_bridge::safe_set_operation_mode(mode, &state.obs, &state.ws.broadcast_tx(), &state.ws.lookahead, &state.vision).await;
    update_tray_mode(mode);
    Ok(())
}

#[tauri::command]
async fn test_lookahead_alert(state: State<'_, AppState>) -> Result<(), String> {
    // Reset previous test state completely
    state.ws.lookahead.clear_test_tickets();

    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;

    let mock_ticket = LookaheadIncidentTicket {
        ticket_id: format!("test_{}", now_ms),
        player_type: "Alloha (Demo)".to_string(),
        eta_seconds: 8.4,
        severity: "high".to_string(),
        reason: "Exposed female breast (Test)".to_string(),
        score: 0.89,
        raw_image_base64: crate::ws_bridge::get_realistic_test_thumbnail(),
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

    state.ws.lookahead.record_ticket(mock_ticket.clone());
    show_lookahead_hud(mock_ticket.clone());

    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "lookahead_ticket",
        "ticket": mock_ticket
    }).to_string());

    Ok(())
}

#[tauri::command]
async fn get_rules_text(state: State<'_, AppState>) -> Result<String, String> {
    let engine = state.lexical.lock().await;
    Ok(engine.export_words_to_text())
}

#[tauri::command]
async fn update_rules_text(state: State<'_, AppState>, text: String) -> Result<usize, String> {
    let mut engine = state.lexical.lock().await;
    engine.clear();
    engine.load_words_from_text(&text);
    let count = engine.rules.len();
    let _ = crate::settings::save_stopwords(&text);
    Ok(count)
}

#[tauri::command]
async fn test_text(state: State<'_, AppState>, text: String) -> Result<serde_json::Value, String> {
    let engine = state.lexical.lock().await;
    let matches = engine.check_text(&text);
    Ok(serde_json::json!({
        "is_banned": !matches.is_empty(),
        "matches": matches
    }))
}

#[tauri::command]
async fn test_screen_censorship(state: State<'_, AppState>, force: bool) -> Result<ScreenAnalysisResult, String> {
    let result = state.vision.analyze_screen(force).await;
    if result.should_censor {
        println!("[Tauri IPC] NSFW/OCR violation detected: {}. Covering screen in OBS 3s", result.status_message);
        let obs = state.obs.clone();
        let vis = state.vision.clone();
        let tx = state.ws.broadcast_tx();
        let msg = result.status_message.clone();
        tokio::spawn(async move {
            WSBridge::trigger_censor_with_continuous_guard(obs, vis, tx, 3000, &msg).await;
        });
    } else {
        println!("[Tauri IPC] NO NSFW DETECTED ({:.0}% risk). Screen remains clean/uncovered.", result.nsfw_score * 100.0);
        state.ws.broadcast_shield_state(false, &result.status_message);
    }
    Ok(result)
}

#[tauri::command]
async fn run_ocr_test(state: State<'_, AppState>, text: Option<String>) -> Result<SpecificTestResult, String> {
    let res = state.vision.run_ocr_test(text);
    if res.activated {
        let obs = state.obs.clone();
        let vis = state.vision.clone();
        let tx = state.ws.broadcast_tx();
        let circ = res.circumstances.clone();
        tokio::spawn(async move {
            WSBridge::trigger_censor_with_continuous_guard(obs, vis, tx, 3000, &circ).await;
        });
    }
    Ok(res)
}

#[tauri::command]
async fn run_nsfw_test(state: State<'_, AppState>, simulate: bool) -> Result<SpecificTestResult, String> {
    let res = state.vision.run_nsfw_test(simulate);
    if res.activated {
        let obs = state.obs.clone();
        let vis = state.vision.clone();
        let tx = state.ws.broadcast_tx();
        let circ = res.circumstances.clone();
        tokio::spawn(async move {
            WSBridge::trigger_censor_with_continuous_guard(obs, vis, tx, 3000, &circ).await;
        });
    }
    Ok(res)
}

#[tauri::command]
async fn emergency_mute(state: State<'_, AppState>, duration_ms: u64) -> Result<(), String> {
    state.obs.trigger_visual_censor(duration_ms).await;
    state.ws.broadcast_emergency_mute(duration_ms);
    Ok(())
}

#[tauri::command]
async fn toggle_emergency_shield(state: State<'_, AppState>, reason: Option<String>) -> Result<bool, String> {
    let obs = state.obs.clone();
    let tx = state.ws.broadcast_tx();
    let r = reason.unwrap_or_else(|| "Manual Emergency Toggle".to_string());
    let active = WSBridge::toggle_emergency_shield(obs, tx, &r).await;
    Ok(active)
}

#[tauri::command]
async fn toggle_fps_boost(state: State<'_, AppState>) -> Result<bool, String> {
    let prev = state.fps_boosted.load(Ordering::Relaxed);
    let next = !prev;
    state.fps_boosted.store(next, Ordering::Relaxed);
    state.vision.cascade.set_boost_mode(next);
    println!("[Vision] FPS mode toggled in Rust: Boost = {} (Cascade direct dual-scan synchronized)", next);
    state.ws.broadcast_fps_mode(next);
    crate::settings::update_cached_settings(|s| s.fps_boosted = next);
    Ok(next)
}

#[tauri::command]
async fn set_fps_boost(state: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    state.fps_boosted.store(enabled, Ordering::Relaxed);
    state.vision.cascade.set_boost_mode(enabled);
    println!("[Vision] FPS mode set in Rust: Boost = {} (Cascade direct dual-scan synchronized)", enabled);
    state.ws.broadcast_fps_mode(enabled);
    crate::settings::update_cached_settings(|s| s.fps_boosted = enabled);
    Ok(enabled)
}

#[tauri::command]
async fn toggle_censor_shield(state: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    crate::ws_bridge::set_censor_shield_enabled_with_sync(&state.obs, &state.ws.broadcast_tx(), enabled).await;
    println!("[Tauri IPC] Censor Shield armed preference updated: {}", enabled);
    Ok(enabled)
}

#[tauri::command]
async fn toggle_shield_donate(state: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    crate::ws_bridge::set_shield_donate_and_sync(&state.obs, &state.ws.broadcast_tx(), enabled).await;
    println!("[Tauri IPC] Censor Shield support banner preference updated: {}", enabled);
    Ok(enabled)
}

#[tauri::command]
async fn toggle_ocr(state: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    state.vision.set_ocr_enabled(enabled);
    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "ocr_state_changed",
        "enabled": enabled
    }).to_string());
    println!("[Tauri IPC] OCR detection toggled: {}", enabled);
    crate::settings::update_cached_settings(|s| s.ocr_enabled = enabled);
    Ok(enabled)
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn().map_err(|e| e.to_string())?;
    }
    println!("[Tauri IPC] Opened external URL: {}", url);
    Ok(())
}

#[tauri::command]
async fn open_windows_graphics_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:display-advancedgraphics"])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    println!("[Tauri IPC] Launched Windows Advanced Graphics Settings");
    Ok(())
}

#[tauri::command]
async fn open_extension_folder() -> Result<(), String> {
    let ext_dir = crate::paths::PathResolver::find_extension_dir();
    let _ = std::fs::create_dir_all(&ext_dir);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer")
            .arg(ext_dir.to_string_lossy().as_ref())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    println!("[Tauri IPC] Opened Chrome extension folder: {:?}", ext_dir);
    Ok(())
}

#[tauri::command]
async fn open_docs_guide() -> Result<(), String> {
    let guide_url = "https://msbluesnow.github.io/blewred/quickstart_streamer";
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", guide_url])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    println!("[Tauri IPC] Opened Streamer Quickstart Guide web URL: {}", guide_url);
    Ok(())
}

#[tauri::command]
async fn set_hide_setup_guide(hide: bool) -> Result<(), String> {
    crate::settings::update_cached_settings(|s| {
        s.hide_setup_guide = hide;
    });
    println!("[Tauri IPC] Updated hide_setup_guide: {}", hide);
    Ok(())
}

#[tauri::command]
async fn set_language(state: State<'_, AppState>, language: String) -> Result<(), String> {
    let clean_lang = if language == "en" { "en" } else { "ru" };
    crate::settings::update_cached_settings(|s| {
        s.language = clean_lang.to_string();
    });
    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "language_changed",
        "language": clean_lang
    }).to_string());
    println!("[Tauri IPC] Language updated and broadcasted: {}", clean_lang);
    Ok(())
}


#[tauri::command]
async fn toggle_realtime_guard(state: State<'_, AppState>) -> Result<bool, String> {
    let prev = state.ws.realtime_guard_active.load(Ordering::Relaxed);
    let next = !prev;
    state.ws.realtime_guard_active.store(next, Ordering::Relaxed);
    println!("[Tauri IPC] Real-time Guard toggled: {}", next);
    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "realtime_guard_changed",
        "enabled": next
    }).to_string());
    Ok(next)
}

#[tauri::command]
async fn set_nsfw_threshold(state: State<'_, AppState>, threshold: u32) -> Result<u32, String> {
    state.vision.set_nsfw_threshold(threshold);
    crate::settings::update_cached_settings(|s| s.nsfw_threshold = threshold);
    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "nsfw_threshold_changed",
        "threshold": threshold
    }).to_string());
    Ok(threshold)
}

#[tauri::command]
async fn set_censor_categories(state: State<'_, AppState>, categories: crate::cascade::CensorCategories) -> Result<crate::cascade::CensorCategories, String> {
    state.vision.set_censor_categories(categories.clone());
    crate::settings::update_cached_settings(|s| s.censor_categories = categories.clone());
    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "censor_categories_changed",
        "categories": categories
    }).to_string());
    Ok(categories)
}

#[tauri::command]
async fn auto_setup_obs(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let rep = state.obs.auto_setup().await?;
    // Wait for the first UDP heartbeat from the (re)attached OBS filter, then report
    // whether the plugin is genuinely alive inside obs64.exe.
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    let plugin_running = state.vision.cascade.is_obs_plugin_active();
    Ok(serde_json::json!({
        "success": rep.success,
        "install_ok": rep.install_ok,
        "injection_ok": rep.injection_ok,
        "plugin_running": plugin_running,
        "message": rep.message,
    }))
}

#[derive(serde::Serialize)]
struct ObsStatusInfo {
    is_running: bool,
    is_connected: bool,
    exe_found: bool,
    plugin_running: bool,
}

#[tauri::command]
async fn check_obs_status(state: State<'_, AppState>) -> Result<ObsStatusInfo, String> {
    let is_connected = *state.obs.is_connected.lock().await;
    let is_running = crate::injector::ObsInjector::find_obs_process().is_some();
    let exe_found = crate::paths::PathResolver::find_obs_executable().is_some();
    let plugin_running = state.vision.cascade.is_obs_plugin_active();
    Ok(ObsStatusInfo {
        is_running,
        is_connected,
        exe_found,
        plugin_running,
    })
}

#[tauri::command]
async fn launch_obs_studio() -> Result<bool, String> {
    if let Some(obs_exe) = crate::paths::PathResolver::find_obs_executable() {
        let working_dir = obs_exe.parent().unwrap_or(std::path::Path::new("C:\\"));
        std::process::Command::new(&obs_exe)
            .current_dir(working_dir)
            .spawn()
            .map_err(|e| format!("Failed to launch OBS Studio: {}", e))?;
        Ok(true)
    } else {
        Err("Executable file obs64.exe was not found on disk".to_string())
    }
}

#[tauri::command]
async fn get_scheduled_cues(state: State<'_, AppState>) -> Result<Vec<crate::cues::ScheduledCue>, String> {
    Ok(state.ws.lookahead.get_cues())
}

#[tauri::command]
async fn recognize_cues_from_image(image_base64: String) -> Result<serde_json::Value, String> {
    let (cues, raw_text) = crate::ocr::get_ocr_engine().recognize_cues_from_image_base64(&image_base64);
    let count = cues.len();

    Ok(serde_json::json!({
        "raw_text": raw_text,
        "cues": cues,
        "count": count
    }))
}

#[tauri::command]
async fn add_scheduled_cues(state: State<'_, AppState>, cues: Vec<crate::cues::ScheduledCue>, append: bool) -> Result<Vec<crate::cues::ScheduledCue>, String> {
    let updated = state.ws.lookahead.add_cues(cues, append);
    let _ = crate::settings::save_saved_cues(&updated);
    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "scheduled_cues_updated",
        "cues": updated
    }).to_string());
    Ok(updated)
}

#[tauri::command]
async fn delete_scheduled_cue(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let deleted = state.ws.lookahead.delete_cue(&id);
    if deleted {
        let updated = state.ws.lookahead.get_cues();
        let _ = crate::settings::save_saved_cues(&updated);
        let _ = state.ws.broadcast_tx().send(serde_json::json!({
            "type": "scheduled_cues_updated",
            "cues": updated
        }).to_string());
    }
    Ok(deleted)
}

#[tauri::command]
async fn clear_scheduled_cues(state: State<'_, AppState>) -> Result<bool, String> {
    state.ws.lookahead.clear_cues();
    let _ = crate::settings::clear_saved_cues();
    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "scheduled_cues_updated",
        "cues": []
    }).to_string());
    Ok(true)
}

#[tauri::command]
async fn set_cues_config(
    state: State<'_, AppState>,
    pre_warning_seconds: Option<f64>,
    auto_censor: Option<bool>,
    append_mode: Option<bool>,
    cue_notifications: Option<bool>,
) -> Result<serde_json::Value, String> {
    if let Some(sec) = pre_warning_seconds {
        state.ws.lookahead.set_pre_warning_seconds(sec);
    }
    if let Some(ac) = auto_censor {
        state.ws.lookahead.set_auto_censor(ac);
    }
    if let Some(ap) = append_mode {
        state.ws.lookahead.set_append_cues_mode(ap);
    }
    if let Some(cn) = cue_notifications {
        state.ws.lookahead.set_notifications_enabled(cn);
    }

    crate::settings::update_cached_settings(|s| {
        if let Some(sec) = pre_warning_seconds { s.pre_warning_seconds = sec; }
        if let Some(ac) = auto_censor { s.auto_censor = ac; }
        if let Some(ap) = append_mode { s.append_mode = ap; }
        if let Some(cn) = cue_notifications { s.cue_notifications = cn; }
    });

    let config = serde_json::json!({
        "pre_warning_seconds": state.ws.lookahead.get_pre_warning_seconds(),
        "auto_censor": state.ws.lookahead.get_auto_censor(),
        "cue_notifications": state.ws.lookahead.get_notifications_enabled(),
        "append_mode": state.ws.lookahead.get_append_cues_mode()
    });

    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "cues_config_updated",
        "config": config
    }).to_string());

    Ok(config)
}

#[tauri::command]
async fn get_user_settings() -> Result<crate::settings::UserSettings, String> {
    Ok(crate::settings::get_cached_settings())
}

#[tauri::command]
async fn save_user_settings(settings: crate::settings::UserSettings) -> Result<crate::settings::UserSettings, String> {
    crate::settings::save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
async fn get_cues_config(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (pname, ptime, playing) = state.ws.lookahead.get_player_sync_state();
    Ok(serde_json::json!({
        "pre_warning_seconds": state.ws.lookahead.get_pre_warning_seconds(),
        "auto_censor": state.ws.lookahead.get_auto_censor(),
        "append_mode": state.ws.lookahead.get_append_cues_mode(),
        "player_name": pname,
        "player_time": ptime,
        "is_playing": playing,
        "cues_count": state.ws.lookahead.get_cues().len()
    }))
}

#[tauri::command]
async fn check_ocr_models_status() -> Result<crate::downloader::ModelsOverallStatus, String> {
    Ok(crate::downloader::ModelDownloader::check_ocr_status())
}

#[tauri::command]
async fn check_models_status() -> Result<crate::downloader::ModelsOverallStatus, String> {
    tokio::task::spawn_blocking(crate::downloader::ModelDownloader::check_models_status)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn minimize_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.hide().map_err(|e| format!("Failed to hide window: {}", e))?;
        println!("[blewred] Main window minimized to system tray.");
    }
    Ok(())
}

#[tauri::command]
fn exit_application(app: tauri::AppHandle) {
    println!("[blewred] Clean application exit requested by user.");
    app.exit(0);
}

#[tauri::command]
fn set_close_action(action: String) -> Result<(), String> {
    println!("[Settings] Setting close action to: {}", action);
    crate::settings::update_cached_settings(|s| {
        s.close_action = action;
    });
    Ok(())
}

#[tauri::command]
fn get_close_action() -> String {
    crate::settings::get_cached_settings().close_action
}

#[tauri::command]
fn toggle_lookahead_preview() -> Result<bool, String> {
    Ok(toggle_lookahead_preview_window())
}

#[tauri::command]
fn show_lookahead_preview() -> Result<(), String> {
    show_lookahead_preview_window();
    Ok(())
}

#[tauri::command]
fn hide_lookahead_preview() -> Result<(), String> {
    hide_lookahead_preview_window();
    Ok(())
}

#[tauri::command]
fn get_latest_lookahead_preview(state: State<'_, AppState>) -> Result<Option<crate::lookahead::LookaheadPreviewData>, String> {
    Ok(state.ws.lookahead.get_latest_preview())
}

#[tauri::command]
fn is_lookahead_preview_open() -> Result<bool, String> {
    Ok(is_lookahead_preview_window_visible())
}

#[tauri::command]
fn test_lookahead_preview_frame(state: State<'_, AppState>) -> Result<(), String> {
    static TOGGLE_THREAT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let is_threat = TOGGLE_THREAT.fetch_xor(true, std::sync::atomic::Ordering::Relaxed);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let test_preview = if is_threat {
        crate::lookahead::LookaheadPreviewData {
            player_id: "test-preview-player".to_string(),
            player_type: "Test Player (Lookahead)".to_string(),
            current_time_sec: 142.0,
            lookahead_time_sec: 152.0,
            lead_seconds: 10.0,
            score: 0.94,
            is_threat: true,
            severity: "OBVIOUS NSFW".to_string(),
            label: "EXPOSED_BREAST_F".to_string(),
            boxes: vec![crate::cascade::NormalizedBox {
                x1: 0.28,
                y1: 0.20,
                x2: 0.72,
                y2: 0.80,
                class_id: 3,
                label: "EXPOSED_BREAST_F".to_string(),
                score: 0.94,
            }],
            image_base64: "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='480' height='270' viewBox='0 0 480 270'><rect width='480' height='270' fill='%231f1013'/><rect x='10' y='10' width='460' height='250' fill='none' stroke='%23ff4141' stroke-dasharray='4'/><text x='240' y='125' fill='%23ff4141' font-size='18' font-family='sans-serif' font-weight='bold' text-anchor='middle'>blewred Lookahead</text><text x='240' y='155' fill='%23fca5a5' font-size='12' font-family='sans-serif' text-anchor='middle'>Lookahead NSFW detection: 94%</text><text x='240' y='180' fill='%23ef4444' font-size='11' font-family='sans-serif' font-weight='bold' text-anchor='middle'>DirectML Active • ViT + NudeNet 640m</text></svg>".to_string(),
            timestamp_ms: now_ms,
        }
    } else {
        crate::lookahead::LookaheadPreviewData {
            player_id: "test-preview-player".to_string(),
            player_type: "Test Player (Lookahead)".to_string(),
            current_time_sec: 142.0,
            lookahead_time_sec: 152.0,
            lead_seconds: 10.0,
            score: 0.08,
            is_threat: false,
            severity: "SAFE".to_string(),
            label: "Neutral".to_string(),
            boxes: vec![],
            image_base64: "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='480' height='270' viewBox='0 0 480 270'><rect width='480' height='270' fill='%2311141b'/><rect x='10' y='10' width='460' height='250' fill='none' stroke='%232b313c' stroke-dasharray='4'/><text x='240' y='125' fill='%2338bdf8' font-size='18' font-family='sans-serif' font-weight='bold' text-anchor='middle'>blewred Lookahead (+10s)</text><text x='240' y='155' fill='%2394a3b8' font-size='12' font-family='sans-serif' text-anchor='middle'>Lookahead video buffer active</text><text x='240' y='180' fill='%2310b981' font-size='11' font-family='sans-serif' text-anchor='middle'>DirectML Active • ViT + NudeNet: Safe (8%)</text></svg>".to_string(),
            timestamp_ms: now_ms,
        }
    };

    state.ws.lookahead.set_latest_preview(test_preview.clone());
    emit_lookahead_preview_frame(&test_preview);
    let _ = state.ws.broadcast_tx().send(serde_json::json!({
        "type": "lookahead_preview_frame",
        "preview": test_preview
    }).to_string());

    Ok(())
}

#[cfg(target_os = "windows")]
mod hotkeys {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use crate::obs::OBSClient;
    use crate::vision::VisionEngine;
    use crate::ws_bridge::WSBridge;

    #[link(name = "user32")]
    extern "system" {
        fn RegisterHotKey(hWnd: isize, id: i32, fsModifiers: u32, vk: u32) -> i32;
        fn UnregisterHotKey(hWnd: isize, id: i32) -> i32;
        fn GetMessageW(lpMsg: *mut Msg, hWnd: isize, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
        fn PostThreadMessageW(idThread: u32, Msg: u32, wParam: usize, lParam: isize) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: isize,
        message: u32,
        w_param: usize,
        l_param: isize,
        time: u32,
        pt: Point,
    }

    const WM_HOTKEY: u32 = 0x0312;
    const WM_RELOAD_HOTKEYS: u32 = 0x0400 + 77;
    static HOTKEY_THREAD_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    pub fn parse_vk_code(key: &str) -> u32 {
        let k = key.trim().to_uppercase();
        if k == "FN" {
            return 0x78; // Default F9
        }
        if k.starts_with('F') {
            if let Ok(num) = k[1..].parse::<u32>() {
                if (1..=24).contains(&num) {
                    return 0x6F + num;
                }
            }
        }
        if k.len() == 1 {
            let ch = k.chars().next().unwrap();
            if ch.is_ascii_digit() {
                return ch as u32;
            }
            if ch.is_ascii_alphabetic() {
                return ch as u32;
            }
        }
        match k.as_str() {
            "SPACE" => 0x20,
            "INSERT" => 0x2D,
            "DELETE" => 0x2E,
            "HOME" => 0x24,
            "END" => 0x23,
            "PAGEUP" => 0x21,
            "PAGEDOWN" => 0x22,
            "PAUSE" => 0x13,
            "TAB" => 0x09,
            "ENTER" | "RETURN" => 0x0D,
            "ESCAPE" | "ESC" => 0x1B,
            // Modifiers when used as standalone keys:
            "CTRL" | "CONTROL" => 0x11, // VK_CONTROL
            "LCTRL" | "LEFT CTRL" | "L_CTRL" | "CONTROLLEFT" => 0xA2, // VK_LCONTROL
            "RCTRL" | "RIGHT CTRL" | "R_CTRL" | "CONTROLRIGHT" => 0xA3, // VK_RCONTROL
            "ALT" | "MENU" => 0x12, // VK_MENU
            "LALT" | "LEFT ALT" | "L_ALT" | "ALTLEFT" => 0xA4, // VK_LMENU
            "RALT" | "RIGHT ALT" | "R_ALT" | "ALTRIGHT" => 0xA5, // VK_RMENU
            "SHIFT" => 0x10, // VK_SHIFT
            "LSHIFT" | "LEFT SHIFT" | "L_SHIFT" | "SHIFTLEFT" => 0xA0, // VK_LSHIFT
            "RSHIFT" | "RIGHT SHIFT" | "R_SHIFT" | "SHIFTRIGHT" => 0xA1, // VK_RSHIFT
            "WIN" | "WINDOWS" | "SUPER" | "META" | "LWIN" | "OSLEFT" => 0x5B, // VK_LWIN
            "RWIN" | "OSRIGHT" => 0x5C, // VK_RWIN
            // Arrow keys & Backspace:
            "UP" | "ARROWUP" => 0x26,
            "DOWN" | "ARROWDOWN" => 0x28,
            "LEFT" | "ARROWLEFT" => 0x25,
            "RIGHT" | "ARROWRIGHT" => 0x27,
            "BACKSPACE" | "BACK" => 0x08,
            "PRINTSCREEN" | "SNAPSHOT" => 0x2C,
            _ => 0x78, // default F9
        }
    }

    /// Parses multi-key hotkey combinations (e.g. "Ctrl+Shift+F9", "Left Ctrl", "Right Ctrl", "Alt+F8")
    /// into Win32 modifier flags and virtual key code.
    pub fn parse_hotkey_combo(combo: &str) -> (u32, u32) {
        let mut modifiers: u32 = 0x4000; // MOD_NOREPEAT by default
        let normalized = combo.replace('-', "+");
        let parts: Vec<&str> = normalized
            .split('+')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if parts.is_empty() {
            return (modifiers, 0x78); // Default F9
        }

        if parts.len() == 1 {
            let vk = parse_vk_code(parts[0]);
            return (modifiers, vk);
        }

        let mut base_key = "";
        for part in &parts {
            let p_upper = part.to_uppercase();
            match p_upper.as_str() {
                "CTRL" | "CONTROL" | "LEFT CTRL" | "RIGHT CTRL" | "LCTRL" | "RCTRL" | "CONTROLLEFT" | "CONTROLRIGHT" => {
                    modifiers |= 0x0002; // MOD_CONTROL
                }
                "ALT" | "LEFT ALT" | "RIGHT ALT" | "LALT" | "RALT" | "ALTLEFT" | "ALTRIGHT" => {
                    modifiers |= 0x0001; // MOD_ALT
                }
                "SHIFT" | "LEFT SHIFT" | "RIGHT SHIFT" | "LSHIFT" | "RSHIFT" | "SHIFTLEFT" | "SHIFTRIGHT" => {
                    modifiers |= 0x0004; // MOD_SHIFT
                }
                "WIN" | "WINDOWS" | "SUPER" | "META" | "LWIN" | "RWIN" | "OSLEFT" | "OSRIGHT" => {
                    modifiers |= 0x0008; // MOD_WIN
                }
                _ => {
                    base_key = part;
                }
            }
        }

        let vk = if base_key.is_empty() {
            let last = parts.last().unwrap();
            let last_upper = last.to_uppercase();
            if last_upper.contains("CTRL") || last_upper.contains("CONTROL") {
                modifiers &= !0x0002;
            } else if last_upper.contains("ALT") {
                modifiers &= !0x0001;
            } else if last_upper.contains("SHIFT") {
                modifiers &= !0x0004;
            } else if last_upper.contains("WIN") || last_upper.contains("META") {
                modifiers &= !0x0008;
            }
            parse_vk_code(last)
        } else {
            parse_vk_code(base_key)
        };
        (modifiers, vk)
    }

    unsafe fn register_hotkey_safe(id: i32, modifiers: u32, vk: u32) -> i32 {
        let mut res = RegisterHotKey(0, id, modifiers, vk);
        if res == 0 && (modifiers & 0x4000 != 0) {
            // Fallback without MOD_NOREPEAT (0x4000) for compatibility
            res = RegisterHotKey(0, id, modifiers & !0x4000, vk);
        }
        res
    }

    pub fn reload_global_hotkeys() {
        let tid = HOTKEY_THREAD_ID.load(std::sync::atomic::Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                PostThreadMessageW(tid, WM_RELOAD_HOTKEYS, 0, 0);
            }
        }
    }

    pub fn start_global_hotkeys(
        obs: Arc<OBSClient>,
        vision: Arc<VisionEngine>,
        ws: Arc<WSBridge>,
        fps_boosted: Arc<AtomicBool>,
    ) {
        std::thread::spawn(move || unsafe {
            HOTKEY_THREAD_ID.store(GetCurrentThreadId(), std::sync::atomic::Ordering::SeqCst);

            let init_s = crate::settings::get_cached_settings();
            if init_s.hotkey_scope != "local" {
                let (mod_p, vk_p) = parse_hotkey_combo(&init_s.hotkey_panic);
                let (mod_t, vk_t) = parse_hotkey_combo(&init_s.hotkey_threat);
                let reg_f9 = register_hotkey_safe(1, mod_p, vk_p);
                let reg_f8 = register_hotkey_safe(2, mod_t, vk_t);
                println!("[Hotkeys] Windows global hotkeys active: {} (mod: {:#x}, vk: {:#x})={}, {} (mod: {:#x}, vk: {:#x})={}", init_s.hotkey_panic, mod_p, vk_p, reg_f9 != 0, init_s.hotkey_threat, mod_t, vk_t, reg_f8 != 0);
            } else {
                println!("[Hotkeys] Hotkey scope is local on launch: global Windows hotkeys not registered");
            }

            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                if msg.message == WM_RELOAD_HOTKEYS {
                    UnregisterHotKey(0, 1);
                    UnregisterHotKey(0, 2);
                    let s = crate::settings::get_cached_settings();
                    if s.hotkey_scope != "local" {
                        let (mod_p, vk_p) = parse_hotkey_combo(&s.hotkey_panic);
                        let (mod_t, vk_t) = parse_hotkey_combo(&s.hotkey_threat);
                        let reg_p = register_hotkey_safe(1, mod_p, vk_p);
                        let reg_t = register_hotkey_safe(2, mod_t, vk_t);
                        println!("[Hotkeys] Re-registered Windows global hotkeys: {} (mod: {:#x})={}, {} (mod: {:#x})={}", s.hotkey_panic, mod_p, reg_p != 0, s.hotkey_threat, mod_t, reg_t != 0);
                    } else {
                        println!("[Hotkeys] Hotkey scope set to local: Windows global hotkeys unregistered");
                    }
                    continue;
                }

                if msg.message == WM_HOTKEY {
                    let s = crate::settings::get_cached_settings();
                    if s.hotkey_scope == "local" {
                        continue;
                    }
                    match msg.w_param {
                        1 => {
                            if ws.lookahead.get_active_ticket_count() > 0 {
                                if let Some(t) = ws.lookahead.resolve_latest("block") {
                                    println!("[Hotkeys] Global F9 confirmed lookahead ticket {}: BLOCK in OBS", t.ticket_id);
                                    crate::hide_lookahead_hud();
                                    let obs_ref = obs.clone();
                                    let tx_b = ws.broadcast_tx().clone();
                                    let reason = t.reason.clone();
                                    let player_type = t.player_type.clone();

                                    let now_ts = VisionEngine::get_current_timestamp_hms();

                                    if t.ticket_id.starts_with("cue-warn-") {
                                        let cue_id = t.ticket_id["cue-warn-".len()..].to_string();
                                        let reason_c = reason.clone();
                                        tauri::async_runtime::spawn(async move {
                                            crate::ws_bridge::update_censor_source(
                                                &obs_ref,
                                                &tx_b,
                                                crate::ws_bridge::CENSOR_SRC_SCHEDULED_CUE,
                                                true,
                                                &reason_c,
                                            ).await;
                                        });
                                        let inc_id = format!("cue-{}", cue_id);
                                        let inc_msg = crate::ws_bridge::make_incident_json(
                                            &inc_id,
                                            &now_ts,
                                            &format!("Player ({})", player_type),
                                            "CUE",
                                            &reason,
                                            1.0,
                                            &format!("Blocked by user: {}. Stream protection active.", reason),
                                            "ACTIVE",
                                            if crate::ws_bridge::is_censor_shield_enabled() { "Censor Shield + Mute" } else { "Screen Blocking + Mute" },
                                        );
                                        let _ = ws.broadcast_tx().send(inc_msg.to_string());
                                        let active_info = ws.lookahead.get_active_cue_info();
                                        let (cue_range, rem_sec, tot_sec) = if let Some(info) = active_info {
                                            (info.range, info.remaining_sec, info.total_duration_sec)
                                        } else if let Some(cue) = ws.lookahead.get_cues().iter().find(|c| c.id == cue_id) {
                                            let cur_t = ws.lookahead.get_player_sync_state().1;
                                            let rem = (cue.end_sec - cur_t).max(0.0);
                                            let tot = (cue.end_sec - cue.start_sec).max(0.1);
                                            (cue.formatted_range.clone(), rem, tot)
                                        } else {
                                            ("".to_string(), 0.0, 0.0)
                                        };

                                        let bcast = serde_json::json!({
                                            "type": "scheduled_cue_started",
                                            "cue_id": cue_id,
                                            "reason": reason,
                                            "range": cue_range,
                                            "player_name": player_type,
                                            "remaining_sec": rem_sec,
                                            "total_sec": tot_sec,
                                            "incident": inc_msg["incident"]
                                        });
                                        let _ = ws.broadcast_tx().send(bcast.to_string());
                                    } else {
                                        let ticket_id_res = t.ticket_id.clone();
                                        tauri::async_runtime::spawn(async move {
                                            crate::ws_bridge::update_censor_source(
                                                &obs_ref,
                                                &tx_b,
                                                crate::ws_bridge::CENSOR_SRC_LOOKAHEAD_DUE,
                                                true,
                                                &reason,
                                            ).await;
                                            tokio::time::sleep(tokio::time::Duration::from_millis(3500)).await;
                                            crate::ws_bridge::update_censor_source(
                                                &obs_ref,
                                                &tx_b,
                                                crate::ws_bridge::CENSOR_SRC_LOOKAHEAD_DUE,
                                                false,
                                                "STREAM SAFE",
                                            ).await;
                                            let res_msg = serde_json::json!({
                                                "type": "incident_resolved",
                                                "incident_id": format!("lookahead-{}", ticket_id_res),
                                                "incident_type": "LOOKAHEAD",
                                                "message": "Preemptive blocking completed, screen restored"
                                            });
                                            let _ = tx_b.send(res_msg.to_string());
                                        });
                                    }

                                    let update = serde_json::json!({
                                        "type": "lookahead_ticket_resolved",
                                        "ticket_id": t.ticket_id,
                                        "action": "block"
                                    });
                                    let _ = ws.broadcast_tx().send(update.to_string());
                                    continue;
                                }
                            }

                            println!("[Hotkeys] Global panic hotkey triggered: toggling Censor Shield");
                            let obs_ref = obs.clone();
                            let ws_ref = ws.clone();
                            tauri::async_runtime::spawn(async move {
                                let new_state = crate::ws_bridge::WSBridge::toggle_emergency_shield(
                                    obs_ref,
                                    ws_ref.broadcast_tx(),
                                    "Manual Panic Trigger"
                                ).await;
                                println!("[Hotkeys] Emergency shield toggled via global hotkey -> active={}", new_state);
                            });
                        }
                        2 => {
                            if ws.lookahead.get_active_ticket_count() > 0 {
                                if let Some(t) = ws.lookahead.resolve_latest("allow") {
                                    println!("[Hotkeys] Global F8 resolved lookahead ticket {}: ALLOW / FALSE ALARM", t.ticket_id);
                                    crate::hide_lookahead_hud();
                                    let update = serde_json::json!({
                                        "type": "lookahead_ticket_resolved",
                                        "ticket_id": t.ticket_id,
                                        "action": "allow"
                                    });
                                    let _ = ws.broadcast_tx().send(update.to_string());
                                    continue;
                                }
                            }

                            let prev = fps_boosted.load(Ordering::Relaxed);
                            let next = !prev;
                            fps_boosted.store(next, Ordering::Relaxed);
                            vision.cascade.set_boost_mode(next);
                            println!("[Hotkeys] Global F8 triggered: FPS Boost={} (Cascade direct dual-scan synchronized)", next);
                            ws.broadcast_fps_mode(next);
                        }
                        _ => {}
                    }
                }
            }
        });
    }
}

#[cfg(target_os = "windows")]
pub use hotkeys::reload_global_hotkeys;

#[cfg(not(target_os = "windows"))]
pub fn reload_global_hotkeys() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut user_settings = crate::settings::get_cached_settings();
    let monitors = vision::VisionEngine::enumerate_monitors();
    let primary_idx = monitors.iter().find(|m| m.is_primary).map(|m| m.index).unwrap_or(0);

    // Validate selected_monitor against current connected monitors:
    // If the saved monitor no longer exists (e.g. broken or disconnected display),
    // automatically update to the active Primary Monitor and persist to settings.
    if !monitors.iter().any(|m| m.index == user_settings.selected_monitor) {
        println!(
            "[Monitor] Saved monitor index {} does not exist in active displays; updating to Primary Monitor (index {})",
            user_settings.selected_monitor, primary_idx
        );
        user_settings.selected_monitor = primary_idx;
        crate::settings::update_cached_settings(|s| s.selected_monitor = primary_idx);
    }

    // Validate hud_monitor against current displays:
    if user_settings.hud_monitor != -1 && !monitors.iter().any(|m| m.index == user_settings.hud_monitor) {
        println!(
            "[HUD] Saved HUD monitor index {} does not exist in active displays; resetting to Auto (-1)",
            user_settings.hud_monitor
        );
        user_settings.hud_monitor = -1;
        crate::settings::update_cached_settings(|s| s.hud_monitor = -1);
    }

    set_hud_monitor_index(user_settings.hud_monitor);

    let mut lexical_engine = LexicalEngine::new();
    let stopwords = crate::settings::load_stopwords();
    lexical_engine.load_words_from_text(&stopwords);

    let lexical_arc = Arc::new(Mutex::new(lexical_engine));
    let obs_client = Arc::new(OBSClient::new("127.0.0.1".to_string(), 4455));

    let vision_engine = Arc::new(VisionEngine::new(lexical_arc.clone()));
    vision_engine.set_selected_monitor(user_settings.selected_monitor);
    vision_engine.set_nsfw_threshold(user_settings.nsfw_threshold);
    vision_engine.set_censor_categories(user_settings.censor_categories);
    vision_engine.set_ocr_enabled(user_settings.ocr_enabled);
    vision_engine.cascade.set_boost_mode(user_settings.fps_boosted);

    let fps_boosted = Arc::new(AtomicBool::new(user_settings.fps_boosted));

    crate::ws_bridge::set_censor_shield_enabled(user_settings.censor_shield_enabled);
    crate::ws_bridge::set_shield_donate_enabled(user_settings.shield_donate_enabled);
    crate::ws_bridge::sync_censor_shield_html_donate(user_settings.shield_donate_enabled);
    crate::ws_bridge::set_operation_mode(user_settings.operation_mode);

    // WebSocket server for Browser Extension, OCR and UI telemetry
    let ws_bridge = Arc::new(WSBridge::new(
        lexical_arc.clone(),
        obs_client.clone(),
        vision_engine.clone(),
        fps_boosted.clone(),
        51789,
    ));

    ws_bridge.lookahead.set_pre_warning_seconds(user_settings.pre_warning_seconds);
    ws_bridge.lookahead.set_auto_censor(user_settings.auto_censor);
    ws_bridge.lookahead.set_notifications_enabled(user_settings.cue_notifications);
    ws_bridge.lookahead.set_append_cues_mode(user_settings.append_mode);

    let saved_cues = crate::settings::load_saved_cues();
    if !saved_cues.is_empty() {
        ws_bridge.lookahead.add_cues(saved_cues, false);
    }

    // Start Windows Global Hotkeys (F8 / F9)
    #[cfg(target_os = "windows")]
    hotkeys::start_global_hotkeys(obs_client.clone(), vision_engine.clone(), ws_bridge.clone(), fps_boosted.clone());

    let app_state = AppState {
        lexical: lexical_arc.clone(),
        obs: obs_client.clone(),
        vision: vision_engine.clone(),
        ws: ws_bridge.clone(),
        fps_boosted: fps_boosted.clone(),
    };

    let ws_bridge_start = ws_bridge.clone();
    let obs_client_reconnect = obs_client.clone();
    let ws_bridge_tray = ws_bridge.clone();
    let obs_client_tray = obs_client.clone();
    let vision_engine_tray = vision_engine.clone();

    tauri::Builder::default()
        .manage(app_state)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let settings = crate::settings::get_cached_settings();
                    match settings.close_action.as_str() {
                        "tray" => {
                            println!("[blewred] Main window close requested: minimizing to tray per user settings.");
                            let _ = window.hide();
                        }
                        "exit" => {
                            println!("[blewred] Main window close requested: cleanly terminating process per user settings.");
                            std::process::exit(0);
                        }
                        _ => {
                            println!("[blewred] Main window close requested: prompting user (ask mode).");
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                            let _ = window.emit("blewred://request-close", ());
                        }
                    }
                } else if window.label() == "lookahead_preview" {
                    api.prevent_close();
                    let _ = window.hide();
                    LOOKAHEAD_PREVIEW_VISIBLE.store(false, Ordering::SeqCst);
                } else if window.label() == "lookahead_hud" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            APP_HANDLE.set(app.handle().clone()).ok();
            let user_settings = crate::settings::get_cached_settings();

            // Setup System Tray Icon & Context Menu
            let show_item = MenuItem::with_id(app, "show", "Show blewred", true, None::<&str>)?;
            let hide_item = MenuItem::with_id(app, "hide", "Minimize to Tray", true, None::<&str>)?;
            let preview_win_item = MenuItem::with_id(app, "toggle_lookahead_preview", "Lookahead Video Window", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            // 1. Operation Mode Submenu
            let mode_sub = Submenu::new(app, "Operation Mode", true)?;
            let mode_0 = CheckMenuItem::with_id(app, "mode_0", "Hybrid (Screen + Player)", true, user_settings.operation_mode == 0, None::<&str>)?;
            let mode_1 = CheckMenuItem::with_id(app, "mode_1", "Player Analysis (Lookahead)", true, user_settings.operation_mode == 1, None::<&str>)?;
            let mode_2 = CheckMenuItem::with_id(app, "mode_2", "Screen Analysis (Desktop)", true, user_settings.operation_mode == 2, None::<&str>)?;
            let mode_3 = CheckMenuItem::with_id(app, "mode_3", "Protection Off (Standby)", true, user_settings.operation_mode == 3, None::<&str>)?;

            mode_sub.append(&mode_0)?;
            mode_sub.append(&mode_1)?;
            mode_sub.append(&mode_2)?;
            mode_sub.append(&mode_3)?;

            let mode_items = vec![
                (0, mode_0),
                (1, mode_1),
                (2, mode_2),
                (3, mode_3),
            ];

            // 2. Screen Capture Monitor Submenu (Physical monitors ONLY)
            let screen_mon_sub = Submenu::new(app, "Screen Capture Monitor", true)?;
            let mut screen_mon_items = Vec::new();
            let monitors = vision::VisionEngine::enumerate_monitors();
            for m in &monitors {
                let id = format!("screen_mon_{}", m.index);
                let text = if m.is_primary {
                    format!("Monitor {} (Primary) — {}×{}", m.index + 1, m.width, m.height)
                } else {
                    format!("Monitor {} — {}×{}", m.index + 1, m.width, m.height)
                };
                let checked = user_settings.selected_monitor == m.index;
                let item = CheckMenuItem::with_id(app, &id, &text, true, checked, None::<&str>)?;
                screen_mon_sub.append(&item)?;
                screen_mon_items.push((m.index, item));
            }

            // 3. HUD Alert Monitor Submenu
            let hud_mon_sub = Submenu::new(app, "Monitor for HUD", true)?;
            let mut hud_mon_items = Vec::new();
            let auto_hud_item = CheckMenuItem::with_id(app, "hud_mon_-1", "Auto (Streamer Screen)", true, user_settings.hud_monitor == -1, None::<&str>)?;
            hud_mon_sub.append(&auto_hud_item)?;
            hud_mon_items.push((-1, auto_hud_item));

            for m in &monitors {
                let id = format!("hud_mon_{}", m.index);
                let text = if m.is_primary {
                    format!("Monitor {} (Primary) — {}×{}", m.index + 1, m.width, m.height)
                } else {
                    format!("Monitor {} — {}×{}", m.index + 1, m.width, m.height)
                };
                let checked = user_settings.hud_monitor == m.index;
                let item = CheckMenuItem::with_id(app, &id, &text, true, checked, None::<&str>)?;
                hud_mon_sub.append(&item)?;
                hud_mon_items.push((m.index, item));
            }

            // 4. Move Window Monitor Submenu
            let move_win_sub = Submenu::new(app, "Move Window to...", true)?;
            let move_auto = MenuItem::with_id(app, "move_win_-1", "Primary Monitor", true, None::<&str>)?;
            move_win_sub.append(&move_auto)?;
            for m in &monitors {
                let id = format!("move_win_{}", m.index);
                let text = format!("Monitor {} ({}) — {}×{}", m.index + 1, m.name, m.width, m.height);
                let item = MenuItem::with_id(app, &id, &text, true, None::<&str>)?;
                move_win_sub.append(&item)?;
            }

            let tray_state = TrayMenuState {
                mode_items,
                screen_mon_items,
                hud_mon_items,
            };
            let _ = TRAY_STATE.set(std::sync::Mutex::new(tray_state));

            let menu = Menu::new(app)?;
            menu.append(&show_item)?;
            menu.append(&hide_item)?;
            menu.append(&preview_win_item)?;
            menu.append(&sep1)?;
            menu.append(&mode_sub)?;
            menu.append(&screen_mon_sub)?;
            menu.append(&hud_mon_sub)?;
            menu.append(&move_win_sub)?;
            menu.append(&sep2)?;
            menu.append(&quit_item)?;

            let icon_bytes = include_bytes!("../icons/32x32.png");
            let tray_icon_img = tauri::image::Image::from_bytes(icon_bytes)?;

            let tray = TrayIconBuilder::with_id("main_tray")
                .icon(tray_icon_img)
                .tooltip("blewred")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    let id_str = event.id.as_ref();
                    match id_str {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                ensure_window_on_primary_monitor(&w);
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                        "hide" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                        "toggle_lookahead_preview" => {
                            toggle_lookahead_preview_window();
                        }
                        "quit" => {
                            println!("[blewred] Tray menu exit requested.");
                            std::process::exit(0);
                        }
                        "mode_0" | "mode_1" | "mode_2" | "mode_3" => {
                            let mode: u8 = match id_str {
                                "mode_0" => 0,
                                "mode_1" => 1,
                                "mode_2" => 2,
                                "mode_3" => 3,
                                _ => 0,
                            };
                            update_tray_mode(mode);
                            crate::settings::update_cached_settings(|s| s.operation_mode = mode);
                            let obs = obs_client_tray.clone();
                            let ws = ws_bridge_tray.clone();
                            let vis = vision_engine_tray.clone();
                            tauri::async_runtime::spawn(async move {
                                crate::ws_bridge::safe_set_operation_mode(mode, &obs, &ws.broadcast_tx(), &ws.lookahead, &vis).await;
                            });
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.emit("blewred://operation-mode-changed", mode);
                            }
                        }
                        _ => {
                            if let Some(mon_str) = id_str.strip_prefix("screen_mon_") {
                                if let Ok(idx) = mon_str.parse::<i32>() {
                                    update_tray_screen_monitor(idx);
                                    vision_engine_tray.set_selected_monitor(idx);
                                    crate::settings::update_cached_settings(|s| s.selected_monitor = idx);
                                    println!("[Tray] Screen capture monitor changed to: {}", idx);
                                    if let Some(w) = app.get_webview_window("main") {
                                        let _ = w.emit("blewred://monitor-changed", idx);
                                    }
                                }
                            } else if let Some(hud_str) = id_str.strip_prefix("hud_mon_") {
                                if let Ok(idx) = hud_str.parse::<i32>() {
                                    update_tray_hud_monitor(idx);
                                    set_hud_monitor_index(idx);
                                    crate::settings::update_cached_settings(|s| s.hud_monitor = idx);
                                    println!("[Tray] HUD alert monitor changed to: {}", idx);
                                    if let Some(w) = app.get_webview_window("main") {
                                        let _ = w.emit("blewred://hud-monitor-changed", idx);
                                    }
                                }
                            } else if let Some(mon_str) = id_str.strip_prefix("move_win_") {
                                if let Ok(idx) = mon_str.parse::<i32>() {
                                    if let Some(w) = app.get_webview_window("main") {
                                        place_window_on_monitor_by_index(&w, idx);
                                    }
                                }
                            }
                        }
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            ensure_window_on_primary_monitor(&w);
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            let _ = TRAY_HOLDER.set(tray);

            let window = match app.get_webview_window("main") {
                Some(w) => w,
                None => {
                    println!("[Setup] 'main' window not pre-created, building dynamically...");
                    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
                        .title("blewred")
                        .inner_size(1040.0, 650.0)
                        .min_inner_size(850.0, 520.0)
                        .resizable(true)
                        .visible(true)
                        .center()
                        .build()?
                }
            };

            // Strictly place, adapt and center the main window on the Windows Primary Monitor on EVERY launch
            ensure_window_on_primary_monitor(&window);
            let app_icon_res = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))
                .or_else(|_| tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png")));
            if let Ok(icon) = &app_icon_res {
                let _ = window.set_icon(icon.clone());
                if let Some(preview_win) = app.get_webview_window("lookahead_preview") {
                    let _ = preview_win.set_icon(icon.clone());
                }
                if let Some(hud_win) = app.get_webview_window("lookahead_hud") {
                    let _ = hud_win.set_icon(icon.clone());
                }
            }
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
            println!("[Setup] Main window displayed, centered and focused successfully on Primary Monitor.");

            tauri::async_runtime::spawn(async move {
                obs_client_reconnect.start_reconnect_loop();
                ws_bridge_start.start().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_telemetry,
            get_rules_text,
            update_rules_text,
            test_text,
            test_screen_censorship,
            run_ocr_test,
            run_nsfw_test,
            emergency_mute,
            toggle_fps_boost,
            set_fps_boost,
            auto_setup_obs,
            check_obs_status,
            launch_obs_studio,
            get_monitors,
            set_monitor,
            set_hud_monitor,
            get_hud_monitor,
            reposition_to_primary_monitor,
            get_operation_mode,
            set_operation_mode,
            test_lookahead_alert,
            toggle_realtime_guard,
            set_nsfw_threshold,
            set_censor_categories,
            toggle_censor_shield,
            toggle_shield_donate,
            toggle_emergency_shield,
            toggle_ocr,
            open_external_url,
            get_scheduled_cues,
            recognize_cues_from_image,
            add_scheduled_cues,
            delete_scheduled_cue,
            clear_scheduled_cues,
            set_cues_config,
            get_cues_config,
            check_ocr_models_status,
            check_models_status,
            get_user_settings,
            save_user_settings,
            open_windows_graphics_settings,
            open_extension_folder,
            open_docs_guide,
            set_hide_setup_guide,
            set_language,
            minimize_to_tray,
            exit_application,
            set_close_action,
            get_close_action,
            toggle_lookahead_preview,
            show_lookahead_preview,
            hide_lookahead_preview,
            get_latest_lookahead_preview,
            is_lookahead_preview_open,
            test_lookahead_preview_frame
        ])
        .run(tauri::generate_context!())
        .map_err(|e| {
            let msg = format!("Tauri application run error: {}", e);
            let _ = std::fs::write("crash.log", &msg);
            msg
        })
        .expect("error while running tauri application");
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::hotkeys::{parse_hotkey_combo, parse_vk_code};

    #[test]
    fn test_parse_vk_code_function_keys() {
        assert_eq!(parse_vk_code("F1"), 0x70);
        assert_eq!(parse_vk_code("F8"), 0x77);
        assert_eq!(parse_vk_code("F9"), 0x78);
        assert_eq!(parse_vk_code("F12"), 0x7B);
        assert_eq!(parse_vk_code("Fn"), 0x78);
    }

    #[test]
    fn test_parse_vk_code_special_and_chars() {
        assert_eq!(parse_vk_code("SPACE"), 0x20);
        assert_eq!(parse_vk_code("A"), 'A' as u32);
        assert_eq!(parse_vk_code("1"), '1' as u32);
        assert_eq!(parse_vk_code("INSERT"), 0x2D);
    }

    #[test]
    fn test_parse_hotkey_combo_modifiers() {
        // Plain key with default MOD_NOREPEAT (0x4000)
        let (mod_f9, vk_f9) = parse_hotkey_combo("F9");
        assert_eq!(mod_f9, 0x4000);
        assert_eq!(vk_f9, 0x78);

        // Ctrl + Shift + F9 (0x4000 | 0x0002 | 0x0004 = 0x4006)
        let (mod_cs, vk_cs) = parse_hotkey_combo("Ctrl+Shift+F9");
        assert_eq!(mod_cs, 0x4000 | 0x0002 | 0x0004);
        assert_eq!(vk_cs, 0x78);

        // Ctrl-Shift-Fn (using dash separator as requested by user)
        let (mod_dash, vk_dash) = parse_hotkey_combo("Ctrl-Shift-Fn");
        assert_eq!(mod_dash, 0x4000 | 0x0002 | 0x0004);
        assert_eq!(vk_dash, 0x78);

        // Alt + F8 (0x4000 | 0x0001 = 0x4001)
        let (mod_alt, vk_alt) = parse_hotkey_combo("Alt + F8");
        assert_eq!(mod_alt, 0x4000 | 0x0001);
        assert_eq!(vk_alt, 0x77);

        // Standalone Left Ctrl, Right Ctrl, Ctrl
        let (mod_lctrl, vk_lctrl) = parse_hotkey_combo("Left Ctrl");
        assert_eq!(mod_lctrl, 0x4000);
        assert_eq!(vk_lctrl, 0xA2);

        let (mod_rctrl, vk_rctrl) = parse_hotkey_combo("Right Ctrl");
        assert_eq!(mod_rctrl, 0x4000);
        assert_eq!(vk_rctrl, 0xA3);

        let (mod_ctrl, vk_ctrl) = parse_hotkey_combo("Ctrl");
        assert_eq!(mod_ctrl, 0x4000);
        assert_eq!(vk_ctrl, 0x11);

        // Ctrl + Alt + Shift + Win + F12
        let (mod_all, vk_all) = parse_hotkey_combo("Ctrl+Alt+Shift+Win+F12");
        assert_eq!(mod_all, 0x4000 | 0x0002 | 0x0001 | 0x0004 | 0x0008);
        assert_eq!(vk_all, 0x7B);
    }

    #[test]
    fn test_inspect_monitors() {
        let monitors = crate::vision::VisionEngine::enumerate_monitors();
        println!("=== VisionEngine::enumerate_monitors ===");
        for m in &monitors {
            println!("  index: {}, id: '{}', name: '{}', primary: {}, pos: ({},{}), size: {}x{}",
                m.index, m.id, m.name, m.is_primary, m.x, m.y, m.width, m.height);
        }
    }
}
