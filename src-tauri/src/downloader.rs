use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use crate::paths::PathResolver;

/// Model descriptor containing cryptographic hash, expected size, and download URLs
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub filename: &'static str,
    pub display_name: &'static str,
    pub expected_size: u64,
    pub expected_sha256: &'static str,
    pub primary_url: &'static str,
    pub mirror_url: &'static str,
    pub additional_urls: &'static [&'static str],
    pub acceptable_sizes: &'static [u64],
    pub acceptable_hashes: &'static [&'static str],
}

pub const REQUIRED_MODELS: [ModelSpec; 2] = [
    ModelSpec {
        filename: "vit_nsfw.onnx",
        display_name: "Falconsai ViT Classifier (Stage 1)",
        expected_size: 87_348_425,
        expected_sha256: "10354606f3442d24308c0a37d1fd6205ab7ce7bf31669e186a4fc5cf7f51b455",
        primary_url: "https://huggingface.co/onnx-community/nsfw_image_detection-ONNX/resolve/main/onnx/model_quantized.onnx",
        mirror_url: "https://huggingface.co/d0gr/falconsai-nsfw-detector-onnx/resolve/main/nsfw_classifier/model.onnx",
        additional_urls: &[
            "https://huggingface.co/onnx-community/nsfw-classifier-ONNX/resolve/main/onnx/model_quantized.onnx",
            "https://huggingface.co/AdamCodd/vit-base-nsfw-detector/resolve/main/onnx/model_quantized.onnx",
            "https://github.com/blewred/blewred/releases/download/v1.0.0/vit_nsfw.onnx",
        ],
        acceptable_sizes: &[
            87_348_425,
            87_333_629,
            86_939_557,
            87_335_960,
            88_500_985,
        ],
        acceptable_hashes: &[
            "10354606f3442d24308c0a37d1fd6205ab7ce7bf31669e186a4fc5cf7f51b455",
            "37566bc62c49cff0699aad1a553161eb3508babcd81961fd3b067619881d5bd4",
            "656ea522313479ddcc5993791a812aae18d8086c0942e63ca80c7652c2db61eb",
            "5bafe76229a739eba9debf53864109b1c32f5bfb8fef2983d63333cc616bef6b",
            "c81a49b96d643f7938a383f973a805e728e07409a5fa77dc65d50c8799471368",
        ],
    },
    ModelSpec {
        filename: "640m.onnx",
        display_name: "NudeNet 640m Localizer (Stage 2)",
        expected_size: 103_538_690,
        expected_sha256: "5fd488c39acfb268efb4a92bce4fbc95967c059bd7ee1c01026fcaf81aec5c9e",
        primary_url: "https://huggingface.co/zhangsongbo365/nudenet_onnx/resolve/main/640m.onnx",
        mirror_url: "https://github.com/notAI-tech/NudeNet/releases/download/v0/640m.onnx",
        additional_urls: &[],
        acceptable_sizes: &[103_538_690],
        acceptable_hashes: &["5fd488c39acfb268efb4a92bce4fbc95967c059bd7ee1c01026fcaf81aec5c9e"],
    },
];
/// Native Windows.Media.Ocr requires 0 downloads (built into Windows 10/11)
pub const OCR_MODELS: [ModelSpec; 0] = [];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFileInfo {
    pub filename: String,
    pub display_name: String,
    pub expected_size: u64,
    pub expected_sha256: String,
    pub exists: bool,
    pub valid: bool,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsOverallStatus {
    pub all_ready: bool,
    pub models: Vec<ModelFileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    pub filename: String,
    pub display_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: f64,
    pub percent: f32,
    pub status: String, // "checking", "downloading", "verifying", "completed", "error"
    pub error: Option<String>,
}

pub struct ModelDownloader;

impl ModelDownloader {
    /// Computes the SHA-256 hex string of a file
    pub fn compute_file_sha256(path: &Path) -> Result<String, std::io::Error> {
        use std::io::Read;
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Determines target directory where models should be saved when downloaded.
    /// Prefers local ./models in project/installation root if writable, fallback to %LOCALAPPDATA%\blewred\models.
    pub fn get_target_models_dir() -> PathBuf {
        let root = PathResolver::find_project_root();
        let local_models = root.join("models");

        // Try to create and use the models folder inside the project/install directory
        if std::fs::create_dir_all(&local_models).is_ok() {
            return local_models;
        }

        // Fallback to %LOCALAPPDATA%\blewred\models
        let appdata = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\".to_string());
        let target = PathBuf::from(appdata).join("blewred").join("models");
        let _ = std::fs::create_dir_all(&target);
        target
    }

    /// Checks whether required models exist locally and match expected cryptographic hash
    pub fn check_models_status() -> ModelsOverallStatus {
        let mut all_ready = true;
        let mut models = Vec::new();

        for spec in &REQUIRED_MODELS {
            let found = PathResolver::find_model(spec.filename);
            let mut exists = false;
            let mut valid = false;
            let mut local_path = None;

            if let Some(ref p) = found {
                if p.exists() {
                    exists = true;
                    local_path = Some(p.to_string_lossy().to_string());

                    // Fast size check first
                    if let Ok(metadata) = std::fs::metadata(p) {
                        let size_ok = metadata.len() == spec.expected_size
                            || spec.acceptable_sizes.contains(&metadata.len());
                        if size_ok {
                            // File size matches; verify SHA-256
                            if let Ok(hash) = Self::compute_file_sha256(p) {
                                let hash_ok = hash.eq_ignore_ascii_case(spec.expected_sha256)
                                    || spec.acceptable_hashes.iter().any(|h| hash.eq_ignore_ascii_case(h));
                                if hash_ok {
                                    valid = true;
                                } else {
                                    eprintln!("[Downloader] SHA-256 mismatch for {:?}: expected {}, got {}", p, spec.expected_sha256, hash);
                                }
                            }
                        } else {
                            eprintln!("[Downloader] File size mismatch for {:?}: expected {}, got {}", p, spec.expected_size, metadata.len());
                        }
                    }
                }
            }

            if !valid {
                all_ready = false;
            }

            models.push(ModelFileInfo {
                filename: spec.filename.to_string(),
                display_name: spec.display_name.to_string(),
                expected_size: spec.expected_size,
                expected_sha256: spec.expected_sha256.to_string(),
                exists,
                valid,
                local_path,
            });
        }

        ModelsOverallStatus {
            all_ready,
            models,
        }
    }

    pub fn check_ocr_status() -> ModelsOverallStatus {
        let is_ready = cfg!(target_os = "windows");
        let models = vec![
            ModelFileInfo {
                filename: "windows_media_ocr".to_string(),
                display_name: "Windows.Media.Ocr (Native OS Engine)".to_string(),
                expected_size: 0,
                expected_sha256: String::new(),
                exists: is_ready,
                valid: is_ready,
                local_path: Some("Windows System Core".to_string()),
            }
        ];

        ModelsOverallStatus {
            all_ready: is_ready,
            models,
        }
    }

    /// Downloads a specific model asynchronously with progress reporting
    pub async fn download_model<F>(
        spec: &ModelSpec,
        cancel_token: Arc<AtomicBool>,
        progress_callback: F,
    ) -> Result<PathBuf, String>
    where
        F: Fn(DownloadProgressEvent) + Send + Sync + 'static,
    {
        let target_dir = Self::get_target_models_dir();
        let _ = std::fs::create_dir_all(&target_dir);
        let final_path = target_dir.join(spec.filename);
        let temp_path = target_dir.join(format!("{}.tmp", spec.filename));

        // Check if existing file is already valid at target path or anywhere in project
        let local_candidate = if final_path.exists() {
            Some(final_path.clone())
        } else {
            PathResolver::find_model(spec.filename)
        };

        if let Some(candidate) = local_candidate {
            if let Ok(meta) = std::fs::metadata(&candidate) {
                let size_ok = meta.len() == spec.expected_size
                    || spec.acceptable_sizes.contains(&meta.len())
                    || spec.expected_size == 0;
                if size_ok {
                    progress_callback(DownloadProgressEvent {
                        filename: spec.filename.to_string(),
                        display_name: spec.display_name.to_string(),
                        downloaded_bytes: meta.len(),
                        total_bytes: meta.len(),
                        speed_bytes_per_sec: 0.0,
                        percent: 100.0,
                        status: "verifying".to_string(),
                        error: None,
                    });

                    let is_valid = if spec.expected_sha256.is_empty() {
                        true
                    } else if let Ok(hash) = Self::compute_file_sha256(&candidate) {
                        hash.eq_ignore_ascii_case(spec.expected_sha256)
                            || spec.acceptable_hashes.iter().any(|h| hash.eq_ignore_ascii_case(h))
                    } else {
                        false
                    };

                    if is_valid {
                        // If found elsewhere, ensure it's also present at target_dir
                        if candidate != final_path && !final_path.exists() {
                            let _ = std::fs::copy(&candidate, &final_path);
                        }

                        progress_callback(DownloadProgressEvent {
                            filename: spec.filename.to_string(),
                            display_name: spec.display_name.to_string(),
                            downloaded_bytes: meta.len(),
                            total_bytes: meta.len(),
                            speed_bytes_per_sec: 0.0,
                            percent: 100.0,
                            status: "completed".to_string(),
                            error: None,
                        });
                        return Ok(candidate);
                    }
                }
            }
        }

        let mut urls = vec![spec.primary_url, spec.mirror_url];
        urls.extend_from_slice(spec.additional_urls);
        let mut last_err = String::new();

        let client = reqwest::Client::builder()
            .user_agent("blewred/0.5.0 (Windows NT 10.0; Win64; x64)")
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("Failed to initialize HTTP client: {}", e))?;

        for url in &urls {
            if cancel_token.load(Ordering::Relaxed) {
                return Err("Download cancelled by user".to_string());
            }

            println!("[Downloader] Attempting download for {} from {}", spec.filename, url);
            progress_callback(DownloadProgressEvent {
                filename: spec.filename.to_string(),
                display_name: spec.display_name.to_string(),
                downloaded_bytes: 0,
                total_bytes: spec.expected_size,
                speed_bytes_per_sec: 0.0,
                percent: 0.0,
                status: "downloading".to_string(),
                error: None,
            });

            match client.get(*url).send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        last_err = format!("HTTP error: {}", response.status());
                        continue;
                    }

                    let total_size = response.content_length().unwrap_or(spec.expected_size);

                    let mut file = match tokio::fs::File::create(&temp_path).await {
                        Ok(f) => f,
                        Err(e) => {
                            last_err = format!("Failed to create temporary file {:?}: {}", temp_path, e);
                            continue;
                        }
                    };

                    let mut stream = response.bytes_stream();
                    let mut downloaded: u64 = 0;
                    let mut hasher = Sha256::new();
                    let start_time = Instant::now();
                    let mut last_speed_time = Instant::now();
                    let mut last_speed_bytes = 0u64;

                    let mut download_ok = true;

                    while let Some(chunk_res) = stream.next().await {
                        if cancel_token.load(Ordering::Relaxed) {
                            let _ = tokio::fs::remove_file(&temp_path).await;
                            return Err("Download cancelled".to_string());
                        }

                        match chunk_res {
                            Ok(chunk) => {
                                if let Err(e) = file.write_all(&chunk).await {
                                    last_err = format!("Disk write error: {}", e);
                                    download_ok = false;
                                    break;
                                }
                                hasher.update(&chunk);
                                downloaded += chunk.len() as u64;

                                // Update speed measurement every 200ms
                                let now = Instant::now();
                                let elapsed = now.duration_since(last_speed_time).as_secs_f64();
                                if elapsed >= 0.2 {
                                    let bytes_diff = downloaded - last_speed_bytes;
                                    let current_speed = (bytes_diff as f64) / elapsed;
                                    last_speed_time = now;
                                    last_speed_bytes = downloaded;

                                    let pct = if total_size > 0 {
                                        ((downloaded as f32 / total_size as f32) * 100.0).min(99.0)
                                    } else {
                                        0.0
                                    };

                                    progress_callback(DownloadProgressEvent {
                                        filename: spec.filename.to_string(),
                                        display_name: spec.display_name.to_string(),
                                        downloaded_bytes: downloaded,
                                        total_bytes: total_size,
                                        speed_bytes_per_sec: current_speed,
                                        percent: pct,
                                        status: "downloading".to_string(),
                                        error: None,
                                    });
                                }
                            }
                            Err(e) => {
                                last_err = format!("Network stream error: {}", e);
                                download_ok = false;
                                break;
                            }
                        }
                    }

                    if !download_ok {
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        continue;
                    }

                    if let Err(e) = file.flush().await {
                        last_err = format!("Buffer flush error: {}", e);
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        continue;
                    }
                    drop(file);

                    // Verification phase
                    progress_callback(DownloadProgressEvent {
                        filename: spec.filename.to_string(),
                        display_name: spec.display_name.to_string(),
                        downloaded_bytes: downloaded,
                        total_bytes: total_size,
                        speed_bytes_per_sec: 0.0,
                        percent: 99.5,
                        status: "verifying".to_string(),
                        error: None,
                    });

                    let calculated_hash = format!("{:x}", hasher.finalize());
                    let hash_ok = calculated_hash.eq_ignore_ascii_case(spec.expected_sha256)
                        || spec.acceptable_hashes.iter().any(|h| calculated_hash.eq_ignore_ascii_case(h))
                        || (spec.expected_sha256.is_empty() && downloaded > 50_000_000);

                    if !hash_ok {
                        last_err = format!(
                            "SHA-256 checksum mismatch! Expected: {}, got: {}",
                            spec.expected_sha256, calculated_hash
                        );
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        continue;
                    }

                    // Move to final filename
                    if let Err(e) = std::fs::rename(&temp_path, &final_path) {
                        last_err = format!("Failed to rename temp file {:?} to {:?}: {}", temp_path, final_path, e);
                        continue;
                    }

                    let duration = start_time.elapsed().as_secs_f64();
                    println!(
                        "[Downloader] Successfully downloaded and verified {} ({:.1} MB in {:.1}s)",
                        spec.filename,
                        (downloaded as f64) / (1024.0 * 1024.0),
                        duration
                    );

                    progress_callback(DownloadProgressEvent {
                        filename: spec.filename.to_string(),
                        display_name: spec.display_name.to_string(),
                        downloaded_bytes: downloaded,
                        total_bytes: total_size,
                        speed_bytes_per_sec: 0.0,
                        percent: 100.0,
                        status: "completed".to_string(),
                        error: None,
                    });

                    return Ok(final_path);
                }
                Err(e) => {
                    last_err = format!("Connection error to {}: {}", url, e);
                    continue;
                }
            }
        }

        let _ = tokio::fs::remove_file(&temp_path).await;
        progress_callback(DownloadProgressEvent {
            filename: spec.filename.to_string(),
            display_name: spec.display_name.to_string(),
            downloaded_bytes: 0,
            total_bytes: spec.expected_size,
            speed_bytes_per_sec: 0.0,
            percent: 0.0,
            status: "error".to_string(),
            error: Some(last_err.clone()),
        });

        Err(format!("Failed to download {}: {}", spec.filename, last_err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_models_dir_discovery() {
        let dir = ModelDownloader::get_target_models_dir();
        assert!(dir.is_dir() || dir.parent().map(|p| p.is_dir()).unwrap_or(false));
    }

    #[test]
    fn test_models_status_check() {
        let status = ModelDownloader::check_models_status();
        println!("Models status: {:?}", status);
        if PathBuf::from("..").join("models").join("vit_nsfw.onnx").exists()
            || PathBuf::from("models").join("vit_nsfw.onnx").exists()
        {
            assert_eq!(status.models.len(), 2);
            let vit = status.models.iter().find(|m| m.filename == "vit_nsfw.onnx").unwrap();
            assert!(vit.exists);
            assert!(vit.valid);

            let m640 = status.models.iter().find(|m| m.filename == "640m.onnx").unwrap();
            assert!(m640.exists);
            assert!(m640.valid);

            assert!(status.all_ready);
        }
    }

    #[test]
    fn test_ocr_models_status_check() {
        let status = ModelDownloader::check_ocr_status();
        println!("OCR Models status: {:?}", status);
        for m in &status.models {
            println!("  OCR model {}: exists={}, valid={}, path={:?}", m.filename, m.exists, m.valid, m.local_path);
        }
    }

    #[tokio::test]
    async fn test_model_urls_reachable() {
        let client = reqwest::Client::builder()
            .user_agent("blewred/0.5.0 (Windows NT 10.0; Win64; x64)")
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap();

        for model in &REQUIRED_MODELS {
            let res = client.head(model.primary_url).send().await;
            println!("Testing primary URL for {}: {:?} -> {:?}", model.filename, model.primary_url, res.as_ref().map(|r| r.status()));
            assert!(res.is_ok(), "Primary URL should be reachable: {}", model.primary_url);
            let status = res.unwrap().status();
            assert!(status.is_success() || status.is_redirection(), "Primary URL for {} should return success or redirect, got: {}", model.filename, status);
        }
    }
}
