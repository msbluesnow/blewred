use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use crate::lexical::LexicalEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenAnalysisResult {
    pub should_censor: bool,
    pub is_nsfw: bool,
    pub nsfw_score: f32,
    pub nsfw_label: String,
    pub nsfw_category: String,
    pub has_banned_text: bool,
    pub banned_matches: Vec<String>,
    pub gpu_info: String,
    pub trigger_circumstances: String,
    pub status_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub index: i32,
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecificTestResult {
    pub test_type: String,     // "ocr" or "nsfw"
    pub activated: bool,       // true = screen covered, false = stream remains open
    pub score: f32,            // probability or detection confidence
    pub matched_rule: String,  // rule category or detection locus
    pub matched_item: String,  // specific word or visual class
    pub circumstances: String, // human-readable explanation of why it activated / didn't activate
    pub gpu_info: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveFrameData {
    pub monitor_index: i32,
    pub monitor_name: String,
    pub nsfw_category: String,
    pub nsfw_score: f32,
    pub nsfw_label: String,
    pub nsfw_violation: bool,
    pub ocr_snippet: String,
    pub ocr_violation: bool,
    pub banned_words: Vec<String>,
    pub should_censor: bool,
    pub reason: String,
    pub timestamp: String,
    pub stage1_score: f32,
    pub stage1_label: String,
    pub stage2_score: f32,
    pub obs_plugin_active: bool,
    pub box_count: usize,
    pub censor_all: bool,
}

#[cfg(target_os = "windows")]
#[allow(non_snake_case)]
mod gdi {
    pub type MonitorEnumProc = unsafe extern "system" fn(
        hMonitor: isize,
        hdcMonitor: isize,
        lprcMonitor: *const RECT,
        dwData: isize,
    ) -> i32;

    #[repr(C)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    pub struct MONITORINFOEXW {
        pub cbSize: u32,
        pub rcMonitor: RECT,
        pub rcWork: RECT,
        pub dwFlags: u32,
        pub szDevice: [u16; 32],
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetDC(hWnd: isize) -> isize;
        pub fn ReleaseDC(hWnd: isize, hDC: isize) -> i32;
        pub fn GetSystemMetrics(nIndex: i32) -> i32;
        pub fn OpenClipboard(hWndNewOwner: isize) -> i32;
        pub fn CloseClipboard() -> i32;
        pub fn GetClipboardData(uFormat: u32) -> isize;
        pub fn GetMonitorInfoW(hMonitor: isize, lpmi: *mut MONITORINFOEXW) -> i32;
        pub fn EnumDisplayMonitors(
            hdc: isize,
            lprcClip: *const RECT,
            lpfnEnum: MonitorEnumProc,
            dwData: isize,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GlobalLock(hMem: isize) -> *mut u16;
        pub fn GlobalUnlock(hMem: isize) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        pub fn CreateCompatibleDC(hdc: isize) -> isize;
        pub fn SelectObject(hdc: isize, hgdiobj: isize) -> isize;
        pub fn BitBlt(
            hdcDest: isize,
            nXDest: i32,
            nYDest: i32,
            nWidth: i32,
            nHeight: i32,
            hdcSrc: isize,
            nXSrc: i32,
            nYSrc: i32,
            dwRop: u32,
        ) -> i32;
        pub fn DeleteDC(hdc: isize) -> i32;
        pub fn DeleteObject(ho: isize) -> i32;
        pub fn CreateDIBSection(
            hdc: isize,
            pbmi: *const BITMAPINFO,
            usage: u32,
            ppvBits: *mut *mut u8,
            hSection: isize,
            offset: u32,
        ) -> isize;
    }

    #[repr(C)]
    pub struct BITMAPINFOHEADER {
        pub biSize: u32,
        pub biWidth: i32,
        pub biHeight: i32,
        pub biPlanes: u16,
        pub biBitCount: u16,
        pub biCompression: u32,
        pub biSizeImage: u32,
        pub biXPelsPerMeter: i32,
        pub biYPelsPerMeter: i32,
        pub biClrUsed: u32,
        pub biClrImportant: u32,
    }

    #[repr(C)]
    pub struct RGBQUAD {
        pub rgbBlue: u8,
        pub rgbGreen: u8,
        pub rgbRed: u8,
        pub rgbReserved: u8,
    }

    #[repr(C)]
    pub struct BITMAPINFO {
        pub bmiHeader: BITMAPINFOHEADER,
        pub bmiColors: [RGBQUAD; 1],
    }
}

pub fn is_explicit_nudenet_class(class_id: usize) -> bool {
    // 2: BUTTOCKS_EXPOSED, 3: FEMALE_BREAST_EXPOSED, 4: FEMALE_GENITALIA_EXPOSED, 6: ANUS_EXPOSED, 14: MALE_GENITALIA_EXPOSED
    matches!(class_id, 2 | 3 | 4 | 6 | 14)
}

pub fn is_suggestive_nudenet_class(class_id: usize) -> bool {
    // 0: FEMALE_GENITALIA_COVERED, 5: MALE_BREAST_EXPOSED, 15: ANUS_COVERED, 16: FEMALE_BREAST_COVERED, 17: BUTTOCKS_COVERED
    matches!(class_id, 0 | 5 | 15 | 16 | 17)
}

pub fn nudenet_class_name(class_id: usize) -> &'static str {
    match class_id {
        0 => "FEMALE_GENITALIA_COVERED",
        1 => "FACE_FEMALE",
        2 => "BUTTOCKS_EXPOSED",
        3 => "FEMALE_BREAST_EXPOSED",
        4 => "FEMALE_GENITALIA_EXPOSED",
        5 => "MALE_BREAST_EXPOSED",
        6 => "ANUS_EXPOSED",
        7 => "FEET_EXPOSED",
        8 => "BELLY_COVERED",
        9 => "FEET_COVERED",
        10 => "ARMPITS_COVERED",
        11 => "ARMPITS_EXPOSED",
        12 => "FACE_MALE",
        13 => "BELLY_EXPOSED",
        14 => "MALE_GENITALIA_EXPOSED",
        15 => "ANUS_COVERED",
        16 => "FEMALE_BREAST_COVERED",
        17 => "BUTTOCKS_COVERED",
        _ => "UNKNOWN",
    }
}

pub fn nudenet_class_label(class_id: usize) -> &'static str {
    match class_id {
        0 => "Female Genitalia (Covered)",
        1 => "Female Face",
        2 => "Buttocks (Exposed)",
        3 => "Female Breast (Exposed)",
        4 => "Female Genitalia (Exposed)",
        5 => "Male Breast",
        6 => "Anus / Anorectal Region",
        7 => "Feet (Exposed)",
        8 => "Belly (Covered)",
        9 => "Feet (Covered)",
        10 => "Armpits (Covered)",
        11 => "Armpits (Exposed)",
        12 => "Male Face",
        13 => "Belly (Exposed)",
        14 => "Male Genitalia (Exposed)",
        15 => "Anus (Covered)",
        16 => "Female Breast (Covered)",
        17 => "Buttocks (Covered)",
        _ => "Anatomical Region",
    }
}

#[derive(Debug, Clone)]
pub struct RawBox {
    pub class_id: usize,
    pub score: f32,
    pub box_coords: (f32, f32, f32, f32),
}

/// Class-aware Non-Maximum Suppression (NMS)
/// Prevents non-censored overlapping classes (faces, belly, feet) from erasing critical censored anatomy.
pub fn run_nudenet_nms(mut dets: Vec<RawBox>, iou_thresh: f32) -> Vec<RawBox> {
    dets.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut keep = Vec::new();
    let mut suppressed = vec![false; dets.len()];

    for i in 0..dets.len() {
        if suppressed[i] { continue; }
        keep.push(dets[i].clone());
        let a = &dets[i];
        let area_a = (a.box_coords.2 - a.box_coords.0).max(0.0) * (a.box_coords.3 - a.box_coords.1).max(0.0);

        for j in (i + 1)..dets.len() {
            if suppressed[j] { continue; }
            let b = &dets[j];
            // Class-aware constraint: only suppress boxes of the identical class
            if a.class_id != b.class_id {
                continue;
            }
            let xx1 = a.box_coords.0.max(b.box_coords.0);
            let yy1 = a.box_coords.1.max(b.box_coords.1);
            let xx2 = a.box_coords.2.min(b.box_coords.2);
            let yy2 = a.box_coords.3.min(b.box_coords.3);
            let inter = (xx2 - xx1).max(0.0) * (yy2 - yy1).max(0.0);
            let area_b = (b.box_coords.2 - b.box_coords.0).max(0.0) * (b.box_coords.3 - b.box_coords.1).max(0.0);
            let union_area = area_a + area_b - inter;
            if union_area > 0.0 && (inter / union_area) > iou_thresh {
                suppressed[j] = true;
            }
        }
    }
    keep
}

pub struct VisionEngine {
    pub lexical: Arc<Mutex<LexicalEngine>>,
    pub is_ocr_enabled: AtomicBool,
    pub nsfw_threshold: AtomicU32,
    pub cascade: Arc<crate::cascade::CascadeEngine>,
    pub selected_monitor: Arc<tokio::sync::Mutex<i32>>,
    pub cached_ocr_result: Arc<std::sync::Mutex<(bool, Vec<String>, String)>>,
    pub last_ocr_time: std::sync::atomic::AtomicU64,
    pub directml_available: AtomicBool,
    pub cuda_available: AtomicBool,
    pub gpu_name: String,
}

impl VisionEngine {
    pub fn new(lexical: Arc<Mutex<LexicalEngine>>) -> Self {
        let (gpu_name, is_gpu) = Self::detect_gpu_device();

        println!("[VisionEngine] Initializing with GPU: {}", gpu_name);

        let cascade = Arc::new(crate::cascade::CascadeEngine::new());

        Self {
            lexical,
            is_ocr_enabled: AtomicBool::new(true),
            nsfw_threshold: AtomicU32::new(70),
            cascade,
            selected_monitor: Arc::new(tokio::sync::Mutex::new(0)),
            cached_ocr_result: Arc::new(std::sync::Mutex::new((false, Vec::new(), String::new()))),
            last_ocr_time: std::sync::atomic::AtomicU64::new(0),
            directml_available: AtomicBool::new(is_gpu),
            cuda_available: AtomicBool::new(is_gpu),
            gpu_name,
        }
    }

    pub fn set_ocr_enabled(&self, enabled: bool) {
        self.is_ocr_enabled.store(enabled, Ordering::SeqCst);
        println!("[VisionEngine] OCR set to: {}", enabled);
    }

    pub fn is_ocr_enabled(&self) -> bool {
        self.is_ocr_enabled.load(Ordering::Relaxed)
    }

    pub fn is_model_ready(&self) -> bool {
        self.cascade.is_ready()
    }

    pub fn is_russian_ocr_installed() -> bool {
        #[cfg(target_os = "windows")]
        {
            let windir = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
            let win_path = std::path::Path::new(&windir);
            let path = win_path.join("System32").join("ru-RU").join("msocr.dll");
            let path_win11 = win_path.join("OCR").join("ru-RU").join("MsOcr.dll");
            path.exists() || path_win11.exists()
        }
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    pub fn is_english_ocr_installed() -> bool {
        #[cfg(target_os = "windows")]
        {
            let windir = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
            let win_path = std::path::Path::new(&windir);
            let path = win_path.join("System32").join("en-US").join("msocr.dll");
            let path_win11 = win_path.join("OCR").join("en-US").join("MsOcr.dll");
            path.exists() || path_win11.exists()
        }
        #[cfg(not(target_os = "windows"))]
        {
            true
        }
    }

    pub fn get_nsfw_threshold(&self) -> u32 {
        self.nsfw_threshold.load(Ordering::Relaxed)
    }

    pub fn set_nsfw_threshold(&self, val: u32) {
        let clamped = val.clamp(1, 100);
        self.nsfw_threshold.store(clamped, Ordering::SeqCst);
        println!("[VisionEngine] NSFW threshold updated to: {}%", clamped);
    }

    pub fn get_censor_categories(&self) -> crate::cascade::CensorCategories {
        self.cascade.get_categories()
    }

    pub fn set_censor_categories(&self, cats: crate::cascade::CensorCategories) {
        self.cascade.set_categories(cats);
    }

    pub fn clear_obs_censor(&self) {
        self.cascade.clear_obs_plugin_censor();
    }

    pub fn get_selected_monitor(&self) -> i32 {
        if let Ok(guard) = self.selected_monitor.try_lock() {
            *guard
        } else {
            0
        }
    }

    pub fn set_selected_monitor(&self, index: i32) {
        let valid_idx = index.max(0);
        if let Ok(mut guard) = self.selected_monitor.try_lock() {
            *guard = valid_idx;
            println!("[VisionEngine] Selected monitor changed to physical monitor index: {}", valid_idx);
        }
        // Invalidate OCR cache and reset tracker state so switching monitors is immediate and clean
        if let Ok(mut cache) = self.cached_ocr_result.lock() {
            *cache = (false, Vec::new(), String::new());
        }
        self.last_ocr_time.store(0, std::sync::atomic::Ordering::Relaxed);
        self.cascade.clear_obs_plugin_censor();
    }

    pub fn get_selected_monitor_name(&self) -> String {
        let idx = self.get_selected_monitor();
        let monitors = Self::enumerate_monitors();
        monitors.iter().find(|m| m.index == idx)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| format!("Monitor {}", idx + 1))
    }

    pub fn get_current_timestamp() -> String {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let secs = now.as_secs();
        let millis = now.subsec_millis();
        let h = (secs / 3600) % 24;
        let m = (secs / 60) % 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
    }

    pub fn get_current_timestamp_hms() -> String {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let secs = now.as_secs();
        let h = (secs / 3600) % 24;
        let m = (secs / 60) % 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", h, m, s)
    }

    pub fn enumerate_monitors() -> Vec<MonitorInfo> {
        #[cfg(target_os = "windows")]
        {
            unsafe extern "system" fn monitor_enum_proc(
                h_monitor: isize,
                _hdc: isize,
                _lprc: *const gdi::RECT,
                dw_data: isize,
            ) -> i32 {
                let list = &mut *(dw_data as *mut Vec<MonitorInfo>);
                let mut mi: gdi::MONITORINFOEXW = std::mem::zeroed();
                mi.cbSize = std::mem::size_of::<gdi::MONITORINFOEXW>() as u32;

                if gdi::GetMonitorInfoW(h_monitor, &mut mi) != 0 {
                    let is_primary = (mi.dwFlags & 1) != 0;
                    let width = mi.rcMonitor.right - mi.rcMonitor.left;
                    let height = mi.rcMonitor.bottom - mi.rcMonitor.top;
                    let dev_name = String::from_utf16_lossy(&mi.szDevice)
                        .trim_matches(char::from(0))
                        .to_string();

                    let index = list.len() as i32;
                    let friendly_name = if is_primary {
                        format!("Monitor {} (Primary) — {}×{}", index + 1, width, height)
                    } else {
                        format!("Monitor {} — {}×{}", index + 1, width, height)
                    };

                    list.push(MonitorInfo {
                        index,
                        id: dev_name,
                        name: friendly_name,
                        x: mi.rcMonitor.left,
                        y: mi.rcMonitor.top,
                        width,
                        height,
                        is_primary,
                    });
                }
                1
            }

            let mut monitors = Vec::new();
            unsafe {
                gdi::EnumDisplayMonitors(
                    0,
                    std::ptr::null(),
                    monitor_enum_proc,
                    &mut monitors as *mut Vec<MonitorInfo> as isize,
                );
            }
            monitors
        }
        #[cfg(not(target_os = "windows"))]
        {
            vec![MonitorInfo {
                index: 0,
                id: "default".to_string(),
                name: "Primary Monitor".to_string(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                is_primary: true,
            }]
        }
    }

    fn detect_gpu_device() -> (String, bool) {
        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name -First 1"])
                .output()
            {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !name.is_empty() {
                    return (format!("{} [DirectML]", name), true);
                }
            }
        }
        ("DirectML / DirectX 12 GPU".to_string(), true)
    }

    pub fn capture_screen(monitor_idx: Option<i32>) -> Option<(u32, u32, Vec<u8>)> {
        #[cfg(target_os = "windows")]
        unsafe {
            let hdc_screen = gdi::GetDC(0);
            if hdc_screen == 0 {
                return None;
            }

            let (src_x, src_y, width, height) = match monitor_idx {
                None => {
                    (0, 0, gdi::GetSystemMetrics(0), gdi::GetSystemMetrics(1))
                }
                Some(idx) => {
                    if idx == -1 {
                        let x = gdi::GetSystemMetrics(76);
                        let y = gdi::GetSystemMetrics(77);
                        let w = gdi::GetSystemMetrics(78);
                        let h = gdi::GetSystemMetrics(79);
                        (x, y, w, h)
                    } else {
                        let monitors = Self::enumerate_monitors();
                        if let Some(m) = monitors.iter().find(|m| m.index == idx) {
                            (m.x, m.y, m.width, m.height)
                        } else {
                            (0, 0, gdi::GetSystemMetrics(0), gdi::GetSystemMetrics(1))
                        }
                    }
                }
            };

            if width <= 0 || height <= 0 {
                gdi::ReleaseDC(0, hdc_screen);
                return None;
            }

            let hdc_mem = gdi::CreateCompatibleDC(hdc_screen);
            if hdc_mem == 0 {
                gdi::ReleaseDC(0, hdc_screen);
                return None;
            }

            let mut bmi: gdi::BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<gdi::BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width;
            bmi.bmiHeader.biHeight = height; // Positive = bottom-up BMP
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = 0; // BI_RGB

            let mut bits_ptr: *mut u8 = std::ptr::null_mut();
            let hbmp = gdi::CreateDIBSection(
                hdc_mem,
                &bmi,
                0,
                &mut bits_ptr,
                0,
                0,
            );

            if hbmp == 0 || bits_ptr.is_null() {
                gdi::DeleteDC(hdc_mem);
                gdi::ReleaseDC(0, hdc_screen);
                return None;
            }

            let old_bmp = gdi::SelectObject(hdc_mem, hbmp);

            let rop_srccopy: u32 = 0x00CC0020;
            let capture_success = gdi::BitBlt(
                hdc_mem,
                0,
                0,
                width,
                height,
                hdc_screen,
                src_x,
                src_y,
                rop_srccopy | 0x40000000,
            );

            if capture_success == 0 {
                gdi::SelectObject(hdc_mem, old_bmp);
                gdi::DeleteObject(hbmp);
                gdi::DeleteDC(hdc_mem);
                gdi::ReleaseDC(0, hdc_screen);
                return None;
            }

            let total_bytes = (width * height * 4) as usize;
            let bottom_up_slice = std::slice::from_raw_parts(bits_ptr, total_bytes);

            let row_stride = (width * 4) as usize;
            let mut top_down_bgra = vec![0u8; total_bytes];

            for y in 0..height as usize {
                let src_row = height as usize - 1 - y;
                let src_start = src_row * row_stride;
                let dst_start = y * row_stride;
                top_down_bgra[dst_start..dst_start + row_stride]
                    .copy_from_slice(&bottom_up_slice[src_start..src_start + row_stride]);
            }

            gdi::SelectObject(hdc_mem, old_bmp);
            gdi::DeleteObject(hbmp);
            gdi::DeleteDC(hdc_mem);
            gdi::ReleaseDC(0, hdc_screen);

            Some((width as u32, height as u32, top_down_bgra))
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }

    pub fn classify_nsfw(&self, width: u32, height: u32, pixels: &[u8]) -> (bool, f32, String, String) {
        let cascade_res = self.cascade.classify_frame(pixels, width, height, self.get_nsfw_threshold(), true);
        let is_nsfw = cascade_res.is_violation;
        let nsfw_score = cascade_res.score;
        let label = cascade_res.primary_label.clone();
        let category = if is_nsfw { "Pornography".to_string() } else { "Neutral".to_string() };

        (is_nsfw, nsfw_score, label, category)
    }

    pub fn recognize_text_from_bgra(width: u32, height: u32, pixels: &[u8]) -> String {
        crate::ocr::get_ocr_engine().recognize_text(width, height, pixels)
    }

    pub fn get_clipboard_text() -> Option<String> {
        #[cfg(target_os = "windows")]
        unsafe {
            const CF_UNICODETEXT: u32 = 13;
            if gdi::OpenClipboard(0) != 0 {
                let hmem = gdi::GetClipboardData(CF_UNICODETEXT);
                if hmem != 0 {
                    let ptr = gdi::GlobalLock(hmem);
                    if !ptr.is_null() {
                        let mut len = 0;
                        while *ptr.add(len) != 0 && len < 4096 {
                            len += 1;
                        }
                        let slice = std::slice::from_raw_parts(ptr, len);
                        let text = String::from_utf16_lossy(slice);
                        gdi::GlobalUnlock(hmem);
                        gdi::CloseClipboard();
                        return Some(text);
                    }
                    gdi::GlobalUnlock(hmem);
                }
                gdi::CloseClipboard();
            }
        }
        None
    }

    pub fn check_screen_for_stopwords(&self, monitor_idx: Option<i32>) -> (bool, Vec<String>, String) {
        let m_idx = monitor_idx.unwrap_or_else(|| self.get_selected_monitor());
        let mut combined_text = String::new();

        if let Some((w, h, px)) = Self::capture_screen(Some(m_idx)) {
            let ocr_text = Self::recognize_text_from_bgra(w, h, &px);
            combined_text.push_str(&ocr_text);
        }

        if let Some(clip_text) = Self::get_clipboard_text() {
            if !combined_text.is_empty() {
                combined_text.push(' ');
            }
            combined_text.push_str(&clip_text);
        }

        let (has_banned, words) = if let Ok(engine) = self.lexical.try_lock() {
            let matches = engine.check_text(&combined_text);
            (!matches.is_empty(), matches.into_iter().map(|m| m.word).collect())
        } else {
            (false, Vec::new())
        };

        (has_banned, words, combined_text)
    }

    pub fn check_screen_pixels_for_stopwords(&self, width: u32, height: u32, pixels: &[u8]) -> (bool, Vec<String>, String) {
        let ocr_text = Self::recognize_text_from_bgra(width, height, pixels);
        let (has_banned, words) = if let Ok(engine) = self.lexical.try_lock() {
            let matches = engine.check_text(&ocr_text);
            (!matches.is_empty(), matches.into_iter().map(|m| m.word).collect())
        } else {
            (false, Vec::new())
        };

        (has_banned, words, ocr_text)
    }

    /// Fast Real-time Stream Analysis of the selected monitor:
    /// Runs two-stage neural cascade classifier (ViT + NudeNet 640m) and Windows OCR directly on the captured frame.
    pub fn analyze_frame_realtime(&self, monitor_idx: i32) -> Option<LiveFrameData> {
        let valid_idx = monitor_idx.max(0);
        let monitors = Self::enumerate_monitors();
        let default_monitor_name = if let Some(m) = monitors.iter().find(|m| m.index == valid_idx) {
            m.name.clone()
        } else {
            format!("Monitor {}", valid_idx + 1)
        };

        // Capture exclusively the selected physical monitor via Windows GDI desktop capture.
        // Recognition happens strictly on this chosen monitor.
        let (w, h, px) = Self::capture_screen(Some(valid_idx))?;
        let (width, height, pixels, is_top_down, monitor_name) = (w, h, px, true, default_monitor_name);

        // 1. Two-Stage Cascaded Neural Network (ViT + NudeNet 640m)
        let cascade_res = self.cascade.classify_frame(&pixels, width, height, self.get_nsfw_threshold(), is_top_down);
        let is_nsfw = cascade_res.is_violation;
        let nsfw_score = cascade_res.score;
        let nsfw_label = cascade_res.primary_label.clone();
        let nsfw_category = cascade_res.primary_label.clone();

        let (ocr_violation, banned_words, ocr_text) = if !self.is_ocr_enabled() {
            (false, Vec::new(), "(OCR recognition disabled)".to_string())
        } else {
            let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
            let last_ocr = self.last_ocr_time.load(std::sync::atomic::Ordering::Relaxed);
            if now_ms.saturating_sub(last_ocr) >= 500 {
                let res = self.check_screen_pixels_for_stopwords(width, height, &pixels);
                self.last_ocr_time.store(now_ms, std::sync::atomic::Ordering::Relaxed);
                if let Ok(mut cache) = self.cached_ocr_result.lock() {
                    *cache = res.clone();
                }
                res
            } else if let Ok(cache) = self.cached_ocr_result.lock() {
                cache.clone()
            } else {
                (false, Vec::new(), String::new())
            }
        };

        let should_censor = is_nsfw || ocr_violation;

        let reason = if is_nsfw {
            cascade_res.primary_label.clone()
        } else if ocr_violation && !banned_words.is_empty() {
            format!("STOPWORD: {}", banned_words[0].to_uppercase())
        } else {
            "Neutral".to_string()
        };

        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let secs = now.as_secs();
        let millis = now.subsec_millis();
        let h = (secs / 3600) % 24;
        let m = (secs / 60) % 60;
        let s = secs % 60;
        let timestamp = format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis);

        Some(LiveFrameData {
            monitor_index: valid_idx,
            monitor_name,
            nsfw_category,
            nsfw_score,
            nsfw_label,
            nsfw_violation: is_nsfw,
            ocr_snippet: ocr_text,
            ocr_violation,
            banned_words,
            should_censor,
            reason,
            timestamp,
            stage1_score: cascade_res.stage1_score,
            stage1_label: cascade_res.stage1_label,
            stage2_score: cascade_res.stage2_score,
            obs_plugin_active: self.cascade.is_obs_plugin_active(),
            box_count: cascade_res.boxes.len(),
            censor_all: cascade_res.censor_all,
        })
    }

    /// Dedicated OCR / Banned Words Test:
    /// Evaluates text input or screen OCR against lexical dictionary.
    /// Activates ONLY if banned words are actually found, and writes detailed circumstances.
    pub fn run_ocr_test(&self, custom_text: Option<String>) -> SpecificTestResult {
        let m_idx = self.get_selected_monitor();
        let monitors = Self::enumerate_monitors();
        let monitor_label = if let Some(m) = monitors.iter().find(|m| m.index == m_idx) {
            m.name.clone()
        } else {
            format!("Monitor {}", m_idx + 1)
        };

        let (found, word, rule, source, snippet) = if let Some(custom) = custom_text.filter(|s| !s.trim().is_empty()) {
            let matches = if let Ok(engine) = self.lexical.try_lock() {
                engine.check_text(&custom)
            } else {
                Vec::new()
            };
            if !matches.is_empty() {
                (true, matches[0].word.clone(), matches[0].rule.clone(), "User Input".to_string(), custom)
            } else {
                (false, String::new(), String::new(), "User Input".to_string(), custom)
            }
        } else {
            let (has_banned, words, recognized) = self.check_screen_for_stopwords(Some(m_idx));
            let chars: Vec<char> = recognized.chars().collect();
            let short_snippet = if has_banned && !words.is_empty() {
                let target = &words[0];
                if let Some(idx) = recognized.to_lowercase().find(&target.to_lowercase()) {
                    let char_pos = recognized[..idx].chars().count();
                    let start = char_pos.saturating_sub(15);
                    let end = (char_pos + target.chars().count() + 15).min(chars.len());
                    let mut s = chars[start..end].iter().collect::<String>();
                    if start > 0 { s = format!("...{}", s); }
                    if end < chars.len() { s = format!("{}...", s); }
                    s
                } else if chars.len() > 100 {
                    format!("{}...", chars.iter().take(100).collect::<String>())
                } else {
                    chars.iter().collect::<String>()
                }
            } else if chars.len() > 100 {
                format!("{}...", chars.iter().take(100).collect::<String>())
            } else {
                chars.iter().collect::<String>()
            };

            let src_desc = format!("Screen OCR [{}]", monitor_label);

            if has_banned && !words.is_empty() {
                (true, words[0].clone(), "twitch_hate_speech".to_string(), src_desc, short_snippet)
            } else {
                (false, String::new(), String::new(), src_desc, short_snippet)
            }
        };

        let activated = found;
        let circumstances = if activated {
            if snippet.is_empty() {
                format!("Triggered: In source '{}' found prohibited word '{}' (rule: {}). Censor shield activated for 3 sec!", source, word, rule)
            } else {
                format!("Triggered: In source '{}' found prohibited word '{}' (rule: {}). Recognized text: \"{}\". Censor shield activated for 3 sec!", source, word, rule, snippet)
            }
        } else {
            if snippet.is_empty() {
                format!("Not triggered: In source '{}' no text detected. No prohibited words. Screen NOT covered.", source)
            } else {
                format!("Not triggered: In source '{}' prohibited words not found. Recognized text: \"{}\". Screen NOT covered.", source, snippet)
            }
        };

        SpecificTestResult {
            test_type: "ocr".to_string(),
            activated,
            score: if activated { 1.0 } else { 0.0 },
            matched_rule: if activated { rule } else { "none".to_string() },
            matched_item: if activated { word } else { String::new() },
            circumstances,
            gpu_info: self.gpu_name.clone(),
        }
    }

    /// Evaluates visual image buffer via OBS shared memory or GDI capture on selected monitor and two-stage neural cascade classifier.
    /// Activates ONLY if explicit NSFW is detected (or simulated), and writes circumstances with category and score.
    pub fn run_nsfw_test(&self, force_simulated: bool) -> SpecificTestResult {
        let m_idx = self.get_selected_monitor();
        let monitors = Self::enumerate_monitors();
        let monitor_label = if let Some(m) = monitors.iter().find(|m| m.index == m_idx) {
            m.name.clone()
        } else {
            format!("Monitor {}", m_idx + 1)
        };

        let threshold_pct = self.get_nsfw_threshold();

        let (is_nsfw, nsfw_score, nsfw_label, category, source) = if force_simulated {
            (true, 0.94, "Pornography".to_string(), "Pornography".to_string(), "Simulated test frame".to_string())
        } else {
            let captured = Self::capture_screen(Some(m_idx)).map(|(w, h, px)| (w, h, px, true, format!("Screen Capture [{}]", monitor_label)));

            match captured {
                Some((w, h, ref px, is_top_down, src_name)) => {
                    let cascade_res = self.cascade.classify_frame(px, w, h, threshold_pct, is_top_down);
                    (cascade_res.is_violation, cascade_res.score, cascade_res.primary_label.clone(), cascade_res.primary_label.clone(), src_name)
                }
                None => (false, 0.0, "Frame empty".to_string(), "Neutral".to_string(), format!("Screen Capture [{}]", monitor_label)),
            }
        };

        let activated = is_nsfw;
        let circumstances = if activated {
            format!("Triggered: {} detected illicit content (class: {}, probability: {:.0}% with threshold {}%). Censor shield activated for 3 sec!", source, nsfw_label, nsfw_score * 100.0, threshold_pct)
        } else {
            format!("Not triggered: {} — illicit content not detected (class: {}, probability: {:.0}%, threshold {}%). Screen NOT covered.", source, nsfw_label, nsfw_score * 100.0, threshold_pct)
        };

        SpecificTestResult {
            test_type: "nsfw".to_string(),
            activated,
            score: nsfw_score,
            matched_rule: if activated { category } else { "none".to_string() },
            matched_item: nsfw_label,
            circumstances,
            gpu_info: self.gpu_name.clone(),
        }
    }

    /// Synchronous raw screen check for the continuous guard loop while censor is active
    pub fn analyze_screen_raw(&self) -> ScreenAnalysisResult {
        let m_idx = self.get_selected_monitor();
        let captured = Self::capture_screen(Some(m_idx)).map(|(w, h, px)| (w, h, px, true));

        let (is_nsfw, nsfw_score, nsfw_label, nsfw_category, width, height, px_opt) = match captured {
            Some((w, h, px, is_top_down)) => {
                let cascade_res = self.cascade.classify_frame(&px, w, h, self.get_nsfw_threshold(), is_top_down);
                (cascade_res.is_violation, cascade_res.score, cascade_res.primary_label.clone(), cascade_res.primary_label.clone(), w, h, Some(px))
            }
            None => (false, 0.0, "Frame clean".to_string(), "Neutral".to_string(), 0, 0, None),
        };

        let (has_banned_text, banned_matches, _) = if let Some(ref px) = px_opt {
            self.check_screen_pixels_for_stopwords(width, height, px)
        } else {
            (false, Vec::new(), String::new())
        };
        let should_censor = is_nsfw || has_banned_text;

        let trigger_circumstances = if should_censor {
            if is_nsfw {
                format!("{}: {:.0}%", nsfw_category, nsfw_score * 100.0)
            } else {
                format!("OCR: {}", banned_matches.join(", "))
            }
        } else {
            "Frame safe".to_string()
        };

        let status_message = if should_censor {
            format!("NSFW DETECTED: {}", trigger_circumstances)
        } else {
            "NO NSFW DETECTED — Clean".to_string()
        };

        ScreenAnalysisResult {
            should_censor,
            is_nsfw,
            nsfw_score,
            nsfw_label,
            nsfw_category,
            has_banned_text,
            banned_matches,
            gpu_info: self.gpu_name.clone(),
            trigger_circumstances,
            status_message,
        }
    }

    /// Evaluates current screen against both OCR stop-words and visual NSFW model
    pub async fn analyze_screen(&self, force_trigger: bool) -> ScreenAnalysisResult {
        if force_trigger {
            return ScreenAnalysisResult {
                should_censor: true,
                is_nsfw: true,
                nsfw_score: 0.94,
                nsfw_label: "Pornography".to_string(),
                nsfw_category: "Pornography".to_string(),
                has_banned_text: false,
                banned_matches: Vec::new(),
                gpu_info: self.gpu_name.clone(),
                trigger_circumstances: "Simulated NSFW test trigger".to_string(),
                status_message: "NSFW DETECTED: Pornography (Censor test)".to_string(),
            };
        }

        self.analyze_screen_raw()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_and_capture_all_monitors() {
        let monitors = VisionEngine::enumerate_monitors();
        println!("\n=== Detected Monitors: {} ===", monitors.len());
        for m in &monitors {
            println!("Testing Monitor {}: {} ({}x{} at {},{})", m.index, m.name, m.width, m.height, m.x, m.y);
            if let Some((w, h, px)) = VisionEngine::capture_screen(Some(m.index)) {
                println!("Captured frame: {}x{}, buffer size: {}", w, h, px.len());
                let text = VisionEngine::recognize_text_from_bgra(w, h, &px);
                println!("OCR Recognized on Monitor {}: '{}'", m.index, text.trim());
            } else {
                println!("Failed to capture monitor {}", m.index);
            }
        }
    }

    #[tokio::test]
    async fn test_ocr_inside_tokio() {
        println!("Testing OCR inside tokio::test...");
        let monitors = VisionEngine::enumerate_monitors();
        if let Some(m) = monitors.first() {
            if let Some((w, h, px)) = VisionEngine::capture_screen(Some(m.index)) {
                let start = std::time::Instant::now();
                let text = VisionEngine::recognize_text_from_bgra(w, h, &px);
                println!("Tokio test OCR finished in {:.2}s: text len={}", start.elapsed().as_secs_f32(), text.len());
            }
        }
    }

    #[tokio::test]
    async fn test_spawn_blocking_ocr() {
        let lex = Arc::new(Mutex::new(LexicalEngine::new()));
        let vis = Arc::new(VisionEngine::new(lex));
        let vis_clone = vis.clone();
        let res = tokio::task::spawn_blocking(move || {
            vis_clone.run_ocr_test(None)
        }).await;
        match res {
            Ok(r) => println!("spawn_blocking result: activated={}, circ={}", r.activated, r.circumstances),
            Err(e) => println!("spawn_blocking PANICKED: {:?}", e),
        }
    }
}
