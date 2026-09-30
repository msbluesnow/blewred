use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

static OBS_INSTALL_DIR_CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();
static OBS_EXE_CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Creates a process Command that NEVER spawns a visible terminal/console window on Windows.
#[inline]
pub fn create_no_window_command(prog: &str) -> Command {
    let mut cmd = Command::new(prog);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Universal Path & Environment Resolver for blewred
/// Ensures 100% functionality across different PCs, drives, and user accounts.
pub struct PathResolver;

impl PathResolver {
    /// Detects the root directory of the blewred project/installation.
    /// Traverses parent directories from current_exe() and checks current_dir().
    pub fn find_project_root() -> PathBuf {
        // 1. Check explicit environment override
        if let Ok(env_root) = std::env::var("BLEWRED_ROOT") {
            let p = PathBuf::from(env_root);
            if p.exists() {
                return p;
            }
        }

        // 2. Check current working directory
        if let Ok(cwd) = std::env::current_dir() {
            if Self::is_project_root(&cwd) {
                return cwd;
            }
        }

        // 3. Walk upwards from current_exe()
        if let Ok(exe) = std::env::current_exe() {
            // First check immediate parent of current executable (primary candidate for installed app)
            if let Some(exe_dir) = exe.parent() {
                if Self::is_project_root(exe_dir) {
                    return exe_dir.to_path_buf();
                }
            }

            let mut curr = exe.parent();
            while let Some(dir) = curr {
                if Self::is_project_root(dir) {
                    return dir.to_path_buf();
                }
                curr = dir.parent();
            }

            // If running a release executable not in target/, its parent dir is the install root
            if let Some(exe_dir) = exe.parent() {
                let is_build_dir = exe_dir.ends_with("target\\debug")
                    || exe_dir.ends_with("target\\release")
                    || exe_dir.ends_with("target/debug")
                    || exe_dir.ends_with("target/release");
                if !is_build_dir {
                    return exe_dir.to_path_buf();
                }
            }
        }

        // 4. Fallback to current directory or AppData (strictly lowercase blewred)
        std::env::current_dir().unwrap_or_else(|_| {
            let appdata = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\".to_string());
            PathBuf::from(appdata).join("blewred")
        })
    }

    fn is_project_root(dir: &Path) -> bool {
        // Dev tree markers
        if dir.join("ui").join("index.html").exists()
            || dir.join("ui").join("censor_shield.html").exists()
            || (dir.join("src-tauri").exists() && dir.join("ui").exists())
        {
            return true;
        }

        // Installed release binary markers
        let has_exe = dir.join("blewred.exe").exists();
        let is_build_dir = dir.ends_with("target\\debug")
            || dir.ends_with("target\\release")
            || dir.ends_with("target/debug")
            || dir.ends_with("target/release");

        if has_exe && !is_build_dir {
            return true;
        }

        // Bundled extension or models markers
        if dir.join("extensions").join("chrome").exists()
            || (dir.join("extensions").exists() && dir.join("config").exists())
            || (dir.join("models").exists() && has_exe)
        {
            return true;
        }

        false
    }

    /// Finds a neural network model file on any PC
    pub fn find_model(filename: &str) -> Option<PathBuf> {
        Self::find_file_exact(filename)
    }

    fn find_file_exact(filename: &str) -> Option<PathBuf> {
        let root = Self::find_project_root();
        
        let candidate_paths = [
            root.join("models").join(filename),
            root.join("assets").join("models").join(filename),
            root.join("src-tauri").join("models").join(filename),
            PathBuf::from("models").join(filename),
            PathBuf::from("../models").join(filename),
            PathBuf::from("assets/models").join(filename),
            PathBuf::from("../assets/models").join(filename),
            PathBuf::from("src-tauri/models").join(filename),
            PathBuf::from("../src-tauri/models").join(filename),
        ];

        for p in &candidate_paths {
            if p.exists() {
                return Some(p.clone());
            }
        }

        // Check relative to current_exe()
        if let Ok(exe) = std::env::current_exe() {
            let mut curr = exe.parent();
            for _ in 0..5 {
                if let Some(dir) = curr {
                    let p1 = dir.join("models").join(filename);
                    if p1.exists() {
                        return Some(p1);
                    }
                    let p1_assets = dir.join("assets").join("models").join(filename);
                    if p1_assets.exists() {
                        return Some(p1_assets);
                    }
                    let p1_res = dir.join("resources").join("models").join(filename);
                    if p1_res.exists() {
                        return Some(p1_res);
                    }
                    let p2 = dir.join(filename);
                    if p2.exists() {
                        return Some(p2);
                    }
                    curr = dir.parent();
                } else {
                    break;
                }
            }
        }

        // Check LocalAppData / ProgramData (both lowercase blewred and legacy BlewRed)
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            let p_lower = PathBuf::from(&appdata).join("blewred").join("models").join(filename);
            if p_lower.exists() {
                return Some(p_lower);
            }
            let p = PathBuf::from(appdata).join("BlewRed").join("models").join(filename);
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(progdata) = std::env::var("PROGRAMDATA") {
            let p_lower = PathBuf::from(&progdata).join("blewred").join("models").join(filename);
            if p_lower.exists() {
                return Some(p_lower);
            }
            let p = PathBuf::from(progdata).join("BlewRed").join("models").join(filename);
            if p.exists() {
                return Some(p);
            }
        }

        None
    }

    /// Returns a guaranteed valid, absolute file path to `censor_shield.html` for OBS Studio browser source.
    /// If no file exists on disk, extracts the embedded template to `%LOCALAPPDATA%\blewred\censor_shield.html`.
    pub fn get_shield_html_path() -> String {
        let root = Self::find_project_root();
        let p1 = root.join("ui").join("censor_shield.html");
        if p1.exists() {
            return p1.to_string_lossy().to_string();
        }

        if let Ok(cwd) = std::env::current_dir() {
            let p2 = cwd.join("ui").join("censor_shield.html");
            if p2.exists() {
                return p2.to_string_lossy().to_string();
            }
            let p3 = cwd.join("censor_shield.html");
            if p3.exists() {
                return p3.to_string_lossy().to_string();
            }
        }

        // Self-extraction guarantee to LocalAppData
        let local_appdata = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\Temp".to_string());
        let target_dir = PathBuf::from(local_appdata).join("blewred").join("ui");
        let _ = std::fs::create_dir_all(&target_dir);
        let target_file = target_dir.join("censor_shield.html");

        let embedded_content = include_str!("../../ui/censor_shield.html");
        let need_write = match std::fs::read_to_string(&target_file) {
            Ok(existing) => existing.len() != embedded_content.len(),
            Err(_) => true,
        };

        if need_write {
            let _ = std::fs::write(&target_file, embedded_content);
            println!("[PathResolver] Deployed embedded censor_shield.html to: {:?}", target_file);
        }

        target_file.to_string_lossy().to_string()
    }

    /// Locates the Chrome browser extension directory.
    /// Checks root installation folder, resources folder, and ensures extraction if missing.
    pub fn find_extension_dir() -> PathBuf {
        let root = Self::find_project_root();

        let candidates = [
            root.join("extensions").join("chrome"),
            root.join("resources").join("extensions").join("chrome"),
            root.join("extensions"),
            root.join("resources").join("extensions"),
        ];

        for c in &candidates {
            if c.join("manifest.json").exists() {
                return c.clone();
            }
        }

        // Check relative to current_exe()
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let exe_candidates = [
                    dir.join("extensions").join("chrome"),
                    dir.join("resources").join("extensions").join("chrome"),
                    dir.join("extensions"),
                    dir.join("resources").join("extensions"),
                ];
                for c in &exe_candidates {
                    if c.join("manifest.json").exists() {
                        return c.clone();
                    }
                }
            }
        }

        // Check current working directory
        if let Ok(cwd) = std::env::current_dir() {
            let p = cwd.join("extensions").join("chrome");
            if p.join("manifest.json").exists() {
                return p;
            }
        }

        // Fallback: self-heal by extracting embedded extension to root.join("extensions").join("chrome")
        Self::ensure_extension_extracted(&root.join("extensions").join("chrome"))
    }

    /// Self-healing extractor: extracts embedded browser extension files if not present on disk
    pub fn ensure_extension_extracted(target_dir: &Path) -> PathBuf {
        let _ = std::fs::create_dir_all(target_dir);
        let icons_dir = target_dir.join("icons");
        let _ = std::fs::create_dir_all(&icons_dir);

        // Embedded extension files
        let manifest = include_str!("../../extensions/chrome/manifest.json");
        let content_js = include_str!("../../extensions/chrome/content.js");
        let bg_js = include_str!("../../extensions/chrome/background.js");
        let popup_html = include_str!("../../extensions/chrome/popup.html");
        let popup_js = include_str!("../../extensions/chrome/popup.js");
        let hls_js = include_str!("../../extensions/chrome/hls.min.js");
        let interceptor_js = include_str!("../../extensions/chrome/page_interceptor.js");

        let icon_32 = include_bytes!("../../extensions/chrome/icons/icon-32.png");
        let icon_128 = include_bytes!("../../extensions/chrome/icons/icon-128.png");
        let icon_512 = include_bytes!("../../extensions/chrome/icons/icon-512.png");

        let write_if_needed = |path: &Path, content: &[u8]| {
            let need_write = match std::fs::read(path) {
                Ok(existing) => existing != content,
                Err(_) => true,
            };
            if need_write {
                let _ = std::fs::write(path, content);
            }
        };

        write_if_needed(&target_dir.join("manifest.json"), manifest.as_bytes());
        write_if_needed(&target_dir.join("content.js"), content_js.as_bytes());
        write_if_needed(&target_dir.join("background.js"), bg_js.as_bytes());
        write_if_needed(&target_dir.join("popup.html"), popup_html.as_bytes());
        write_if_needed(&target_dir.join("popup.js"), popup_js.as_bytes());
        write_if_needed(&target_dir.join("hls.min.js"), hls_js.as_bytes());
        write_if_needed(&target_dir.join("page_interceptor.js"), interceptor_js.as_bytes());

        write_if_needed(&icons_dir.join("icon-32.png"), icon_32);
        write_if_needed(&icons_dir.join("icon-128.png"), icon_128);
        write_if_needed(&icons_dir.join("icon-512.png"), icon_512);

        println!("[PathResolver] Verified/extracted browser extension to: {:?}", target_dir);
        target_dir.to_path_buf()
    }

    /// Locates the `obs-blewred.dll` native plugin inside the project or system
    pub fn find_obs_plugin_dll() -> Option<PathBuf> {
        let root = Self::find_project_root();
        let candidates = [
            root.join("plugins").join("obs-blewred").join("dist").join("obs-blewred.dll"),
            root.join("plugins").join("obs-blewred").join("obs-blewred.dll"),
            PathBuf::from("plugins/obs-blewred/dist/obs-blewred.dll"),
            PathBuf::from("plugins/obs-blewred/obs-blewred.dll"),
            PathBuf::from("../plugins/obs-blewred/dist/obs-blewred.dll"),
            PathBuf::from("../plugins/obs-blewred/obs-blewred.dll"),
        ];

        for c in &candidates {
            if c.exists() {
                return Some(c.clone());
            }
        }

        if let Ok(exe) = std::env::current_exe() {
            let mut curr = exe.parent();
            for _ in 0..5 {
                if let Some(dir) = curr {
                    let p_dist = dir.join("plugins").join("obs-blewred").join("dist").join("obs-blewred.dll");
                    if p_dist.exists() {
                        return Some(p_dist);
                    }
                    let p_res_dist = dir.join("resources").join("plugins").join("obs-blewred").join("dist").join("obs-blewred.dll");
                    if p_res_dist.exists() {
                        return Some(p_res_dist);
                    }
                    let p_res = dir.join("resources").join("plugins").join("obs-blewred").join("obs-blewred.dll");
                    if p_res.exists() {
                        return Some(p_res);
                    }
                    let p = dir.join("plugins").join("obs-blewred").join("obs-blewred.dll");
                    if p.exists() {
                        return Some(p);
                    }
                    let p_root = dir.join("obs-blewred.dll");
                    if p_root.exists() {
                        return Some(p_root);
                    }
                    let p_res_root = dir.join("resources").join("obs-blewred.dll");
                    if p_res_root.exists() {
                        return Some(p_res_root);
                    }
                    curr = dir.parent();
                } else {
                    break;
                }
            }
        }

        // Check if already installed in user AppData
        if let Ok(appdata) = std::env::var("APPDATA") {
            let user_p = PathBuf::from(&appdata)
                .join("obs-studio")
                .join("plugins")
                .join("obs-blewred")
                .join("bin")
                .join("64bit")
                .join("obs-blewred.dll");
            if user_p.exists() {
                return Some(user_p);
            }
            let legacy_user_p = PathBuf::from(&appdata)
                .join("obs-studio")
                .join("obs-plugins")
                .join("64bit")
                .join("obs-blewred.dll");
            if legacy_user_p.exists() {
                return Some(legacy_user_p);
            }
        }

        // Check if already installed in system OBS
        if let Some(obs_dir) = Self::find_obs_install_dir() {
            let sys_p = obs_dir.join("obs-plugins").join("64bit").join("obs-blewred.dll");
            if sys_p.exists() {
                return Some(sys_p);
            }
        }

        None
    }

    /// Checks if the obs-blewred plugin is already installed in any OBS plugin directory
    pub fn is_obs_plugin_installed() -> bool {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let user_p = PathBuf::from(&appdata)
                .join("obs-studio")
                .join("plugins")
                .join("obs-blewred")
                .join("bin")
                .join("64bit")
                .join("obs-blewred.dll");
            if user_p.exists() {
                return true;
            }
            let legacy_user_p = PathBuf::from(&appdata)
                .join("obs-studio")
                .join("obs-plugins")
                .join("64bit")
                .join("obs-blewred.dll");
            if legacy_user_p.exists() {
                return true;
            }
        }
        if let Some(obs_dir) = Self::find_obs_install_dir() {
            let sys_p = obs_dir.join("obs-plugins").join("64bit").join("obs-blewred.dll");
            if sys_p.exists() {
                return true;
            }
        }
        false
    }

    /// Queries Windows Registry or common filesystem locations to find OBS Studio installation folder
    pub fn find_obs_install_dir() -> Option<PathBuf> {
        OBS_INSTALL_DIR_CACHE.get_or_init(|| {
            // 1. Check well-known filesystem paths across common drives FIRST (pure file check, 0 child processes)
            let drives = ["C", "D", "E", "F"];
            for d in &drives {
                let candidates = [
                    format!(r"{}:\Program Files\obs-studio", d),
                    format!(r"{}:\Program Files (x86)\obs-studio", d),
                    format!(r"{}:\SteamLibrary\steamapps\common\OBS Studio", d),
                    format!(r"{}:\Program Files (x86)\Steam\steamapps\common\OBS Studio", d),
                ];
                for c in &candidates {
                    let p = PathBuf::from(c);
                    if p.join("bin").join("64bit").join("obs64.exe").exists() {
                        return Some(p);
                    }
                }
            }

            // 2. Query Registry keys without showing console window
            let reg_queries = [
                r"HKLM\SOFTWARE\OBS Studio",
                r"HKLM\SOFTWARE\WOW6432Node\OBS Studio",
                r"HKCU\SOFTWARE\OBS Studio",
            ];

            for key in &reg_queries {
                if let Ok(output) = create_no_window_command("reg").args(["query", key, "/ve"]).output() {
                    if output.status.success() {
                        let text = String::from_utf8_lossy(&output.stdout);
                        for line in text.lines() {
                            if line.contains("REG_SZ") {
                                if let Some(pos) = line.find("REG_SZ") {
                                    let path_str = line[pos + 6..].trim();
                                    let p = PathBuf::from(path_str);
                                    if p.exists() {
                                        return Some(p);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 3. Check Uninstall key for InstallLocation
            let uninst_key = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\OBS Studio";
            if let Ok(output) = create_no_window_command("reg").args(["query", uninst_key, "/v", "InstallLocation"]).output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    for line in text.lines() {
                        if line.contains("REG_SZ") {
                            if let Some(pos) = line.find("REG_SZ") {
                                let path_str = line[pos + 6..].trim();
                                let p = PathBuf::from(path_str);
                                if p.exists() {
                                    return Some(p);
                                }
                            }
                        }
                    }
                }
            }

            None
        }).clone()
    }

    /// Finds the `obs64.exe` executable path (cached to run 0 child processes on repeated checks)
    pub fn find_obs_executable() -> Option<PathBuf> {
        OBS_EXE_CACHE.get_or_init(|| {
            // Check well-known filesystem paths FIRST
            let drives = ["C", "D", "E", "F"];
            for d in &drives {
                let candidates = [
                    format!(r"{}:\Program Files\obs-studio\bin\64bit\obs64.exe", d),
                    format!(r"{}:\Program Files (x86)\obs-studio\bin\64bit\obs64.exe", d),
                    format!(r"{}:\SteamLibrary\steamapps\common\OBS Studio\bin\64bit\obs64.exe", d),
                    format!(r"{}:\Program Files (x86)\Steam\steamapps\common\OBS Studio\bin\64bit\obs64.exe", d),
                ];
                for c in &candidates {
                    let p = PathBuf::from(c);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }

            if let Some(dir) = Self::find_obs_install_dir() {
                let exe = dir.join("bin").join("64bit").join("obs64.exe");
                if exe.exists() {
                    return Some(exe);
                }
            }

            // Fallback: Check PATH via where.exe without showing console window
            if let Ok(output) = create_no_window_command("where.exe").arg("obs64.exe").output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_line) = text.lines().next() {
                        let p = PathBuf::from(first_line.trim());
                        if p.exists() {
                            return Some(p);
                        }
                    }
                }
            }

            None
        }).clone()
    }

    /// Installs the native OBS filter plugin `obs-blewred.dll` automatically.
    /// Uses user-level plugin directory `%APPDATA%\obs-studio\plugins` (no admin rights needed!)
    /// and also attempts system directory `<obs_dir>\obs-plugins\64bit` if writable.
    pub fn install_obs_plugin() -> Result<String, String> {
        let src = match Self::find_obs_plugin_dll() {
            Some(s) => s,
            None => return Err("Source file obs-blewred.dll not found in project directory".to_string()),
        };

        let mut installed_locations = Vec::new();

        // 1. User profile plugin directory (OBS 28+ standard, no UAC elevation needed!)
        if let Ok(appdata) = std::env::var("APPDATA") {
            let user_plugin_dir = PathBuf::from(&appdata)
                .join("obs-studio")
                .join("plugins")
                .join("obs-blewred")
                .join("bin")
                .join("64bit");

            if let Ok(_) = std::fs::create_dir_all(&user_plugin_dir) {
                let dest = user_plugin_dir.join("obs-blewred.dll");
                if std::fs::copy(&src, &dest).is_ok() {
                    installed_locations.push(format!("user profile ({:?})", dest));
                } else if dest.exists() {
                    installed_locations.push(format!("user profile (already installed: {:?})", dest));
                }
            }

            // Also copy to legacy user obs-plugins
            let legacy_user_dir = PathBuf::from(&appdata)
                .join("obs-studio")
                .join("obs-plugins")
                .join("64bit");
            let _ = std::fs::create_dir_all(&legacy_user_dir);
            let legacy_dest = legacy_user_dir.join("obs-blewred.dll");
            if std::fs::copy(&src, &legacy_dest).is_ok() {
                installed_locations.push(format!("legacy user profile ({:?})", legacy_dest));
            } else if legacy_dest.exists() {
                installed_locations.push(format!("legacy user profile (already present: {:?})", legacy_dest));
            }
        }

        // 2. System OBS plugins directory (if accessible/writable or via UAC elevation)
        if let Some(obs_dir) = Self::find_obs_install_dir() {
            let sys_plugin_dir = obs_dir.join("obs-plugins").join("64bit");
            let _ = std::fs::create_dir_all(&sys_plugin_dir);
            let sys_dest = sys_plugin_dir.join("obs-blewred.dll");

            if std::fs::copy(&src, &sys_dest).is_ok() {
                installed_locations.push(format!("system OBS directory ({:?})", sys_dest));
            } else if sys_dest.exists() {
                installed_locations.push(format!("system OBS directory (already present: {:?})", sys_dest));
            } else {
                // If direct copy fails due to permissions (C:\Program Files requires UAC),
                // invoke elevated copy via PowerShell Start-Process -Verb RunAs
                println!("[PathResolver] System OBS plugin directory requires admin elevation. Triggering UAC prompt...");
                let _ = create_no_window_command("powershell.exe")
                    .args([
                        "-NoProfile",
                        "-WindowStyle", "Hidden",
                        "-Command",
                        &format!(
                            "Start-Process cmd.exe -ArgumentList '/c copy /y \"{}\" \"{}\"' -Verb RunAs -Wait",
                            src.to_string_lossy(),
                            sys_dest.to_string_lossy()
                        ),
                    ])
                    .status();

                if sys_dest.exists() {
                    installed_locations.push(format!("system OBS directory ({:?})", sys_dest));
                }
            }
        }

        if installed_locations.is_empty() {
            if Self::is_obs_plugin_installed() {
                Ok("obs-blewred plugin is already installed in OBS Studio".to_string())
            } else {
                Err("Failed to copy obs-blewred.dll to OBS directories (Administrator privileges required)".to_string())
            }
        } else {
            Ok(format!("obs-blewred plugin installed to: {}", installed_locations.join(", ")))
        }
    }

    /// Injects the `obs-blewred.dll` directly into the currently running OBS Studio process (obs64.exe)
    pub fn inject_obs_plugin_live() -> Result<String, String> {
        let src = match Self::find_obs_plugin_dll() {
            Some(s) => s,
            None => return Err("Source obs-blewred.dll not found in project directory".to_string()),
        };

        crate::injector::ObsInjector::inject_into_obs(&src)
    }

    /// Ensures OBS WebSocket configuration file exists and has `server_enabled = true`.
    /// Returns (port, Option<password>).
    pub fn ensure_obs_websocket_configured() -> (u16, Option<String>) {
        let appdata = match std::env::var("APPDATA") {
            Ok(a) => a,
            Err(_) => return (4455, None),
        };

        let cfg_dir = PathBuf::from(&appdata)
            .join("obs-studio")
            .join("plugin_config")
            .join("obs-websocket");
        let _ = std::fs::create_dir_all(&cfg_dir);
        let cfg_path = cfg_dir.join("config.json");

        if !cfg_path.exists() {
            // Create default open websocket config
            let default_cfg = serde_json::json!({
                "server_enabled": true,
                "ServerEnabled": true,
                "server_port": 4455,
                "auth_required": false,
                "alerts_enabled": false,
                "first_load": false
            });
            if let Ok(text) = serde_json::to_string_pretty(&default_cfg) {
                let _ = std::fs::write(&cfg_path, text);
                println!("[PathResolver] Created initial obs-websocket config at: {:?}", cfg_path);
            }
            return (4455, None);
        }

        let content = match std::fs::read_to_string(&cfg_path) {
            Ok(c) => c,
            Err(_) => return (4455, None),
        };

        let mut val: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return (4455, None),
        };

        let port = val.get("server_port")
            .or_else(|| val.get("ServerPort"))
            .and_then(|v| v.as_u64())
            .unwrap_or(4455) as u16;

        let pass = val.get("server_password")
            .or_else(|| val.get("ServerPassword"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let auth_req = val.get("auth_required")
            .or_else(|| val.get("AuthRequired"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let password = if auth_req { pass } else { None };

        // Ensure server_enabled is true
        let mut modified = false;
        if val.get("server_enabled").and_then(|v| v.as_bool()) != Some(true) {
            val["server_enabled"] = serde_json::json!(true);
            modified = true;
        }
        if val.get("ServerEnabled").and_then(|v| v.as_bool()) != Some(true) {
            val["ServerEnabled"] = serde_json::json!(true);
            modified = true;
        }

        if modified {
            if let Ok(text) = serde_json::to_string_pretty(&val) {
                let _ = std::fs::write(&cfg_path, text);
                println!("[PathResolver] Activated obs-websocket in existing config: {:?}", cfg_path);
            }
        }

        (port, password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_project_root() {
        let root = PathResolver::find_project_root();
        println!("Project root detected at: {:?}", root);
        assert!(root.exists(), "Project root directory should exist");
    }

    #[test]
    fn test_find_extension_dir() {
        let ext_dir = PathResolver::find_extension_dir();
        println!("Extension dir resolved to: {:?}", ext_dir);
        assert!(ext_dir.join("manifest.json").exists(), "manifest.json must exist in resolved extension dir");
    }

    #[test]
    fn test_ensure_extension_extracted_in_temp() {
        let temp_dir = std::env::temp_dir().join("blewred_test_ext_extract");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let extracted = PathResolver::ensure_extension_extracted(&temp_dir);
        assert_eq!(extracted, temp_dir);
        assert!(temp_dir.join("manifest.json").exists());
        assert!(temp_dir.join("content.js").exists());
        assert!(temp_dir.join("background.js").exists());
        assert!(temp_dir.join("popup.html").exists());
        assert!(temp_dir.join("popup.js").exists());
        assert!(temp_dir.join("icons").join("icon-32.png").exists());
        assert!(temp_dir.join("icons").join("icon-128.png").exists());
        assert!(temp_dir.join("icons").join("icon-512.png").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_models() {
        let m640 = PathResolver::find_model("640m.onnx");
        assert!(m640.is_some(), "640m.onnx should be found in project");
        let vit = PathResolver::find_model("vit_nsfw.onnx");
        assert!(vit.is_some(), "vit_nsfw.onnx should be found in project");
    }
}

