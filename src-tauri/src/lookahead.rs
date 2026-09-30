use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use base64::Engine;
use image::GenericImageView;
use serde::{Deserialize, Serialize};

use crate::cascade::{CascadeEngine, NormalizedBox};
use crate::cues::{ScheduledCue, CueStatus, format_seconds};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookaheadFramePacket {
    pub player_id: String,
    pub player_type: String,
    pub current_time_sec: f64,
    pub lookahead_time_sec: f64,
    pub width: u32,
    pub height: u32,
    pub image_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookaheadIncidentTicket {
    pub ticket_id: String,
    pub player_type: String,
    pub eta_seconds: f64,
    pub severity: String,
    pub reason: String,
    pub score: f32,
    pub raw_image_base64: String,
    pub boxes: Vec<NormalizedBox>,
    pub created_at_ms: u64,
    pub execute_at_ms: u64,
    pub status: String, // "pending", "confirmed_censor", "allowed"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookaheadPreviewData {
    pub player_id: String,
    pub player_type: String,
    pub current_time_sec: f64,
    pub lookahead_time_sec: f64,
    pub lead_seconds: f64,
    pub score: f32,
    pub is_threat: bool,
    pub severity: String,
    pub label: String,
    pub boxes: Vec<NormalizedBox>,
    pub image_base64: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone)]
struct RecentTrigger {
    timestamp: Instant,
    _score: f32,
}

pub struct LookaheadEngine {
    cascade: Arc<CascadeEngine>,
    active_tickets: Mutex<HashMap<String, LookaheadIncidentTicket>>,
    recent_triggers: Mutex<Vec<RecentTrigger>>,
    next_ticket_id: Mutex<u64>,
    last_ticket_time: Mutex<Instant>,
    latest_preview: Mutex<Option<LookaheadPreviewData>>,
    // Scheduled Cues & Player Tracking
    scheduled_cues: Mutex<Vec<ScheduledCue>>,
    pre_warning_seconds: Mutex<f64>,
    auto_censor_enabled: std::sync::atomic::AtomicBool,
    append_cues_mode: std::sync::atomic::AtomicBool,
    notifications_enabled: std::sync::atomic::AtomicBool,
    last_player_time: Mutex<f64>,
    last_player_name: Mutex<String>,
    is_player_playing: std::sync::atomic::AtomicBool,
    active_censor_cue_id: Mutex<Option<String>>,
    last_player_sync_ts: std::sync::atomic::AtomicU64,
}

impl LookaheadEngine {
    pub fn new(cascade: Arc<CascadeEngine>) -> Self {
        Self {
            cascade,
            active_tickets: Mutex::new(HashMap::new()),
            recent_triggers: Mutex::new(Vec::new()),
            next_ticket_id: Mutex::new(1),
            last_ticket_time: Mutex::new(Instant::now().checked_sub(std::time::Duration::from_secs(60)).unwrap_or_else(Instant::now)),
            latest_preview: Mutex::new(None),
            scheduled_cues: Mutex::new(Vec::new()),
            pre_warning_seconds: Mutex::new(20.0),
            auto_censor_enabled: std::sync::atomic::AtomicBool::new(true),
            append_cues_mode: std::sync::atomic::AtomicBool::new(true),
            notifications_enabled: std::sync::atomic::AtomicBool::new(true),
            last_player_time: Mutex::new(0.0),
            last_player_name: Mutex::new("Waiting for player...".to_string()),
            is_player_playing: std::sync::atomic::AtomicBool::new(false),
            active_censor_cue_id: Mutex::new(None),
            last_player_sync_ts: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Process an upcoming frame from browser extension lookahead buffer (+5 to +10s),
    /// producing both continuous preview telemetry and optional warning tickets on violation.
    pub fn process_frame_with_preview(&self, packet: LookaheadFramePacket) -> (Option<LookaheadIncidentTicket>, Option<LookaheadPreviewData>) {
        let clean_b64 = if let Some(idx) = packet.image_base64.find("base64,") {
            &packet.image_base64[idx + 7..]
        } else {
            &packet.image_base64
        };

        let raw_bytes = match base64::engine::general_purpose::STANDARD.decode(clean_b64) {
            Ok(b) => b,
            Err(_) => return (None, None),
        };
        let dyn_img = match image::load_from_memory(&raw_bytes) {
            Ok(img) => img,
            Err(_) => return (None, None),
        };
        let (w, h) = dyn_img.dimensions();
        if w == 0 || h == 0 {
            return (None, None);
        }

        // Convert to BGRA format expected by CascadeEngine
        let rgba_img = dyn_img.to_rgba8();
        let rgba_raw = rgba_img.as_raw();
        let mut bgra_pixels = Vec::with_capacity((w * h * 4) as usize);
        for chunk in rgba_raw.chunks_exact(4) {
            bgra_pixels.push(chunk[2]); // B
            bgra_pixels.push(chunk[1]); // G
            bgra_pixels.push(chunk[0]); // R
            bgra_pixels.push(chunk[3]); // A
        }

        // Run through cascaded ViT + NudeNet 640m engine (top-down orientation)
        let cascade_res = self.cascade.classify_frame(&bgra_pixels, w, h, 65, true);
        let score = cascade_res.score;
        let now = Instant::now();

        // 1. Maintain sliding window of triggers (last 2.5 seconds) for burst / frequency analysis
        let mut burst_count = 0;
        if let Ok(mut trigs) = self.recent_triggers.lock() {
            trigs.retain(|t| now.duration_since(t.timestamp).as_secs_f32() < 2.5);
            if score >= 0.25 {
                trigs.push(RecentTrigger { timestamp: now, _score: score });
            }
            burst_count = trigs.len();
        }

        // 2. Frequency & Ambiguity Heuristic:
        let is_obvious = score >= 0.70 || cascade_res.boxes.iter().any(|b| b.score >= 0.65 && (b.class_id == 3 || b.class_id == 4 || b.class_id == 14));
        let is_suspicious = !is_obvious && (score >= 0.30 || (burst_count >= 2 && score >= 0.22));
        let is_threat = is_obvious || is_suspicious;

        let diff = packet.lookahead_time_sec - packet.current_time_sec;
        let eta_sec = if diff >= 0.5 { diff } else { 0.0 };
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let severity = if is_obvious {
            "OBVIOUS NSFW".to_string()
        } else if is_suspicious {
            "SUSPICIOUS CONTENT (UNCERTAIN)".to_string()
        } else {
            "SAFE".to_string()
        };

        let primary_label = if !cascade_res.boxes.is_empty() {
            cascade_res.boxes[0].label.clone()
        } else if !cascade_res.primary_label.is_empty() && cascade_res.primary_label != "Neutral" {
            cascade_res.primary_label.clone()
        } else {
            "Neutral".to_string()
        };

        // Format raw image thumbnail with data URL prefix for clean rendering in blewred UI
        let thumb_url = if packet.image_base64.starts_with("data:image/") {
            packet.image_base64.clone()
        } else {
            format!("data:image/jpeg;base64,{}", clean_b64)
        };

        let preview = LookaheadPreviewData {
            player_id: packet.player_id.clone(),
            player_type: packet.player_type.clone(),
            current_time_sec: packet.current_time_sec,
            lookahead_time_sec: packet.lookahead_time_sec,
            lead_seconds: (eta_sec * 10.0).round() / 10.0,
            score,
            is_threat,
            severity: severity.clone(),
            label: primary_label.clone(),
            boxes: cascade_res.boxes.clone(),
            image_base64: thumb_url.clone(),
            timestamp_ms: now_ms,
        };

        if let Ok(mut prev_lock) = self.latest_preview.lock() {
            *prev_lock = Some(preview.clone());
        }

        if !is_threat {
            return (None, Some(preview));
        }

        // Prevent ticket flooding: at least 2.5s between tickets of same scene unless score jumps significantly
        {
            if let Ok(mut last_t) = self.last_ticket_time.lock() {
                if now.duration_since(*last_t).as_secs_f32() < 2.5 && !is_obvious {
                    return (None, Some(preview));
                }
                *last_t = now;
            }
        }

        let ticket_id_num = {
            let mut id_lock = self.next_ticket_id.lock().unwrap();
            let cur = *id_lock;
            *id_lock += 1;
            cur
        };

        let execute_at_ms = if eta_sec > 0.0 {
            now_ms + (eta_sec * 1000.0) as u64
        } else {
            now_ms
        };

        let reason = if !cascade_res.boxes.is_empty() {
            format!("{}: {}", severity, cascade_res.boxes[0].label)
        } else if !cascade_res.primary_label.is_empty() && cascade_res.primary_label != "Neutral" {
            format!("{}: {}", severity, cascade_res.primary_label)
        } else {
            format!("{}: Spike detected ({:.0}%)", severity, score * 100.0)
        };

        let ticket = LookaheadIncidentTicket {
            ticket_id: format!("ticket-{}", ticket_id_num),
            player_type: packet.player_type,
            eta_seconds: (eta_sec * 10.0).round() / 10.0,
            severity,
            reason,
            score,
            raw_image_base64: thumb_url,
            boxes: cascade_res.boxes,
            created_at_ms: now_ms,
            execute_at_ms,
            status: "pending".to_string(),
        };

        if let Ok(mut map) = self.active_tickets.lock() {
            map.insert(ticket.ticket_id.clone(), ticket.clone());
        }

        (Some(ticket), Some(preview))
    }

    /// Process an upcoming frame from browser extension lookahead buffer (+5 to +10s)
    pub fn process_frame(&self, packet: LookaheadFramePacket) -> Option<LookaheadIncidentTicket> {
        self.process_frame_with_preview(packet).0
    }

    pub fn get_latest_preview(&self) -> Option<LookaheadPreviewData> {
        self.latest_preview.lock().ok().and_then(|p| p.clone())
    }

    pub fn set_latest_preview(&self, preview: LookaheadPreviewData) {
        if let Ok(mut p) = self.latest_preview.lock() {
            *p = Some(preview);
        }
    }

    /// Record a ticket directly (useful for tests and manual alerts)
    pub fn record_ticket(&self, ticket: LookaheadIncidentTicket) {
        if let Ok(mut map) = self.active_tickets.lock() {
            map.insert(ticket.ticket_id.clone(), ticket);
        }
    }

    /// Clear all active test tickets to reset previous test state
    pub fn clear_test_tickets(&self) {
        if let Ok(mut map) = self.active_tickets.lock() {
            map.retain(|id, _| !id.starts_with("test_"));
        }
    }

    /// Cancel/remove a specific ticket from the queue
    pub fn cancel_ticket(&self, ticket_id: &str) {
        if let Ok(mut map) = self.active_tickets.lock() {
            map.remove(ticket_id);
        }
    }

    /// Streamer decision: "block" (F9 / Confirm) or "allow" (F8 / False Alarm / Skip)
    pub fn resolve_ticket(&self, ticket_id: &str, action: &str) -> Option<LookaheadIncidentTicket> {
        let mut target_cue_id = None;
        if ticket_id.starts_with("cue-warn-") {
            let cue_id = &ticket_id["cue-warn-".len()..];
            if let Ok(mut cues) = self.scheduled_cues.lock() {
                if let Some(cue) = cues.iter_mut().find(|c| c.id == cue_id) {
                    if action == "allow" {
                        cue.status = CueStatus::Dismissed;
                    } else if action == "block" {
                        cue.status = CueStatus::BlockedByUser;
                        target_cue_id = Some(cue.id.clone());
                    }
                }
            }
        }

        if let Some(cid) = target_cue_id {
            if let Ok(mut active_cue_lock) = self.active_censor_cue_id.lock() {
                *active_cue_lock = Some(cid);
            }
        }

        if let Ok(mut map) = self.active_tickets.lock() {
            if let Some(t) = map.get_mut(ticket_id) {
                if action == "block" {
                    t.status = "confirmed_censor".to_string();
                } else if action == "allow" {
                    t.status = "allowed".to_string();
                }
                return Some(t.clone());
            }
        }
        None
    }

    /// Resolve the most urgent pending ticket with action
    pub fn resolve_latest(&self, action: &str) -> Option<LookaheadIncidentTicket> {
        let mut target_id = None;
        if let Ok(map) = self.active_tickets.lock() {
            let mut pending_keys: Vec<String> = map
                .iter()
                .filter(|(_, t)| t.status == "pending")
                .map(|(k, _)| k.clone())
                .collect();
            target_id = pending_keys.pop();
        }

        if let Some(key) = target_id {
            return self.resolve_ticket(&key, action);
        }
        None
    }

    /// Retrieve pending cues whose ETA has arrived and must execute in OBS
    pub fn poll_due_cues(&self) -> Vec<LookaheadIncidentTicket> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut due = Vec::new();
        if let Ok(mut map) = self.active_tickets.lock() {
            let mut to_remove = Vec::new();
            for (id, ticket) in map.iter_mut() {
                if ticket.status == "allowed" {
                    to_remove.push(id.clone());
                } else if now_ms >= ticket.execute_at_ms {
                    // Due for censorship in OBS!
                    due.push(ticket.clone());
                    to_remove.push(id.clone());
                } else if now_ms > ticket.execute_at_ms + 10000 {
                    to_remove.push(id.clone());
                }
            }
            for k in to_remove {
                map.remove(&k);
            }
        }
        due
    }

    pub fn get_active_ticket_count(&self) -> usize {
        self.active_tickets.lock().map(|m| m.values().filter(|t| t.status == "pending").count()).unwrap_or(0)
    }

    /// Process periodic player synchronization (every 400ms from browser extension)
    /// Checks warning horizons (e.g. 20s before timestamp) and active cue ranges.
    pub fn handle_player_sync(&self, player_name: &str, current_time: f64, _duration: f64, is_playing: bool) -> CueSyncResult {
        let prev_time = if let Ok(mut t) = self.last_player_time.lock() {
            let old = *t;
            *t = current_time;
            old
        } else {
            current_time
        };

        if let Ok(mut name) = self.last_player_name.lock() {
            *name = player_name.to_string();
        }
        self.is_player_playing.store(is_playing, std::sync::atomic::Ordering::Relaxed);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_player_sync_ts.store(now_ms, std::sync::atomic::Ordering::Relaxed);

        let pre_warn = self.pre_warning_seconds.lock().map(|s| *s).unwrap_or(20.0);
        let auto_censor = self.auto_censor_enabled.load(std::sync::atomic::Ordering::Relaxed);

        // Detect user seeking / scrubbing: time jump > 1.2s or backward movement > 0.5s
        let is_seek = (current_time - prev_time).abs() > 1.2 || (prev_time > 0.0 && current_time < prev_time - 0.5);

        let mut warning_ticket = None;
        let mut currently_in_range_cue_id: Option<String> = None;
        let mut currently_in_range_reason = String::new();
        let mut currently_in_range_range = String::new();
        let mut currently_in_range_remaining: Option<f64> = None;
        let mut currently_in_range_duration: Option<f64> = None;

        if let Ok(mut cues) = self.scheduled_cues.lock() {
            for cue in cues.iter_mut() {
                // If playhead moved before the warning window, re-arm cue to Pending
                if current_time < (cue.start_sec - pre_warn - 1.0) {
                    if cue.status != CueStatus::Pending {
                        cue.status = CueStatus::Pending;
                    }
                }

                // 1. Check Warning Horizon: [start_sec - pre_warn .. start_sec)
                if current_time >= (cue.start_sec - pre_warn) && current_time < cue.start_sec {
                    let eta = (cue.start_sec - current_time).max(0.1);
                    // Emit warning if:
                    // - cue is Pending (natural approach or newly navigated into warning zone)
                    // - user scrubbed / jumped to a new position inside the warning zone
                    // - or cue was previously Completed / in ActiveCensor and user rewound back into the warning zone
                    // Do NOT warn if already confirmed BlockedByUser or Dismissed!
                    let should_warn = (cue.status == CueStatus::Pending
                        || is_seek
                        || cue.status == CueStatus::Completed
                        || cue.status == CueStatus::ActiveCensor)
                        && cue.status != CueStatus::BlockedByUser
                        && cue.status != CueStatus::Dismissed;

                    if should_warn {
                        cue.status = CueStatus::Approaching;
                        let eta_rounded = (eta * 10.0).round() / 10.0;
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let ticket = LookaheadIncidentTicket {
                            ticket_id: format!("cue-warn-{}", cue.id),
                            player_type: player_name.to_string(),
                            eta_seconds: eta_rounded,
                            severity: "PLANNED BLOCKING".to_string(),
                            reason: format!("Timing {}-{} ({}) in {:.1}s", 
                                format_seconds(cue.start_sec), 
                                format_seconds(cue.end_sec), 
                                cue.reason, 
                                eta_rounded),
                            score: 1.0,
                            raw_image_base64: "".to_string(),
                            boxes: Vec::new(),
                            created_at_ms: now_ms,
                            execute_at_ms: now_ms + (eta * 1000.0) as u64,
                            status: "pending".to_string(),
                        };

                        if let Ok(mut map) = self.active_tickets.lock() {
                            map.insert(ticket.ticket_id.clone(), ticket.clone());
                        }
                        warning_ticket = Some(ticket);
                    }
                }

                // 2. Check Active Horizon & Blocked by User
                let default_reason = if cue.reason.trim().is_empty() {
                    format!("Timing {}", cue.formatted_range)
                } else {
                    cue.reason.clone()
                };

                if cue.status == CueStatus::BlockedByUser {
                    // Confirmed blocked by streamer: stays actively blocking until video is outside range
                    if current_time <= cue.end_sec && current_time >= (cue.start_sec - pre_warn - 1.0) {
                        currently_in_range_cue_id = Some(cue.id.clone());
                        currently_in_range_reason = default_reason.clone();
                        currently_in_range_range = cue.formatted_range.clone();
                        currently_in_range_remaining = Some((cue.end_sec - current_time).max(0.0));
                        currently_in_range_duration = Some((cue.end_sec - cue.start_sec.min(current_time)).max(0.1));
                    } else if current_time > cue.end_sec {
                        cue.status = CueStatus::Completed;
                    }
                } else if current_time >= cue.start_sec && current_time <= cue.end_sec {
                    if cue.status != CueStatus::Dismissed {
                        if auto_censor {
                            cue.status = CueStatus::ActiveCensor;
                        }
                        currently_in_range_cue_id = Some(cue.id.clone());
                        currently_in_range_reason = default_reason;
                        currently_in_range_range = cue.formatted_range.clone();
                        currently_in_range_remaining = Some((cue.end_sec - current_time).max(0.0));
                        currently_in_range_duration = Some((cue.end_sec - cue.start_sec.min(current_time)).max(0.1));
                    }
                } else if current_time > cue.end_sec {
                    if cue.status == CueStatus::ActiveCensor {
                        cue.status = CueStatus::Completed;
                    }
                }
            }
        }

        let mut should_censor_now = false;
        let mut should_uncensor_now = false;
        let mut ended_cue_id: Option<String> = None;

        if let Ok(mut active_cue_lock) = self.active_censor_cue_id.lock() {
            if let Some(ref in_range_id) = currently_in_range_cue_id {
                let is_user_blocked = self.scheduled_cues.lock()
                    .map(|cues| cues.iter().any(|c| c.id == *in_range_id && c.status == CueStatus::BlockedByUser))
                    .unwrap_or(false);

                if auto_censor || is_user_blocked {
                    if active_cue_lock.as_ref() != Some(in_range_id) {
                        *active_cue_lock = Some(in_range_id.clone());
                        should_censor_now = true;
                    }
                } else {
                    if active_cue_lock.is_some() {
                        ended_cue_id = active_cue_lock.take();
                        should_uncensor_now = true;
                    }
                }
            } else {
                // Playhead is outside any active or user-blocked cue interval
                if active_cue_lock.is_some() {
                    ended_cue_id = active_cue_lock.take();
                    should_uncensor_now = true;
                }
            }
        }

        CueSyncResult {
            warning_ticket,
            should_censor_now,
            should_uncensor_now,
            active_cue_id: currently_in_range_cue_id,
            ended_cue_id,
            active_cue_reason: currently_in_range_reason,
            active_cue_range: currently_in_range_range,
            active_cue_player: player_name.to_string(),
            remaining_sec: currently_in_range_remaining,
            total_duration_sec: currently_in_range_duration,
        }
    }

    pub fn get_cues(&self) -> Vec<ScheduledCue> {
        self.scheduled_cues.lock().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_cues(&self, cues: Vec<ScheduledCue>) {
        if let Ok(mut c) = self.scheduled_cues.lock() {
            *c = cues;
        }
    }

    pub fn add_cues(&self, new_cues: Vec<ScheduledCue>, append: bool) -> Vec<ScheduledCue> {
        if let Ok(mut c) = self.scheduled_cues.lock() {
            if !append {
                c.clear();
            }
            c.extend(new_cues);
            c.sort_by(|a, b| a.start_sec.partial_cmp(&b.start_sec).unwrap_or(std::cmp::Ordering::Equal));
            c.clone()
        } else {
            Vec::new()
        }
    }

    pub fn delete_cue(&self, cue_id: &str) -> bool {
        if let Ok(mut c) = self.scheduled_cues.lock() {
            let len_before = c.len();
            c.retain(|cue| cue.id != cue_id);
            c.len() < len_before
        } else {
            false
        }
    }

    pub fn clear_cues(&self) {
        if let Ok(mut c) = self.scheduled_cues.lock() {
            c.clear();
        }
    }

    pub fn set_pre_warning_seconds(&self, seconds: f64) {
        if let Ok(mut s) = self.pre_warning_seconds.lock() {
            *s = seconds.max(3.0).min(120.0);
        }
    }

    pub fn get_pre_warning_seconds(&self) -> f64 {
        self.pre_warning_seconds.lock().map(|s| *s).unwrap_or(20.0)
    }

    pub fn set_auto_censor(&self, enabled: bool) {
        self.auto_censor_enabled.store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_auto_censor(&self) -> bool {
        self.auto_censor_enabled.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_auto_censor_enabled(&self, enabled: bool) {
        self.set_auto_censor(enabled);
    }

    pub fn is_auto_censor_enabled(&self) -> bool {
        self.get_auto_censor()
    }

    pub fn set_append_cues_mode(&self, append: bool) {
        self.append_cues_mode.store(append, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_append_cues_mode(&self) -> bool {
        self.append_cues_mode.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_notifications_enabled(&self, enabled: bool) {
        self.notifications_enabled.store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_notifications_enabled(&self) -> bool {
        self.notifications_enabled.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn is_notifications_enabled(&self) -> bool {
        self.get_notifications_enabled()
    }

    pub fn get_player_sync_state(&self) -> (String, f64, bool) {
        let name = self.last_player_name.lock().map(|n| n.clone()).unwrap_or_else(|_| "Unknown".to_string());
        let time = self.last_player_time.lock().map(|t| *t).unwrap_or(0.0);
        let playing = self.is_player_playing.load(std::sync::atomic::Ordering::Relaxed);
        (name, time, playing)
    }

    pub fn is_player_active(&self) -> bool {
        let last = self.last_player_sync_ts.load(std::sync::atomic::Ordering::Relaxed);
        if last == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now.saturating_sub(last) < 4000
    }

    pub fn clear_active_tickets(&self) {
        if let Ok(mut map) = self.active_tickets.lock() {
            map.clear();
        }
    }

    /// Retrieve detailed progress metadata for the currently active censor cue (if any)
    pub fn get_active_cue_info(&self) -> Option<ActiveCueInfo> {
        let cur_time = self.last_player_time.lock().map(|t| *t).unwrap_or(0.0);
        let cues = self.scheduled_cues.lock().ok()?;

        // 1. First priority: Check explicitly marked active_censor_cue_id
        if let Ok(active_id_lock) = self.active_censor_cue_id.lock() {
            if let Some(ref active_id) = *active_id_lock {
                if let Some(cue) = cues.iter().find(|c| c.id == *active_id) {
                    let remaining_sec = (cue.end_sec - cur_time).max(0.0);
                    let total_duration_sec = (cue.end_sec - cue.start_sec.min(cur_time)).max(0.1);
                    let reason = if cue.reason.trim().is_empty() {
                        format!("Timing {}", cue.formatted_range)
                    } else {
                        cue.reason.clone()
                    };
                    return Some(ActiveCueInfo {
                        cue_id: cue.id.clone(),
                        reason,
                        range: cue.formatted_range.clone(),
                        remaining_sec,
                        total_duration_sec,
                        current_time: cur_time,
                    });
                }
            }
        }

        // 2. Secondary priority / Fallback: Detect if playhead is currently inside ANY scheduled timing
        let pre_warn = self.get_pre_warning_seconds();
        if let Some(cue) = cues.iter().find(|c| {
            (cur_time >= c.start_sec && cur_time <= c.end_sec && c.status != CueStatus::Dismissed && c.status != CueStatus::Completed)
                || (c.status == CueStatus::BlockedByUser && cur_time <= c.end_sec && cur_time >= (c.start_sec - pre_warn - 1.0))
        }) {
            let remaining_sec = (cue.end_sec - cur_time).max(0.0);
            let total_duration_sec = (cue.end_sec - cue.start_sec.min(cur_time)).max(0.1);
            let reason = if cue.reason.trim().is_empty() {
                format!("Timing {}", cue.formatted_range)
            } else {
                cue.reason.clone()
            };
            return Some(ActiveCueInfo {
                cue_id: cue.id.clone(),
                reason,
                range: cue.formatted_range.clone(),
                remaining_sec,
                total_duration_sec,
                current_time: cur_time,
            });
        }

        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCueInfo {
    pub cue_id: String,
    pub reason: String,
    pub range: String,
    pub remaining_sec: f64,
    pub total_duration_sec: f64,
    pub current_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueSyncResult {
    pub warning_ticket: Option<LookaheadIncidentTicket>,
    pub should_censor_now: bool,
    pub should_uncensor_now: bool,
    pub active_cue_id: Option<String>,
    pub ended_cue_id: Option<String>,
    pub active_cue_reason: String,
    pub active_cue_range: String,
    pub active_cue_player: String,
    pub remaining_sec: Option<f64>,
    pub total_duration_sec: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clear_test_tickets_preserves_real_tickets() {
        let cascade = Arc::new(CascadeEngine::new());
        let engine = LookaheadEngine::new(cascade);

        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

        let test_ticket = LookaheadIncidentTicket {
            ticket_id: format!("test_{}", now_ms),
            player_type: "Test".to_string(),
            eta_seconds: 8.0,
            severity: "high".to_string(),
            reason: "Test reason".to_string(),
            score: 0.9,
            raw_image_base64: String::new(),
            boxes: Vec::new(),
            created_at_ms: now_ms,
            execute_at_ms: now_ms + 8000,
            status: "pending".to_string(),
        };

        let real_ticket = LookaheadIncidentTicket {
            ticket_id: "cue-warn-123".to_string(),
            player_type: "Alloha".to_string(),
            eta_seconds: 15.0,
            severity: "high".to_string(),
            reason: "Real cue warning".to_string(),
            score: 0.9,
            raw_image_base64: String::new(),
            boxes: Vec::new(),
            created_at_ms: now_ms,
            execute_at_ms: now_ms + 15000,
            status: "pending".to_string(),
        };

        engine.record_ticket(test_ticket);
        engine.record_ticket(real_ticket);

        assert_eq!(engine.get_active_ticket_count(), 2);

        // Reset test state
        engine.clear_test_tickets();

        // Should only have the real ticket remaining
        assert_eq!(engine.get_active_ticket_count(), 1);
        let active = engine.active_tickets.lock().unwrap();
        assert!(active.contains_key("cue-warn-123"));
    }

    #[test]
    fn test_player_sync_scrubbing_and_warning_horizon() {
        let cascade = Arc::new(CascadeEngine::new());
        let engine = LookaheadEngine::new(cascade);

        // Pre-warning is set to 20 seconds
        engine.set_pre_warning_seconds(20.0);

        // Add a scheduled cue at 01:00 - 01:30 (60s to 90s)
        let cue = ScheduledCue {
            id: "cue_test_1".to_string(),
            start_sec: 60.0,
            end_sec: 90.0,
            duration_sec: 30.0,
            formatted_range: "01:00 – 01:30".to_string(),
            reason: "Prohibited fragment".to_string(),
            raw_text: "01:00-01:30 Prohibited fragment".to_string(),
            status: CueStatus::Pending,
        };
        engine.set_cues(vec![cue]);

        // 1. Playhead far before warning horizon (e.g. 10.0s) -> no warning
        let res1 = engine.handle_player_sync("YouTube", 10.0, 300.0, true);
        assert!(res1.warning_ticket.is_none());
        assert!(!res1.should_censor_now);

        // 2. Playhead reaches warning horizon (40.0s, exactly 20s before start_sec 60.0) -> warning emitted
        let res2 = engine.handle_player_sync("YouTube", 40.0, 300.0, true);
        assert!(res2.warning_ticket.is_some());
        let t2 = res2.warning_ticket.unwrap();
        assert_eq!(t2.eta_seconds, 20.0);
        assert!(t2.reason.contains("20.0s") || t2.reason.contains("20s"));

        // 3. Normal progression 40.4s (no scrub) -> does NOT spam new ticket
        let res3 = engine.handle_player_sync("YouTube", 40.4, 300.0, true);
        assert!(res3.warning_ticket.is_none());

        // 4. USER SCRUBS FORWARD: jumps to 55.0s (5.0 seconds before cue!) while PAUSED (is_playing = false)
        // Warning MUST pop up with exactly 5.0 seconds remaining!
        let res4 = engine.handle_player_sync("YouTube", 55.0, 300.0, false);
        assert!(res4.warning_ticket.is_some(), "Warning must trigger when scrubbing into window even when paused");
        let t4 = res4.warning_ticket.unwrap();
        assert_eq!(t4.eta_seconds, 5.0);
        assert!(t4.reason.contains("5.0s"));

        // 5. Playhead enters active cue (60.5s) -> triggers censorship
        let res5 = engine.handle_player_sync("YouTube", 60.5, 300.0, true);
        assert!(res5.should_censor_now);
        assert_eq!(res5.active_cue_id.as_deref(), Some("cue_test_1"));

        // 6. Playhead exits cue (91.0s) -> uncensors
        let res6 = engine.handle_player_sync("YouTube", 91.0, 300.0, true);
        assert!(res6.should_uncensor_now);
        assert!(res6.active_cue_id.is_none());

        // 7. USER REWINDS back to 52.0s (8 seconds before cue) -> MUST RE-ARM AND WARN WITH 8.0s!
        let res7 = engine.handle_player_sync("YouTube", 52.0, 300.0, false);
        assert!(res7.warning_ticket.is_some(), "Rewinding into warning window must re-arm and emit warning ticket");
        let t7 = res7.warning_ticket.unwrap();
        assert_eq!(t7.eta_seconds, 8.0);
        assert!(t7.reason.contains("8.0s"));
    }

    #[test]
    fn test_player_sync_blocked_by_user_remains_active_until_outside_range() {
        let cascade = Arc::new(CascadeEngine::new());
        let engine = LookaheadEngine::new(cascade);
        engine.set_pre_warning_seconds(20.0);

        let cue = ScheduledCue {
            id: "cue_user_block".to_string(),
            start_sec: 100.0,
            end_sec: 140.0,
            duration_sec: 40.0,
            formatted_range: "01:40 – 02:20".to_string(),
            reason: "Prohibited section".to_string(),
            raw_text: "100-140 Prohibited section".to_string(),
            status: CueStatus::Pending,
        };
        engine.set_cues(vec![cue]);

        // 1. Playhead enters warning zone at 85.0s (15s before start)
        let res1 = engine.handle_player_sync("YouTube", 85.0, 500.0, true);
        assert!(res1.warning_ticket.is_some());

        // 2. Streamer clicks "Block" (F9 / block) on the warning ticket
        let resolved = engine.resolve_ticket("cue-warn-cue_user_block", "block");
        assert!(resolved.is_some());

        // Verify status is BlockedByUser
        let cues = engine.get_cues();
        assert_eq!(cues[0].status, CueStatus::BlockedByUser);

        // 3. Playhead advances at 86.0s (still 14s before cue start):
        // Active block MUST be retained! should_uncensor_now must be FALSE!
        let res2 = engine.handle_player_sync("YouTube", 86.0, 500.0, true);
        assert!(!res2.should_uncensor_now, "Blocked by user must NOT uncensor prematurely");
        assert_eq!(res2.active_cue_id.as_deref(), Some("cue_user_block"));

        // 4. Playhead enters normal cue interval (115.0s):
        // Continues to be active!
        let res3 = engine.handle_player_sync("YouTube", 115.0, 500.0, true);
        assert!(!res3.should_uncensor_now);
        assert_eq!(res3.active_cue_id.as_deref(), Some("cue_user_block"));

        // 5. Playhead nears the end (139.9s):
        let res4 = engine.handle_player_sync("YouTube", 139.9, 500.0, true);
        assert!(!res4.should_uncensor_now);
        assert_eq!(res4.active_cue_id.as_deref(), Some("cue_user_block"));

        // 6. Playhead exits the cue interval (140.5s):
        // NOW and ONLY NOW it should uncensor!
        let res5 = engine.handle_player_sync("YouTube", 140.5, 500.0, true);
        assert!(res5.should_uncensor_now, "Must uncensor when playhead moves outside timing range");
        assert!(res5.active_cue_id.is_none());
        assert_eq!(res5.ended_cue_id.as_deref(), Some("cue_user_block"));

        // Verify status transitioned to Completed
        let cues_end = engine.get_cues();
        assert_eq!(cues_end[0].status, CueStatus::Completed);
    }

    #[test]
    fn test_player_sync_remaining_seconds_and_cue_info() {
        let cascade = Arc::new(CascadeEngine::new());
        let engine = LookaheadEngine::new(cascade);
        engine.set_auto_censor(true);

        let cue = ScheduledCue {
            id: "cue_timing_test".to_string(),
            start_sec: 50.0,
            end_sec: 80.0,
            duration_sec: 30.0,
            formatted_range: "00:50 – 01:20".to_string(),
            reason: "Scheduled spoiler".to_string(),
            raw_text: "50-80 Scheduled spoiler".to_string(),
            status: CueStatus::Pending,
        };
        engine.set_cues(vec![cue]);

        // 1. Before cue: no active cue info, no remaining
        let res_before = engine.handle_player_sync("YouTube", 20.0, 300.0, true);
        assert!(res_before.remaining_sec.is_none());
        assert!(res_before.total_duration_sec.is_none());
        assert!(engine.get_active_cue_info().is_none());

        // 2. Playhead enters cue at 55.0s:
        let res_start = engine.handle_player_sync("YouTube", 55.0, 300.0, true);
        assert!(res_start.should_censor_now);
        assert_eq!(res_start.active_cue_id.as_deref(), Some("cue_timing_test"));
        assert_eq!(res_start.remaining_sec, Some(25.0)); // 80.0 - 55.0 = 25.0s remaining
        assert_eq!(res_start.total_duration_sec, Some(30.0));

        // Verify get_active_cue_info returns accurate live state
        let info = engine.get_active_cue_info().expect("active cue info must be present");
        assert_eq!(info.cue_id, "cue_timing_test");
        assert_eq!(info.reason, "Scheduled spoiler");
        assert_eq!(info.range, "00:50 – 01:20");
        assert_eq!(info.remaining_sec, 25.0);
        assert_eq!(info.total_duration_sec, 30.0);

        // 3. Playhead advances to 72.5s:
        let res_mid = engine.handle_player_sync("YouTube", 72.5, 300.0, true);
        assert_eq!(res_mid.remaining_sec, Some(7.5)); // 80.0 - 72.5 = 7.5s remaining
        let info_mid = engine.get_active_cue_info().unwrap();
        assert_eq!(info_mid.remaining_sec, 7.5);

        // 4. Playhead exits cue at 80.5s:
        let res_exit = engine.handle_player_sync("YouTube", 80.5, 300.0, true);
        assert!(res_exit.should_uncensor_now);
        assert!(res_exit.active_cue_id.is_none());
        assert!(res_exit.remaining_sec.is_none());
        assert!(engine.get_active_cue_info().is_none());
    }

    #[test]
    fn test_player_sync_third_timing_countdown() {
        let cascade = Arc::new(CascadeEngine::new());
        let engine = LookaheadEngine::new(cascade);
        engine.set_auto_censor(true);

        let cue1 = ScheduledCue {
            id: "cue_1".to_string(),
            start_sec: 10.0,
            end_sec: 25.0,
            duration_sec: 15.0,
            formatted_range: "00:10 – 00:25".to_string(),
            reason: "First timing".to_string(),
            raw_text: "10-25 First timing".to_string(),
            status: CueStatus::Pending,
        };
        let cue2 = ScheduledCue {
            id: "cue_2".to_string(),
            start_sec: 60.0,
            end_sec: 90.0,
            duration_sec: 30.0,
            formatted_range: "01:00 – 01:30".to_string(),
            reason: "Second timing".to_string(),
            raw_text: "60-90 Second timing".to_string(),
            status: CueStatus::Pending,
        };
        let cue3 = ScheduledCue {
            id: "cue_3".to_string(),
            start_sec: 150.0,
            end_sec: 200.0,
            duration_sec: 50.0,
            formatted_range: "02:30 – 03:20".to_string(),
            reason: "".to_string(), // Empty reason: must fallback to "Timing 02:30 – 03:20"
            raw_text: "150-200".to_string(),
            status: CueStatus::Pending,
        };
        engine.set_cues(vec![cue1, cue2, cue3]);

        // Player directly jumps into timing #3 at 170.0s
        let res = engine.handle_player_sync("Twitch", 170.0, 1000.0, true);
        assert!(res.should_censor_now);
        assert_eq!(res.active_cue_id.as_deref(), Some("cue_3"));
        assert_eq!(res.active_cue_reason, "Timing 02:30 – 03:20");
        assert_eq!(res.remaining_sec, Some(30.0)); // 200 - 170 = 30s remaining
        assert_eq!(res.total_duration_sec, Some(50.0));

        // get_active_cue_info must return timing 3 with remaining 30s
        let active_info = engine.get_active_cue_info().expect("cue 3 must be active");
        assert_eq!(active_info.cue_id, "cue_3");
        assert_eq!(active_info.reason, "Timing 02:30 – 03:20");
        assert_eq!(active_info.range, "02:30 – 03:20");
        assert_eq!(active_info.remaining_sec, 30.0);
        assert_eq!(active_info.total_duration_sec, 50.0);

        // Player continues inside timing 3 at 185.5s
        let res2 = engine.handle_player_sync("Twitch", 185.5, 1000.0, true);
        assert_eq!(res2.remaining_sec, Some(14.5));
        let active_info2 = engine.get_active_cue_info().unwrap();
        assert_eq!(active_info2.remaining_sec, 14.5);
    }

    #[test]
    fn test_lookahead_engine_ticket_lifecycle() {
        let cascade = Arc::new(CascadeEngine::new());
        let engine = LookaheadEngine::new(cascade);

        assert_eq!(engine.get_active_ticket_count(), 0);

        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let ticket = LookaheadIncidentTicket {
            ticket_id: "test_ticket_1".to_string(),
            player_type: "Alloha".to_string(),
            eta_seconds: 5.0,
            severity: "high".to_string(),
            reason: "Exposed female breast".to_string(),
            score: 0.88,
            raw_image_base64: String::new(),
            boxes: vec![],
            created_at_ms: now_ms,
            execute_at_ms: now_ms + 100,
            status: "pending".to_string(),
        };

        engine.record_ticket(ticket);
        assert_eq!(engine.get_active_ticket_count(), 1);

        let resolved = engine.resolve_ticket("test_ticket_1", "block");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().status, "confirmed_censor");

        let ticket2 = LookaheadIncidentTicket {
            ticket_id: "test_ticket_2".to_string(),
            player_type: "Turbo".to_string(),
            eta_seconds: 4.0,
            severity: "medium".to_string(),
            reason: "Suspicious content (Flash)".to_string(),
            score: 0.55,
            raw_image_base64: String::new(),
            boxes: vec![],
            created_at_ms: now_ms,
            execute_at_ms: now_ms + 50,
            status: "pending".to_string(),
        };

        engine.record_ticket(ticket2);
        let resolved_latest = engine.resolve_latest("allow");
        assert!(resolved_latest.is_some());
        assert_eq!(resolved_latest.unwrap().ticket_id, "test_ticket_2");
    }

    #[test]
    fn test_lookahead_preview_data_storage_and_retrieval() {
        let cascade = Arc::new(CascadeEngine::new());
        let engine = LookaheadEngine::new(cascade);

        assert!(engine.get_latest_preview().is_none());

        let preview = LookaheadPreviewData {
            player_id: "test-p1".to_string(),
            player_type: "YouTube".to_string(),
            current_time_sec: 100.0,
            lookahead_time_sec: 110.0,
            lead_seconds: 10.0,
            score: 0.05,
            is_threat: false,
            severity: "SAFE".to_string(),
            label: "Neutral".to_string(),
            boxes: vec![],
            image_base64: "data:image/jpeg;base64,123".to_string(),
            timestamp_ms: 12345678,
        };

        engine.set_latest_preview(preview.clone());
        let retrieved = engine.get_latest_preview().expect("Preview should be cached");
        assert_eq!(retrieved.player_id, "test-p1");
        assert_eq!(retrieved.player_type, "YouTube");
        assert_eq!(retrieved.lead_seconds, 10.0);
        assert!(!retrieved.is_threat);
        assert_eq!(retrieved.score, 0.05);
    }
}
