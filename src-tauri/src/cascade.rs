use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ort::{session::Session, value::Value};
use serde::{Deserialize, Serialize};

use crate::vision::{
    is_explicit_nudenet_class, is_suggestive_nudenet_class, nudenet_class_label,
    run_nudenet_nms, RawBox,
};

pub const VIT_INPUT_SIZE: usize = 224;
pub const NUDENET_INPUT_SIZE: usize = 640;

pub const OBS_PLUGIN_PORT: u16 = 51799;
pub const OBS_HEARTBEAT_PORT: u16 = 51798;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CensorCategories {
    pub genitalia: bool,
    pub breasts: bool,
    pub buttocks: bool,
    pub underwear: bool,
    pub body_exposed: bool,
}

impl Default for CensorCategories {
    fn default() -> Self {
        Self {
            genitalia: true,
            breasts: true,
            buttocks: true,
            underwear: false,
            body_exposed: false,
        }
    }
}

impl CensorCategories {
    pub fn is_class_allowed(&self, class_id: usize) -> bool {
        match class_id {
            4 | 14 => self.genitalia,
            3 => self.breasts,
            2 | 6 => self.buttocks,
            0 | 15 | 16 | 17 => self.underwear,
            5 | 7 | 8 | 9 | 10 | 11 | 13 => self.body_exposed,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub class_id: usize,
    pub label: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeResult {
    pub is_violation: bool,
    pub score: f32,
    pub primary_label: String,
    pub detailed_reason: String,
    pub censor_all: bool,
    pub boxes: Vec<NormalizedBox>,
    pub stage1_score: f32,
    pub stage1_label: String,
    pub stage2_score: f32,
    pub latency_ms: f32,
    pub obs_plugin_active: bool,
}

#[derive(Serialize)]
struct ObsPacket<'a> {
    censor_all: bool,
    score: f32,
    reason: &'a str,
    boxes: &'a [NormalizedBox],
    ts: u64,
}

#[repr(C, packed)]
struct BlewRedShmHeader {
    magic: u32,
    version: u32,
    width: u32,
    height: u32,
    stride: u32,
    format: u32,
    frame_index: u64,
    timestamp_ms: u64,
    reserved: [u32; 8],
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenFileMappingW(desired_access: u32, inherit_handle: i32, name: *const u16) -> isize;
    fn MapViewOfFile(handle: isize, desired_access: u32, offset_high: u32, offset_low: u32, num_bytes: usize) -> *mut u8;
    fn UnmapViewOfFile(addr: *const u8) -> i32;
    fn CloseHandle(handle: isize) -> i32;
}

pub struct ObsShmReader {
    handle: isize,
    mapped_ptr: *mut u8,
    last_frame_index: u64,
    cached_frame: Option<(u32, u32, Vec<u8>)>,
}

unsafe impl Send for ObsShmReader {}
unsafe impl Sync for ObsShmReader {}

impl ObsShmReader {
    pub fn new() -> Self {
        Self {
            handle: 0,
            mapped_ptr: std::ptr::null_mut(),
            last_frame_index: 0,
            cached_frame: None,
        }
    }

    pub fn try_read_frame(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        unsafe {
            if self.mapped_ptr.is_null() {
                let name: Vec<u16> = "Local\\BlewRed_OBS_Frame_SHM\0".encode_utf16().collect();
                let h = OpenFileMappingW(4 /* FILE_MAP_READ */, 0, name.as_ptr());
                if h != 0 {
                    let total_size = std::mem::size_of::<BlewRedShmHeader>() + (640 * 360 * 4);
                    let ptr = MapViewOfFile(h, 4, 0, 0, total_size);
                    if !ptr.is_null() {
                        self.handle = h;
                        self.mapped_ptr = ptr;
                        println!("[ObsShmReader] Connected to OBS video source Shared Memory stream!");
                    } else {
                        CloseHandle(h);
                        return None;
                    }
                } else {
                    return None;
                }
            }

            if self.mapped_ptr.is_null() {
                return None;
            }

            let hdr = &*(self.mapped_ptr as *const BlewRedShmHeader);
            if hdr.magic != 0x424C5752 {
                return None;
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            if now.saturating_sub(hdr.timestamp_ms) > 2000 {
                return None;
            }

            if hdr.frame_index == self.last_frame_index {
                // If frame has not updated yet, reuse the cached OBS GPU frame
                // instead of dropping back to GDI desktop capture!
                return self.cached_frame.clone();
            }

            self.last_frame_index = hdr.frame_index;
            let w = hdr.width;
            let h = hdr.height;
            let len = (w * h * 4) as usize;
            let pixel_ptr = self.mapped_ptr.add(std::mem::size_of::<BlewRedShmHeader>());
            let slice = std::slice::from_raw_parts(pixel_ptr, len);
            let frame = (w, h, slice.to_vec());
            self.cached_frame = Some(frame.clone());
            Some(frame)
        }
    }
}

impl Drop for ObsShmReader {
    fn drop(&mut self) {
        unsafe {
            if !self.mapped_ptr.is_null() {
                UnmapViewOfFile(self.mapped_ptr);
                self.mapped_ptr = std::ptr::null_mut();
            }
            if self.handle != 0 {
                CloseHandle(self.handle);
                self.handle = 0;
            }
        }
    }
}

pub struct CascadeEngine {
    vit_session: Mutex<Option<Session>>,
    nude640_session: Mutex<Option<Session>>,
    udp_socket: Mutex<Option<UdpSocket>>,
    obs_attached: AtomicBool,
    last_heartbeat: AtomicU64,
    frame_counter: AtomicU64,
    active_tracking_frames: AtomicU32,
    shm_reader: Mutex<ObsShmReader>,
    categories: Mutex<CensorCategories>,
    tracker: Mutex<crate::tracker::ZeroMissTracker>,
    boost_mode: AtomicBool,
}

impl CascadeEngine {
    pub fn new() -> Self {
        let vit = Self::load_model("vit_nsfw.onnx", "Falconsai ViT Classifier");
        let nude640 = Self::load_model("640m.onnx", "NudeNet 640m Localizer");

        // UDP socket bound for non-blocking communication with OBS plugin
        let bind_addr = format!("127.0.0.1:{}", OBS_HEARTBEAT_PORT);
        let udp = match UdpSocket::bind(&bind_addr) {
            Ok(sock) => {
                let _ = sock.set_nonblocking(true);
                println!("[CascadeEngine] Bound UDP sync & heartbeat socket on {}", bind_addr);
                Some(sock)
            }
            Err(e) => {
                eprintln!("[CascadeEngine] Warning: Failed to bind UDP on {}: {:?}", bind_addr, e);
                None
            }
        };

        let engine = Self {
            vit_session: Mutex::new(vit),
            nude640_session: Mutex::new(nude640),
            udp_socket: Mutex::new(udp),
            obs_attached: AtomicBool::new(false),
            last_heartbeat: AtomicU64::new(0),
            frame_counter: AtomicU64::new(0),
            active_tracking_frames: AtomicU32::new(0),
            shm_reader: Mutex::new(ObsShmReader::new()),
            categories: Mutex::new(CensorCategories::default()),
            tracker: Mutex::new(crate::tracker::ZeroMissTracker::new(crate::tracker::TrackerConfig::default())),
            boost_mode: AtomicBool::new(false),
        };

        // Automatic instant GPU warm-up: compiles DirectML compute shaders and pre-allocates VRAM
        engine.warmup();

        engine
    }

    /// Performs an instantaneous dummy warm-up on both ViT (Stage 1) and NudeNet 640m (Stage 2).
    /// Forces DirectML/GPU to compile HLSL compute shaders, build descriptor tables,
    /// and pre-allocate VRAM execution heaps so the very first live stream frame has zero lag.
    pub fn warmup(&self) {
        let t_start = Instant::now();
        println!("[CascadeEngine] Initiating neural engine GPU warm-up...");

        // 1. Warm-up Stage 1: ViT (224x224) - 2 iterations to compile DML shaders and prime driver cache
        if let Ok(mut lock) = self.vit_session.lock() {
            if let Some(ref mut session) = *lock {
                let dummy_vit = vec![0.0f32; 1 * 3 * VIT_INPUT_SIZE * VIT_INPUT_SIZE];
                for _ in 0..2 {
                    if let Ok(val) = Value::from_array(([1, 3, VIT_INPUT_SIZE, VIT_INPUT_SIZE], dummy_vit.clone())) {
                        let _ = session.run(ort::inputs![val]);
                    }
                }
                println!("[CascadeEngine] Stage 1 (ViT 224x224) GPU pipeline warmed up successfully.");
            }
        }

        // 2. Warm-up Stage 2: NudeNet 640m (640x640) - 2 iterations to pre-allocate execution heaps
        if let Ok(mut lock) = self.nude640_session.lock() {
            if let Some(ref mut session) = *lock {
                let dummy_nude = vec![0.0f32; 1 * 3 * NUDENET_INPUT_SIZE * NUDENET_INPUT_SIZE];
                for _ in 0..2 {
                    if let Ok(val) = Value::from_array(([1, 3, NUDENET_INPUT_SIZE, NUDENET_INPUT_SIZE], dummy_nude.clone())) {
                        let _ = session.run(ort::inputs![val]);
                    }
                }
                println!("[CascadeEngine] Stage 2 (NudeNet 640x640) GPU pipeline warmed up successfully.");
            }
        }

        // Reset tracker so dummy frames leave no residual tracking state
        self.reset_tracker();

        println!(
            "[CascadeEngine] Neural GPU engine fully warmed up in {:.2}ms. Ready for real-time zero-lag inference.",
            t_start.elapsed().as_secs_f64() * 1000.0
        );
    }

    pub fn set_boost_mode(&self, boosted: bool) {
        self.boost_mode.store(boosted, Ordering::Relaxed);
        println!("[CascadeEngine] Neural Cascade Scan Mode updated: Boosted (Direct Dual-Scan) = {}", boosted);
    }

    pub fn is_boost_mode(&self) -> bool {
        self.boost_mode.load(Ordering::Relaxed)
    }

    pub fn is_tracker_active(&self) -> bool {
        self.tracker.lock().map(|t| t.is_active()).unwrap_or(false)
    }

    pub fn reset_tracker(&self) {
        if let Ok(mut trk) = self.tracker.lock() {
            trk.reset();
        }
        self.active_tracking_frames.store(0, Ordering::Relaxed);
    }

    pub fn get_categories(&self) -> CensorCategories {
        self.categories.lock().map(|c| *c).unwrap_or_default()
    }

    pub fn set_categories(&self, cats: CensorCategories) {
        if let Ok(mut c) = self.categories.lock() {
            *c = cats;
            println!("[CascadeEngine] Live censor categories updated: {:?}", cats);
        }
    }

    pub fn try_read_obs_frame(&self) -> Option<(u32, u32, Vec<u8>)> {
        if let Ok(mut reader) = self.shm_reader.lock() {
            reader.try_read_frame()
        } else {
            None
        }
    }

    fn load_model(filename: &str, title: &str) -> Option<Session> {
        let found = crate::paths::PathResolver::find_model(filename);

        if let Some(ref path) = found {
            // 1. Try DirectML GPU acceleration first for maximum offload to NVIDIA/AMD GPU
            let build_dml = || -> ort::Result<Session> {
                Session::builder()?
                    .with_execution_providers([ort::ep::DirectML::default().build()])?
                    .with_intra_threads(4)?
                    .commit_from_file(path)
            };

            match build_dml() {
                Ok(s) => {
                    println!("[CascadeEngine] {} ACTIVE WITH DIRECTML (GPU ACCELERATED) from: {:?}", title, path);
                    Some(s)
                }
                Err(dml_err) => {
                    println!("[CascadeEngine] DirectML init notice for {} ({:?}), falling back to optimized CPU (4 threads)", title, dml_err);
                    let build_cpu = || -> ort::Result<Session> {
                        Session::builder()?
                            .with_intra_threads(4)?
                            .commit_from_file(path)
                    };
                    match build_cpu() {
                        Ok(s) => {
                            println!("[CascadeEngine] {} active on CPU (4 intra-threads) from: {:?}", title, path);
                            Some(s)
                        }
                        Err(e) => {
                            eprintln!("[CascadeEngine] Failed to create session for {:?}: {:?}", path, e);
                            None
                        }
                    }
                }
            }
        } else {
            eprintln!("[CascadeEngine] {} ({}) not found on disk!", title, filename);
            None
        }
    }

    pub fn is_ready(&self) -> bool {
        let vit_ok = self.vit_session.lock().map(|s| s.is_some()).unwrap_or(false);
        let nude_ok = self.nude640_session.lock().map(|s| s.is_some()).unwrap_or(false);
        vit_ok || nude_ok
    }

    pub fn is_obs_plugin_active(&self) -> bool {
        self.poll_obs_heartbeat();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last = self.last_heartbeat.load(Ordering::Relaxed);
        (now.saturating_sub(last)) < 3500 && self.obs_attached.load(Ordering::Relaxed)
    }

    /// Primary evaluation: Cascaded ViT Screener + NudeNet 640m Precision Localizer
    pub fn classify_frame(
        &self,
        pixels: &[u8],
        width: u32,
        height: u32,
        threshold_pct: u32,
        is_top_down: bool,
    ) -> CascadeResult {
        let t0 = Instant::now();
        let _frame_num = self.frame_counter.fetch_add(1, Ordering::Relaxed);
        let s_norm = (threshold_pct as f32 / 100.0).clamp(0.20, 0.95);
        let categories = self.get_categories();
        let is_boosted = self.is_boost_mode();

        // Sensitivity formulas for both cascade stages:
        // Stage 1 (ViT) sensitivity thresholds:
        // Lower cutoff drastically so ViT doesn't prematurely drop frames with subtle/partial anatomy:
        // High sensitivity (0.90) -> trigger at 0.013
        // Balanced (0.65) -> trigger at 0.0455
        // Low sensitivity (0.30) -> trigger at 0.091
        let vit_trigger_cutoff = (1.0 - s_norm) * 0.13;
        let vit_full_violation_cutoff = 1.0 - (s_norm * 0.45);

        // Stage 2 (NudeNet) sensitivity cutoffs:
        // High sensitivity (0.90) -> exp_cutoff = 0.185, sug_cutoff = 0.335
        // Balanced (0.65) -> exp_cutoff = 0.272, sug_cutoff = 0.422
        // Low sensitivity (0.30) -> exp_cutoff = 0.395, sug_cutoff = 0.545
        let exp_cutoff = (0.50 - s_norm * 0.35).clamp(0.15, 0.45);
        let sug_cutoff = (0.65 - s_norm * 0.35).clamp(0.25, 0.60);

        if pixels.is_empty() || width == 0 || height == 0 {
            return CascadeResult {
                is_violation: false,
                score: 0.005,
                primary_label: "Frame empty".to_string(),
                detailed_reason: "Frame empty".to_string(),
                censor_all: false,
                boxes: Vec::new(),
                stage1_score: 0.005,
                stage1_label: "Neutral".to_string(),
                stage2_score: 0.0,
                latency_ms: 0.0,
                obs_plugin_active: self.is_obs_plugin_active(),
            };
        }

        // Check if frame is pure uniform pitch-black or turned-off screen
        if pixels.len() >= 64 {
            let sample_first = pixels[0];
            let is_pitch_black = sample_first < 15 && pixels[1] < 15 && pixels[2] < 15;
            if is_pitch_black {
                let mut uniform = true;
                for step in 0..16 {
                    let idx = ((step * pixels.len()) / 16) & !3;
                    if idx + 2 < pixels.len() {
                        if pixels[idx] > 20 || pixels[idx + 1] > 20 || pixels[idx + 2] > 20 {
                            uniform = false;
                            break;
                        }
                    }
                }
                if uniform {
                    return CascadeResult {
                        is_violation: false,
                        score: 0.005,
                        primary_label: "Neutral (Black screen)".to_string(),
                        detailed_reason: "Neutral (Black screen)".to_string(),
                        censor_all: false,
                        boxes: Vec::new(),
                        stage1_score: 0.005,
                        stage1_label: "Neutral".to_string(),
                        stage2_score: 0.0,
                        latency_ms: 0.1,
                        obs_plugin_active: self.is_obs_plugin_active(),
                    };
                }
            }
        }

        // Check heartbeat from OBS plugin
        self.poll_obs_heartbeat();

        let tracker_is_active = self.tracker.lock().map(|t| t.is_active()).unwrap_or(false);
        let is_actively_tracking = self.active_tracking_frames.load(Ordering::Relaxed) > 0 || tracker_is_active;

        // Periodic deep scan pulse:
        // In balanced/idle mode (5 FPS), every 4th frame (~800ms) runs Stage 2 unconditionally
        // so a static frame with subtle anatomy or unusual lighting never sits undetected.
        let periodic_deep_pulse = _frame_num % 4 == 0;

        let mut stage1_score = 0.005f32;
        let mut stage1_label = "Neutral".to_string();
        // Trigger Stage 2 unconditionally if Boost Mode is ON (60 FPS Danger Zone),
        // or actively tracking moving targets, or on periodic deep scan pulse:
        let mut stage1_trigger = is_boosted || is_actively_tracking || periodic_deep_pulse;

        // =========================================================================
        // STAGE 1: Vision Transformer Holistic Scene Screener (224x224)
        // =========================================================================
        if let Ok(mut lock) = self.vit_session.lock() {
            if let Some(ref mut session) = *lock {
                let mut ch_r = Vec::with_capacity(VIT_INPUT_SIZE * VIT_INPUT_SIZE);
                let mut ch_g = Vec::with_capacity(VIT_INPUT_SIZE * VIT_INPUT_SIZE);
                let mut ch_b = Vec::with_capacity(VIT_INPUT_SIZE * VIT_INPUT_SIZE);

                for y in 0..VIT_INPUT_SIZE {
                    let sy = if is_top_down {
                        (y * height as usize) / VIT_INPUT_SIZE
                    } else {
                        (height as usize - 1).saturating_sub((y * height as usize) / VIT_INPUT_SIZE)
                    };
                    for x in 0..VIT_INPUT_SIZE {
                        let sx = (x * width as usize) / VIT_INPUT_SIZE;
                        let idx = (sy * width as usize + sx) * 4;
                        if idx + 2 < pixels.len() {
                            let b = (pixels[idx] as f32 / 127.5) - 1.0;
                            let g = (pixels[idx + 1] as f32 / 127.5) - 1.0;
                            let r = (pixels[idx + 2] as f32 / 127.5) - 1.0;
                            ch_r.push(r);
                            ch_g.push(g);
                            ch_b.push(b);
                        } else {
                            ch_r.push(-1.0);
                            ch_g.push(-1.0);
                            ch_b.push(-1.0);
                        }
                    }
                }

                let mut input_data = Vec::with_capacity(1 * 3 * VIT_INPUT_SIZE * VIT_INPUT_SIZE);
                input_data.extend(ch_r);
                input_data.extend(ch_g);
                input_data.extend(ch_b);

                if let Ok(val) = Value::from_array(([1, 3, VIT_INPUT_SIZE, VIT_INPUT_SIZE], input_data)) {
                    if let Ok(outputs) = session.run(ort::inputs![val]) {
                        if let Ok((_shape, logits)) = outputs[0].try_extract_tensor::<f32>() {
                            if logits.len() >= 5 {
                                // Softmax
                                let mut max_l = logits[0];
                                for &l in logits.iter() {
                                    if l > max_l { max_l = l; }
                                }
                                let mut sum_exp = 0.0f32;
                                let mut probs = [0.0f32; 5];
                                for (i, &l) in logits.iter().take(5).enumerate() {
                                    let e = (l - max_l).exp();
                                    probs[i] = e;
                                    sum_exp += e;
                                }
                                if sum_exp > 0.0 {
                                    for p in &mut probs {
                                        *p /= sum_exp;
                                    }
                                }

                                // Classes: 0: drawings, 1: hentai, 2: neutral, 3: porn, 4: sexy
                                let p_drawings = probs[0];
                                let p_hentai = probs[1];
                                let p_neutral = probs[2];
                                let p_porn = probs[3];
                                let p_sexy = probs[4];

                                // Weighted ViT score:
                                // Include subtle drawings/anime NSFW hint if drawings is high and non-neutral
                                let drawings_nsfw_hint = if p_drawings > 0.35 {
                                    (p_hentai * 1.2 + p_sexy * 0.8).min(0.5)
                                } else {
                                    0.0
                                };
                                stage1_score = (p_porn * 1.0 + p_hentai * 0.95 + p_sexy * 0.70 + drawings_nsfw_hint).min(1.0);

                                if p_porn >= 0.40 {
                                    stage1_label = "Pornography".to_string();
                                } else if p_hentai >= 0.40 {
                                    stage1_label = "Hentai / Anime NSFW".to_string();
                                } else if p_sexy >= 0.40 {
                                    stage1_label = "Erotica / Sexy".to_string();
                                } else if stage1_score > 0.15 {
                                    stage1_label = "Suspicious content".to_string();
                                } else {
                                    stage1_label = "Neutral".to_string();
                                }

                                // Stage 1 triggers Stage 2 if:
                                // - Boost mode / tracking / periodic pulse was already true
                                // - Stage 1 score exceeds sensitive threshold (at 65% sens -> ~0.0455)
                                // - Even 1.5% porn, 2.0% hentai, or 3.5% sexy is detected
                                // - Or ViT neutral confidence falls below 95% (meaning non-trivial suspicion)
                                stage1_trigger = stage1_trigger
                                    || stage1_score >= vit_trigger_cutoff
                                    || stage1_score >= 0.20
                                    || p_porn >= 0.015
                                    || p_hentai >= 0.02
                                    || p_sexy >= 0.035
                                    || p_neutral < 0.95;
                            } else if logits.len() == 2 {
                                // 2-Class Model: 0: normal, 1: nsfw
                                let max_l = logits[0].max(logits[1]);
                                let e0 = (logits[0] - max_l).exp();
                                let e1 = (logits[1] - max_l).exp();
                                let sum_e = e0 + e1;
                                let p_normal = if sum_e > 0.0 { e0 / sum_e } else { 0.5 };
                                let p_nsfw = if sum_e > 0.0 { e1 / sum_e } else { 0.5 };

                                stage1_score = p_nsfw;
                                if p_nsfw >= 0.40 {
                                    stage1_label = "NSFW content".to_string();
                                } else if stage1_score > 0.15 {
                                    stage1_label = "Suspicious content".to_string();
                                } else {
                                    stage1_label = "Neutral".to_string();
                                }

                                stage1_trigger = stage1_trigger
                                    || stage1_score >= vit_trigger_cutoff
                                    || stage1_score >= 0.15
                                    || p_nsfw >= 0.03
                                    || p_normal < 0.95;
                            }
                        }
                    }
                }
            }
        }

        // =========================================================================
        // STAGE 2: NudeNet 640m High-Resolution Anatomical Localizer (640x640)
        // =========================================================================
        let mut raw_detections = Vec::new();
        let mut stage2_score = 0.0f32;
        let mut top_anatomical_label = String::new();
        let mut top_anatomical_score = 0.0f32;

        if stage1_trigger || stage1_score >= vit_full_violation_cutoff {
            if let Ok(mut lock) = self.nude640_session.lock() {
                if let Some(ref mut session) = *lock {
                    let mut ch_r = Vec::with_capacity(NUDENET_INPUT_SIZE * NUDENET_INPUT_SIZE);
                    let mut ch_g = Vec::with_capacity(NUDENET_INPUT_SIZE * NUDENET_INPUT_SIZE);
                    let mut ch_b = Vec::with_capacity(NUDENET_INPUT_SIZE * NUDENET_INPUT_SIZE);

                    let (scaled_w, scaled_h, pad_x, pad_y) = if width > 0 && height > 0 {
                        let r = (NUDENET_INPUT_SIZE as f32 / width as f32)
                            .min(NUDENET_INPUT_SIZE as f32 / height as f32);
                        let s_w = ((width as f32 * r).round() as usize).clamp(1, NUDENET_INPUT_SIZE);
                        let s_h = ((height as f32 * r).round() as usize).clamp(1, NUDENET_INPUT_SIZE);
                        let p_x = (NUDENET_INPUT_SIZE - s_w) / 2;
                        let p_y = (NUDENET_INPUT_SIZE - s_h) / 2;
                        (s_w, s_h, p_x, p_y)
                    } else {
                        (NUDENET_INPUT_SIZE, NUDENET_INPUT_SIZE, 0, 0)
                    };

                    let pad_val = 114.0 / 255.0;

                    for y in 0..NUDENET_INPUT_SIZE {
                        let in_y = y >= pad_y && y < pad_y + scaled_h;
                        let sy = if in_y {
                            let norm_y = y - pad_y;
                            if is_top_down {
                                ((norm_y * height as usize) / scaled_h).min(height as usize - 1)
                            } else {
                                (height as usize - 1).saturating_sub((norm_y * height as usize) / scaled_h)
                            }
                        } else {
                            0
                        };

                        for x in 0..NUDENET_INPUT_SIZE {
                            let in_x = x >= pad_x && x < pad_x + scaled_w;
                            if in_y && in_x {
                                let norm_x = x - pad_x;
                                let sx = ((norm_x * width as usize) / scaled_w).min(width as usize - 1);
                                let idx = (sy * width as usize + sx) * 4;
                                if idx + 2 < pixels.len() {
                                    ch_b.push(pixels[idx] as f32 / 255.0);
                                    ch_g.push(pixels[idx + 1] as f32 / 255.0);
                                    ch_r.push(pixels[idx + 2] as f32 / 255.0);
                                } else {
                                    ch_b.push(pad_val);
                                    ch_g.push(pad_val);
                                    ch_r.push(pad_val);
                                }
                            } else {
                                ch_b.push(pad_val);
                                ch_g.push(pad_val);
                                ch_r.push(pad_val);
                            }
                        }
                    }

                    let mut input_data = Vec::with_capacity(1 * 3 * NUDENET_INPUT_SIZE * NUDENET_INPUT_SIZE);
                    input_data.extend(ch_r);
                    input_data.extend(ch_g);
                    input_data.extend(ch_b);

                    if let Ok(val) = Value::from_array(([1, 3, NUDENET_INPUT_SIZE, NUDENET_INPUT_SIZE], input_data)) {
                        if let Ok(outputs) = session.run(ort::inputs![val]) {
                            if let Ok((_shape, data)) = outputs[0].try_extract_tensor::<f32>() {
                                let mut candidates = Vec::new();
                                let min_conf = 0.04f32;

                                for p in 0..8400 {
                                    let cx = data[0 * 8400 + p];
                                    let cy = data[1 * 8400 + p];
                                    let w = data[2 * 8400 + p];
                                    let h = data[3 * 8400 + p];
                                    let x1 = (cx - w / 2.0).clamp(0.0, 640.0);
                                    let y1 = (cy - h / 2.0).clamp(0.0, 640.0);
                                    let x2 = (cx + w / 2.0).clamp(0.0, 640.0);
                                    let y2 = (cy + h / 2.0).clamp(0.0, 640.0);

                                    for c in 0..18 {
                                        let s = data[(4 + c) * 8400 + p];
                                        if s > stage2_score {
                                            stage2_score = s;
                                        }
                                        if s >= min_conf {
                                            candidates.push(RawBox {
                                                class_id: c,
                                                score: s,
                                                box_coords: (x1, y1, x2, y2),
                                            });
                                        }
                                    }
                                }

                                let nms_results = run_nudenet_nms(candidates, 0.45);

                                for d in nms_results {
                                    let is_exp = is_explicit_nudenet_class(d.class_id);
                                    let is_sug = is_suggestive_nudenet_class(d.class_id);

                                    // Filter by user's active categories
                                    if !categories.is_class_allowed(d.class_id) {
                                        continue;
                                    }

                                    if is_exp || is_sug {
                                        // Per-class threshold calibration:
                                        // High-risk genitalia & anus have extra sensitive trigger floor
                                        let class_cutoff = match d.class_id {
                                            4 | 14 | 6 => (exp_cutoff * 0.75).clamp(0.12, 0.30),
                                            2 | 3 => exp_cutoff,
                                            _ => sug_cutoff,
                                        };

                                        if d.score >= class_cutoff {
                                            let lbl = nudenet_class_label(d.class_id).to_string();
                                            if d.score > top_anatomical_score {
                                                top_anatomical_score = d.score;
                                                top_anatomical_label = lbl.clone();
                                            }

                                            // Map from 640x640 letterbox coordinates back to normalized screen space [0..1]
                                            let u1 = ((d.box_coords.0 - pad_x as f32) / scaled_w as f32).clamp(0.0, 1.0);
                                            let v1 = ((d.box_coords.1 - pad_y as f32) / scaled_h as f32).clamp(0.0, 1.0);
                                            let u2 = ((d.box_coords.2 - pad_x as f32) / scaled_w as f32).clamp(0.0, 1.0);
                                            let v2 = ((d.box_coords.3 - pad_y as f32) / scaled_h as f32).clamp(0.0, 1.0);

                                            let (final_u1, final_u2) = (u1.min(u2), u1.max(u2));
                                            let (final_v1, final_v2) = if is_top_down {
                                                (v1.min(v2), v1.max(v2))
                                            } else {
                                                (1.0 - v2.max(v1), 1.0 - v2.min(v1))
                                            };

                                            if final_u2 > final_u1 && final_v2 > final_v1 {
                                                raw_detections.push(crate::tracker::Detection {
                                                    x1: final_u1,
                                                    y1: final_v1,
                                                    x2: final_u2,
                                                    y2: final_v2,
                                                    score: d.score,
                                                    class_id: d.class_id,
                                                    label: lbl,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Feed detections into Zero-Miss Tracker (Kalman/Inertia velocity continuation, +18% Dilation, 30-frame Hold Retention)
        let mut boxes = Vec::new();
        if let Ok(mut trk) = self.tracker.lock() {
            trk.update(&raw_detections);
            boxes = trk.get_dilated_boxes();
            if trk.is_active() {
                self.active_tracking_frames.store(10, Ordering::Relaxed);
            } else {
                let cur = self.active_tracking_frames.load(Ordering::Relaxed);
                if cur > 0 {
                    self.active_tracking_frames.store(cur - 1, Ordering::Relaxed);
                }
            }
        }

        // If top_anatomical_label is empty but tracker has retained/dilated boxes:
        if top_anatomical_label.is_empty() && !boxes.is_empty() {
            for b in &boxes {
                if b.score > top_anatomical_score {
                    top_anatomical_score = b.score;
                    top_anatomical_label = b.label.clone();
                }
            }
        }

        // =========================================================================
        // STAGE 3: Decision Fusion & Violation Arbitration
        // =========================================================================
        let has_boxes = !boxes.is_empty();
        let vit_violation = stage1_score >= vit_full_violation_cutoff;

        let is_violation = has_boxes || vit_violation;
        let final_score = if top_anatomical_score > stage1_score { top_anatomical_score } else { stage1_score };

        let primary_label = if has_boxes {
            top_anatomical_label.clone()
        } else if vit_violation || stage1_score > 0.15 {
            stage1_label.clone()
        } else {
            "Neutral".to_string()
        };

        let detailed_reason = if has_boxes {
            format!("CASCADE: Localized anatomical markers ({})", primary_label)
        } else if vit_violation {
            format!("CASCADE: Scene content qualified as ({})", primary_label)
        } else {
            primary_label.clone()
        };

        let censor_all = vit_violation && boxes.is_empty();
        let obs_active = self.is_obs_plugin_active();
        let latency = t0.elapsed().as_secs_f32() * 1000.0;

        let result = CascadeResult {
            is_violation,
            score: final_score,
            primary_label,
            detailed_reason,
            censor_all,
            boxes,
            stage1_score,
            stage1_label,
            stage2_score,
            latency_ms: latency,
            obs_plugin_active: obs_active,
        };

        // Broadcast directly to OBS Studio native plugin over UDP
        self.send_to_obs_plugin(&result);

        result
    }

    /// Transmit bounding boxes to OBS Studio Plugin via high-speed UDP
    fn send_to_obs_plugin(&self, result: &CascadeResult) {
        let sock_guard = match self.udp_socket.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let sock = match sock_guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let packet = ObsPacket {
            censor_all: result.censor_all,
            score: result.score,
            reason: &result.detailed_reason,
            boxes: &result.boxes,
            ts: now_ts,
        };

        if let Ok(json_bytes) = serde_json::to_vec(&packet) {
            let target_addr = format!("127.0.0.1:{}", OBS_PLUGIN_PORT);
            let _ = sock.send_to(&json_bytes, target_addr);
        }
    }

    /// Poll heartbeat replies from OBS Studio filter
    pub fn poll_obs_heartbeat(&self) {
        let sock_guard = match self.udp_socket.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let sock = match sock_guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        let mut buf = [0u8; 512];
        while let Ok((bytes, _src)) = sock.recv_from(&mut buf) {
            if let Ok(text) = std::str::from_utf8(&buf[..bytes]) {
                if text.contains("obs_attached") || text.contains("obs_filter_heartbeat") {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let was_attached = self.obs_attached.swap(true, Ordering::Relaxed);
                    if !was_attached {
                        println!("[CascadeEngine] OBS filter plugin heartbeat received! Filter is active.");
                    }
                    self.last_heartbeat.store(now, Ordering::Relaxed);
                }
            }
        }
    }

    /// Force OBS Plugin to blur full video source via GPU (censor_all = true)
    pub fn trigger_obs_full_censor(&self, reason: &str) {
        let sock_guard = match self.udp_socket.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let sock = match sock_guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let empty_boxes: Vec<NormalizedBox> = Vec::new();
        let packet = ObsPacket {
            censor_all: true,
            score: 0.99,
            reason,
            boxes: &empty_boxes,
            ts: now_ts,
        };

        if let Ok(json_bytes) = serde_json::to_vec(&packet) {
            let target_addr = format!("127.0.0.1:{}", OBS_PLUGIN_PORT);
            let _ = sock.send_to(&json_bytes, target_addr);
        }
    }

    /// Clear OBS Plugin censorship (sends clean frame packet with 0 boxes)
    pub fn clear_obs_plugin_censor(&self) {
        let sock_guard = match self.udp_socket.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let sock = match sock_guard.as_ref() {
            Some(s) => s,
            None => return,
        };

        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let empty_boxes: Vec<NormalizedBox> = Vec::new();
        let packet = ObsPacket {
            censor_all: false,
            score: 0.0,
            reason: "CLEAN",
            boxes: &empty_boxes,
            ts: now_ts,
        };

        if let Ok(json_bytes) = serde_json::to_vec(&packet) {
            let target_addr = format!("127.0.0.1:{}", OBS_PLUGIN_PORT);
            let _ = sock.send_to(&json_bytes, target_addr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_censor_categories_mapping() {
        let def_cats = CensorCategories::default();
        assert!(def_cats.genitalia);
        assert!(def_cats.breasts);
        assert!(def_cats.buttocks);
        assert!(!def_cats.underwear);
        assert!(!def_cats.body_exposed);

        // Class 4 (FEMALE_GENITALIA_EXPOSED) & 14 (MALE_GENITALIA_EXPOSED)
        assert!(def_cats.is_class_allowed(4));
        assert!(def_cats.is_class_allowed(14));
        // Class 3 (FEMALE_BREAST_EXPOSED)
        assert!(def_cats.is_class_allowed(3));
        // Class 2 (BUTTOCKS_EXPOSED) & 6 (ANUS_EXPOSED)
        assert!(def_cats.is_class_allowed(2));
        assert!(def_cats.is_class_allowed(6));
        // Underwear & body classes disabled by default
        assert!(!def_cats.is_class_allowed(16));
        assert!(!def_cats.is_class_allowed(17));
        assert!(!def_cats.is_class_allowed(15));
        assert!(!def_cats.is_class_allowed(0));
        assert!(!def_cats.is_class_allowed(5));

        let custom_cats = CensorCategories {
            genitalia: true,
            breasts: false,
            buttocks: false,
            underwear: true,
            body_exposed: false,
        };
        assert!(custom_cats.is_class_allowed(4));
        assert!(!custom_cats.is_class_allowed(3));
        assert!(!custom_cats.is_class_allowed(2));
        assert!(custom_cats.is_class_allowed(16));
        assert!(custom_cats.is_class_allowed(15));
    }
}
