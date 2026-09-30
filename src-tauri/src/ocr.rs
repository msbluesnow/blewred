use base64::Engine;
use image::GenericImageView;

#[cfg(target_os = "windows")]
use windows::Media::Ocr::OcrEngine as WinOcrEngine;
#[cfg(target_os = "windows")]
use windows::Globalization::Language;
#[cfg(target_os = "windows")]
use windows::core::HSTRING;
#[cfg(target_os = "windows")]
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
#[cfg(target_os = "windows")]
use windows::Graphics::Imaging::BitmapDecoder;

/// High-Precision Native OCR Engine powered by Windows.Media.Ocr.
/// Features intelligent image pre-processing (contrast stretch, dark-mode luminance normalization),
/// high-fidelity Catmull-Rom 2x upscaling for small UI fonts, and dual-language Russian/English recognition.
pub struct WindowsOcrEngine;

/// Main OCR engine type
pub type OcrEngine = WindowsOcrEngine;

impl WindowsOcrEngine {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        println!("[WindowsOcrEngine] Native Windows.Media.Ocr engine active and ready.");
        Self
    }

    pub fn is_neural_ready(&self) -> bool {
        cfg!(target_os = "windows")
    }

    pub fn is_ready(&self) -> bool {
        cfg!(target_os = "windows")
    }

    /// Preprocesses image pixels:
    /// - Performs dynamic contrast stretching across luminance range
    /// - Normalizes dark background themes to light background for optimal text edge contrast
    pub fn preprocess_for_ocr(width: u32, height: u32, bgra_pixels: &[u8]) -> (u32, u32, Vec<u8>) {
        if width == 0 || height == 0 || bgra_pixels.len() < (width * height * 4) as usize {
            return (width, height, Vec::new());
        }

        // 1. Compute min, max, and average luminance
        let mut min_l = 255u8;
        let mut max_l = 0u8;
        let mut sum_brightness = 0u64;
        let mut count = 0u64;
        let step = (bgra_pixels.len() / 600).max(4) & !3;
        let mut idx = 0;
        while idx + 2 < bgra_pixels.len() {
            let b = bgra_pixels[idx] as u32;
            let g = bgra_pixels[idx + 1] as u32;
            let r = bgra_pixels[idx + 2] as u32;
            let lum = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
            if lum < min_l { min_l = lum; }
            if lum > max_l { max_l = lum; }
            sum_brightness += lum as u64;
            count += 1;
            idx += step;
        }

        let is_dark_theme = if count > 0 { (sum_brightness / count) < 128 } else { false };
        let range = (max_l.saturating_sub(min_l)).max(10) as f32;

        let mut preprocessed = Vec::with_capacity((width * height * 4) as usize);
        for chunk in bgra_pixels.chunks_exact(4) {
            // Dynamic contrast stretch
            let b_str = (((chunk[0].saturating_sub(min_l)) as f32 / range) * 255.0).clamp(0.0, 255.0) as u8;
            let g_str = (((chunk[1].saturating_sub(min_l)) as f32 / range) * 255.0).clamp(0.0, 255.0) as u8;
            let r_str = (((chunk[2].saturating_sub(min_l)) as f32 / range) * 255.0).clamp(0.0, 255.0) as u8;

            let (b, g, r) = if is_dark_theme {
                (255 - b_str, 255 - g_str, 255 - r_str)
            } else {
                (b_str, g_str, r_str)
            };

            preprocessed.push(b);
            preprocessed.push(g);
            preprocessed.push(r);
            preprocessed.push(chunk[3]);
        }

        (width, height, preprocessed)
    }

    /// Primary entry point: Recognizes text from image bytes using enhanced Windows Media OCR.
    pub fn recognize_text(&self, width: u32, height: u32, pixels: &[u8]) -> String {
        if width < 10 || height < 10 || pixels.is_empty() {
            return String::new();
        }

        let (_w, _h, prep_pixels) = Self::preprocess_for_ocr(width, height, pixels);
        let src_pixels = if prep_pixels.is_empty() { pixels } else { &prep_pixels };

        #[cfg(target_os = "windows")]
        {
            Self::recognize_with_windows_media_ocr(width, height, src_pixels)
        }
        #[cfg(not(target_os = "windows"))]
        {
            String::new()
        }
    }

    /// High-level entry point: decodes a base64 image (PNG, JPEG, WebP, etc.),
    /// processes it through Windows Media OCR, and parses schedule cues with fuzzy time logic.
    pub fn recognize_cues_from_image_base64(&self, image_base64: &str) -> (Vec<crate::cues::ScheduledCue>, String) {
        let clean_b64 = if let Some(idx) = image_base64.find("base64,") {
            &image_base64[idx + 7..]
        } else {
            image_base64
        };

        let raw_bytes = match base64::engine::general_purpose::STANDARD.decode(clean_b64) {
            Ok(b) => b,
            Err(_) => return (Vec::new(), String::new()),
        };

        let dyn_img = match image::load_from_memory(&raw_bytes) {
            Ok(img) => img,
            Err(_) => return (Vec::new(), String::new()),
        };

        let (w, h) = dyn_img.dimensions();
        if w == 0 || h == 0 {
            return (Vec::new(), String::new());
        }

        let rgba = dyn_img.to_rgba8();
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for chunk in rgba.as_raw().chunks_exact(4) {
            bgra.push(chunk[2]);
            bgra.push(chunk[1]);
            bgra.push(chunk[0]);
            bgra.push(chunk[3]);
        }

        let raw_text = self.recognize_text(w, h, &bgra);
        let cleaned_text = crate::cues::clean_cues_whitespace(&raw_text);
        let cues = crate::cues::parse_fuzzy_cues(&cleaned_text);
        (cues, cleaned_text)
    }

    #[cfg(target_os = "windows")]
    pub fn recognize_with_windows_media_ocr(orig_width: u32, orig_height: u32, orig_pixels: &[u8]) -> String {
        let res = (|| -> windows::core::Result<String> {
            if orig_width < 10 || orig_height < 10 || orig_pixels.len() < (orig_width * orig_height * 4) as usize {
                return Ok(String::new());
            }

            // Microsoft recommends text line height >= 40px for WinRT OCR.
            // For compact screenshots / snippets, upscale 2x via Catmull-Rom.
            let (width, height, up_pixels) = if orig_height < 120 {
                let mut bgra_img = image::RgbaImage::new(orig_width, orig_height);
                for (chunk, px) in orig_pixels.chunks_exact(4).zip(bgra_img.pixels_mut()) {
                    *px = image::Rgba([chunk[0], chunk[1], chunk[2], chunk[3]]);
                }
                let scaled = image::imageops::resize(
                    &bgra_img,
                    orig_width * 2,
                    orig_height * 2,
                    image::imageops::FilterType::CatmullRom,
                );
                (orig_width * 2, orig_height * 2, scaled.into_raw())
            } else {
                (orig_width, orig_height, orig_pixels.to_vec())
            };
            let pixels = &up_pixels;

            let mut bmp_data = Vec::with_capacity(54 + (width * height * 4) as usize);

            // BMP Header (14 bytes)
            bmp_data.extend_from_slice(b"BM");
            let file_size = (54 + (width * height * 4)) as u32;
            bmp_data.extend_from_slice(&file_size.to_le_bytes());
            bmp_data.extend_from_slice(&[0u8; 4]);
            let offset: u32 = 54;
            bmp_data.extend_from_slice(&offset.to_le_bytes());

            // DIB Header (40 bytes)
            let dib_size: u32 = 40;
            bmp_data.extend_from_slice(&dib_size.to_le_bytes());
            bmp_data.extend_from_slice(&(width as i32).to_le_bytes());
            let neg_height = -(height as i32);
            bmp_data.extend_from_slice(&neg_height.to_le_bytes());
            let planes: u16 = 1;
            bmp_data.extend_from_slice(&planes.to_le_bytes());
            let bpp: u16 = 32;
            bmp_data.extend_from_slice(&bpp.to_le_bytes());
            let compression: u32 = 0;
            bmp_data.extend_from_slice(&compression.to_le_bytes());
            let img_size = (width * height * 4) as u32;
            bmp_data.extend_from_slice(&img_size.to_le_bytes());
            bmp_data.extend_from_slice(&[0u8; 16]);

            bmp_data.extend_from_slice(pixels);

            let stream = InMemoryRandomAccessStream::new()?;
            let writer = DataWriter::CreateDataWriter(&stream)?;
            writer.WriteBytes(&bmp_data)?;
            writer.StoreAsync()?.get()?;
            let _ = writer.DetachStream();
            stream.Seek(0)?;

            let decoder = match BitmapDecoder::CreateAsync(&stream) {
                Ok(op) => match op.get() {
                    Ok(d) => d,
                    Err(_) => return Ok(String::new()),
                },
                Err(_) => return Ok(String::new()),
            };

            let bitmap = match decoder.GetSoftwareBitmapAsync() {
                Ok(op) => match op.get() {
                    Ok(b) => b,
                    Err(_) => return Ok(String::new()),
                },
                Err(_) => return Ok(String::new()),
            };

            let mut combined = String::new();

            // 1. Primary: Russian OCR (includes full Cyrillic, Latin, digits, and punctuation)
            if let Ok(lang_ru) = Language::CreateLanguage(&HSTRING::from("ru")) {
                if let Ok(engine_ru) = WinOcrEngine::TryCreateFromLanguage(&lang_ru) {
                    if let Ok(async_op) = engine_ru.RecognizeAsync(&bitmap) {
                        if let Ok(res) = async_op.get() {
                            if let Ok(t) = res.Text() {
                                let s = t.to_string();
                                if !s.is_empty() {
                                    combined.push_str(&s);
                                }
                            }
                        }
                    }
                }
            }

            // 2. Fallback: English OCR only if Russian OCR produced nothing (e.g. ru language pack not installed)
            if combined.trim().is_empty() {
                if let Ok(lang_en) = Language::CreateLanguage(&HSTRING::from("en-US")) {
                    if let Ok(engine_en) = WinOcrEngine::TryCreateFromLanguage(&lang_en) {
                        if let Ok(async_op) = engine_en.RecognizeAsync(&bitmap) {
                            if let Ok(res) = async_op.get() {
                                if let Ok(t) = res.Text() {
                                    let s = t.to_string();
                                    if !s.is_empty() {
                                        combined.push_str(&s);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Ok(combined)
        })();

        res.unwrap_or_default()
    }
}

static GLOBAL_OCR_ENGINE: std::sync::OnceLock<WindowsOcrEngine> = std::sync::OnceLock::new();

pub fn get_ocr_engine() -> &'static WindowsOcrEngine {
    GLOBAL_OCR_ENGINE.get_or_init(WindowsOcrEngine::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_ocr_engine_ready() {
        let engine = WindowsOcrEngine::new();
        assert!(engine.is_ready());
    }

    #[test]
    fn test_preprocess_contrast_stretch() {
        let pixels = vec![30u8, 30, 30, 255, 100, 100, 100, 255];
        let (w, h, prep) = WindowsOcrEngine::preprocess_for_ocr(2, 1, &pixels);
        assert_eq!(w, 2);
        assert_eq!(h, 1);
        assert_eq!(prep.len(), 8);
    }

    #[test]
    fn test_diagnose_user_screenshot() {
        // Look for an optional diagnostic image via environment variable or tests fixture
        let test_path = std::env::var("BLEWRED_OCR_TEST_IMAGE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                crate::paths::PathResolver::find_project_root()
                    .join("tests")
                    .join("fixtures")
                    .join("sample_cues.png")
            });

        if let Ok(dyn_img) = image::open(&test_path) {
            let (w, h) = (dyn_img.width(), dyn_img.height());
            use image::GenericImageView;
            let crop_x = (w as f32 * 0.48) as u32;
            let crop_y = (h as f32 * 0.34) as u32;
            let crop_w = (w as f32 * 0.38) as u32;
            let crop_h = (h as f32 * 0.11) as u32;
            let cropped = dyn_img.view(crop_x, crop_y, crop_w, crop_h).to_image();

            let engine = WindowsOcrEngine::new();

            let rgba = cropped.as_raw();
            let mut bgra = Vec::with_capacity(rgba.len());
            for chunk in rgba.chunks_exact(4) {
                bgra.push(chunk[2]);
                bgra.push(chunk[1]);
                bgra.push(chunk[0]);
                bgra.push(chunk[3]);
            }
            let full_text = engine.recognize_text(crop_w, crop_h, &bgra);
            println!("[Diag] Windows Media OCR output:\n---\n{}\n---", full_text);
            let cues = crate::cues::parse_fuzzy_cues(&full_text);
            println!("[Diag] Extracted cues count: {}", cues.len());
            for c in &cues {
                println!("[Diag] Cue: {} ({}..{}) => reason: '{}'", c.formatted_range, c.start_sec, c.end_sec, c.reason);
            }

            assert!(!full_text.is_empty(), "Recognized text must not be empty");
            assert!(!cues.is_empty(), "Should extract at least 1 cue from recognized text");
        } else {
            println!("[Diag] No diagnostic test image found at {:?}, skipping test.", test_path);
        }
    }
}
