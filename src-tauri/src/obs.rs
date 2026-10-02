use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

static REQ_COUNTER: AtomicU64 = AtomicU64::new(100);

pub struct OBSClient {
    pub host: String,
    pub port: Arc<Mutex<u16>>,
    pub password: Arc<Mutex<Option<String>>>,
    pub is_connected: Arc<Mutex<bool>>,
    mute_deadlines: Arc<Mutex<HashMap<String, Instant>>>,
    tx: Arc<Mutex<Option<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>>>>,
    pub pending_requests: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
    pub selected_scene: Arc<Mutex<Option<String>>>,
    pub cached_scenes: Arc<Mutex<Vec<String>>>,
    pub shield_item_ids: Arc<Mutex<HashMap<String, i64>>>,
    pub video_resolution: Arc<Mutex<String>>,
    pub capture_active: Arc<Mutex<bool>>,
    pub capture_source_name: Arc<Mutex<String>>,
    pub is_shield_visible: Arc<std::sync::atomic::AtomicBool>,
    pub censor_trigger_seq: Arc<AtomicU64>,
    pub discovered_audio_inputs: Arc<Mutex<Vec<String>>>,
}

/// Detailed result of `auto_setup`: which stages succeeded and a human-readable report.
#[derive(serde::Serialize)]
pub struct ObsSetupReport {
    /// True when the plugin is either injected live or deployed to disk (usable after OBS restart).
    pub success: bool,
    /// DLL was copied into OBS plugin directories.
    pub install_ok: bool,
    /// DLL was loaded into the running obs64.exe process without restart.
    pub injection_ok: bool,
    pub message: String,
}

impl OBSClient {
    pub fn new(host: String, mut port: u16) -> Self {
        let (discovered_port, discovered_pass) = crate::paths::PathResolver::ensure_obs_websocket_configured();
        if discovered_port != 0 {
            port = discovered_port;
        }

        Self {
            host,
            port: Arc::new(Mutex::new(port)),
            password: Arc::new(Mutex::new(discovered_pass)),
            is_connected: Arc::new(Mutex::new(false)),
            mute_deadlines: Arc::new(Mutex::new(HashMap::new())),
            tx: Arc::new(Mutex::new(None)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            selected_scene: Arc::new(Mutex::new(None)),
            cached_scenes: Arc::new(Mutex::new(Vec::new())),
            shield_item_ids: Arc::new(Mutex::new(HashMap::new())),
            video_resolution: Arc::new(Mutex::new("1920×1080 @ 60 FPS".to_string())),
            capture_active: Arc::new(Mutex::new(false)),
            capture_source_name: Arc::new(Mutex::new("Checking...".to_string())),
            is_shield_visible: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            censor_trigger_seq: Arc::new(AtomicU64::new(0)),
            discovered_audio_inputs: Arc::new(Mutex::new(vec!["Mic/Aux".to_string(), "Desktop Audio".to_string()])),
        }
    }



    pub async fn connect(&self) -> bool {
        let port = *self.port.lock().await;
        let url = format!("ws://{}:{}", self.host, port);
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                let (mut sink, mut stream) = ws_stream.split();
                println!("[OBSClient] TCP connected to {}. Negotiating v5 handshake...", url);

                let mut authenticated = false;

                // Wait for OpCode 0 (Hello)
                if let Some(Ok(Message::Text(text))) = stream.next().await {
                    if let Ok(hello) = serde_json::from_str::<serde_json::Value>(&text) {
                        if hello["op"].as_i64() == Some(0) {
                            // Subscribe to General, Config, Scenes, Inputs, Filters, SceneItems
                            let mut identify_data = json!({
                                "rpcVersion": 1,
                                "eventSubscriptions": 1 | 2 | 4 | 8 | 32 | 128
                            });

                            // Check if authentication challenge is requested
                            if let Some(auth) = hello["d"]["authentication"].as_object() {
                                let challenge = auth.get("challenge").and_then(|v| v.as_str()).unwrap_or("");
                                let salt = auth.get("salt").and_then(|v| v.as_str()).unwrap_or("");
                                let pass_guard = self.password.lock().await;
                                let pass = pass_guard.as_deref().unwrap_or("");

                                let auth_str = Self::compute_auth_response(pass, salt, challenge);
                                identify_data["authentication"] = json!(auth_str);
                            }

                            // Send OpCode 1 (Identify)
                            let identify_msg = json!({
                                "op": 1,
                                "d": identify_data
                            });

                            if sink.send(Message::Text(identify_msg.to_string().into())).await.is_ok() {
                                // Wait for OpCode 2 (Identified)
                                if let Some(Ok(Message::Text(resp_text))) = stream.next().await {
                                    if let Ok(id_resp) = serde_json::from_str::<serde_json::Value>(&resp_text) {
                                        if id_resp["op"].as_i64() == Some(2) {
                                            println!("[OBSClient] Successfully IDENTIFIED with OBS Studio v5!");
                                            authenticated = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if authenticated {
                    *self.tx.lock().await = Some(sink);
                    *self.is_connected.lock().await = true;

                    let is_conn = self.is_connected.clone();
                    let pending_reqs = self.pending_requests.clone();
                    let sel_scene = self.selected_scene.clone();
                    let client_event_handle = self.clone_handle();

                    tokio::spawn(async move {
                        while let Some(msg_res) = stream.next().await {
                            match msg_res {
                                Ok(Message::Text(text)) => {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                        let op = val["op"].as_i64().unwrap_or(0);
                                        if op == 7 {
                                            // OpCode 7: RequestResponse -> resolve pending oneshot
                                            if let Some(req_id) = val["d"]["requestId"].as_str() {
                                                let mut map = pending_reqs.lock().await;
                                                if let Some(tx) = map.remove(req_id) {
                                                    let _ = tx.send(val["d"].clone());
                                                }
                                            }
                                        } else if op == 5 {
                                            // OpCode 5: Event
                                            let event_type = val["d"]["eventType"].as_str().unwrap_or("");
                                            if event_type == "CurrentProgramSceneChanged" {
                                                if let Some(sc) = val["d"]["eventData"]["sceneName"].as_str() {
                                                    *sel_scene.lock().await = Some(sc.to_string());
                                                    println!("[OBS Event] CurrentProgramSceneChanged -> {}", sc);
                                                    if (crate::ws_bridge::is_censor_active() || client_event_handle.is_shield_visible.load(Ordering::Relaxed)) && crate::ws_bridge::is_censor_shield_enabled() {
                                                        let client_re = client_event_handle.clone_handle();
                                                        let sc_name = sc.to_string();
                                                        tokio::spawn(async move {
                                                            if let Ok(id) = client_re.ensure_censor_shield_in_scene(&sc_name).await {
                                                                let _ = client_re.send_request("SetSceneItemIndex", json!({
                                                                    "sceneName": sc_name,
                                                                    "sceneItemId": id,
                                                                    "sceneItemIndex": 999
                                                                })).await;
                                                                let _ = client_re.send_request("SetSceneItemEnabled", json!({
                                                                    "sceneName": sc_name,
                                                                    "sceneItemId": id,
                                                                    "sceneItemEnabled": true
                                                                })).await;
                                                                println!("[OBSClient] Automatically moved active Censor Shield to new scene '{}'", sc_name);
                                                            }
                                                        });
                                                    }
                                                }
                                            } else if event_type == "SceneItemEnableStateChanged" {
                                                if let Some(event_data) = val["d"]["eventData"].as_object() {
                                                    let item_id = event_data.get("sceneItemId").and_then(|v| v.as_i64()).unwrap_or(-1);
                                                    let item_enabled = event_data.get("sceneItemEnabled").and_then(|v| v.as_bool()).unwrap_or(false);
                                                    let is_shield = {
                                                        let map = client_event_handle.shield_item_ids.lock().await;
                                                        map.values().any(|&id| id == item_id)
                                                    };
                                                    if is_shield {
                                                        client_event_handle.is_shield_visible.store(item_enabled, Ordering::Relaxed);
                                                        println!("[OBS Event] blewred_Censor_Shield visibility changed in OBS: {}", item_enabled);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Ok(_) => {}
                                Err(_) => break,
                            }
                        }
                        *is_conn.lock().await = false;
                        println!("[OBSClient] Disconnected from obs-websocket");
                    });

                    // Initial fetch of scenes and video settings
                    let client_init = self.clone_handle();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(150)).await;
                        let _ = client_init.refresh_scenes().await;
                        client_init.update_capture_and_video_telemetry().await;
                    });

                    true
                } else {
                    *self.is_connected.lock().await = false;
                    eprintln!("[OBSClient] Handshake with obs-websocket failed.");
                    false
                }
            }
            Err(_) => {
                *self.is_connected.lock().await = false;
                false
            }
        }
    }

    fn compute_auth_response(password: &str, salt: &str, challenge: &str) -> String {
        let mut hasher1 = Sha256::new();
        hasher1.update(password.as_bytes());
        hasher1.update(salt.as_bytes());
        let pass_salt_hash = hasher1.finalize();
        let secret = BASE64.encode(pass_salt_hash);

        let mut hasher2 = Sha256::new();
        hasher2.update(secret.as_bytes());
        hasher2.update(challenge.as_bytes());
        let final_hash = hasher2.finalize();
        BASE64.encode(final_hash)
    }

    pub fn start_reconnect_loop(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                let connected = *self.is_connected.lock().await;
                if !connected {
                    // Dynamically reload credentials from OBS config in case OBS changed them or newly generated
                    let (disc_port, disc_pass) = crate::paths::PathResolver::ensure_obs_websocket_configured();
                    if disc_port != 0 {
                        *self.port.lock().await = disc_port;
                        *self.password.lock().await = disc_pass;
                    }

                    let success = self.connect().await;
                    if success {
                        println!("[OBSClient] Background loop established connection to OBS");
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    }

    /// Sends a request to OBS WebSocket v5 and waits for the matching response
    pub async fn send_request(
        &self,
        request_type: &str,
        request_data: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let request_id = format!(
            "{}_{}",
            request_type,
            REQ_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut map = self.pending_requests.lock().await;
            map.insert(request_id.clone(), tx);
        }

        let payload = json!({
            "op": 6,
            "d": {
                "requestType": request_type,
                "requestId": request_id,
                "requestData": request_data
            }
        });

        self.send_raw(payload).await;

        match tokio::time::timeout(Duration::from_millis(3500), rx).await {
            Ok(Ok(response_d)) => {
                let status = &response_d["requestStatus"];
                if status["result"].as_bool().unwrap_or(false) {
                    Ok(response_d["responseData"].clone())
                } else {
                    let comment = status["comment"].as_str().unwrap_or("Unknown OBS error");
                    let code = status["code"].as_i64().unwrap_or(-1);
                    Err(format!("OBS error {}: {}", code, comment))
                }
            }
            Ok(Err(_)) => Err("Request dropped".to_string()),
            Err(_) => {
                let mut map = self.pending_requests.lock().await;
                map.remove(&request_id);
                Err("Request timeout".to_string())
            }
        }
    }

    pub async fn get_current_program_scene(&self) -> Result<String, String> {
        let res = self.send_request("GetCurrentProgramScene", json!({})).await?;
        if let Some(name) = res["currentProgramSceneName"].as_str().or_else(|| res["sceneName"].as_str()) {
            return Ok(name.to_string());
        }
        Err("No scene name in GetCurrentProgramScene".to_string())
    }

    pub async fn refresh_scenes(&self) -> Result<(String, Vec<String>), String> {
        let res = self.send_request("GetSceneList", json!({})).await?;
        let current = res["currentProgramSceneName"].as_str().unwrap_or("Scene").to_string();
        let mut scenes = Vec::new();

        if let Some(arr) = res["scenes"].as_array() {
            for s in arr {
                if let Some(name) = s["sceneName"].as_str() {
                    scenes.push(name.to_string());
                }
            }
        }

        {
            let mut cached = self.cached_scenes.lock().await;
            *cached = scenes.clone();
        }
        {
            let mut sel = self.selected_scene.lock().await;
            if sel.is_none() || sel.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                *sel = Some(current.clone());
            }
        }

        // Ensure shield exists in the active scene
        let _ = self.ensure_censor_shield_in_scene(&current).await;

        Ok((current, scenes))
    }

    pub async fn get_scene_item_id(&self, scene_name: &str, source_name: &str) -> Result<i64, String> {
        // Check cache first
        {
            let map = self.shield_item_ids.lock().await;
            if let Some(&id) = map.get(scene_name) {
                return Ok(id);
            }
        }

        // Query GetSceneItemId
        if let Ok(res) = self.send_request("GetSceneItemId", json!({
            "sceneName": scene_name,
            "sourceName": source_name
        })).await {
            if let Some(id) = res["sceneItemId"].as_i64() {
                let mut map = self.shield_item_ids.lock().await;
                map.insert(scene_name.to_string(), id);
                return Ok(id);
            }
        }

        // Fallback: search in GetSceneItemList
        let items_res = self.send_request("GetSceneItemList", json!({
            "sceneName": scene_name
        })).await?;

        if let Some(arr) = items_res["sceneItems"].as_array() {
            for item in arr {
                if item["sourceName"].as_str() == Some(source_name) {
                    if let Some(id) = item["sceneItemId"].as_i64() {
                        let mut map = self.shield_item_ids.lock().await;
                        map.insert(scene_name.to_string(), id);
                        return Ok(id);
                    }
                }
            }
        }

        Err(format!("Source '{}' not found in scene '{}'", source_name, scene_name))
    }

    pub fn get_shield_html_path() -> String {
        crate::paths::PathResolver::get_shield_html_path()
    }

    pub async fn ensure_censor_shield_in_scene(&self, scene_name: &str) -> Result<i64, String> {
        let html_path = Self::get_shield_html_path();

        // 0. Auto-migrate legacy capitalized "BlewRed_Censor_Shield" if present in OBS
        if let Ok(_) = self.send_request("GetInputSettings", json!({ "inputName": "BlewRed_Censor_Shield" })).await {
            println!("[OBSClient] Migrating legacy capitalized 'BlewRed_Censor_Shield' to 'blewred_Censor_Shield' in OBS...");
            let rename_res = self.send_request("SetInputName", json!({
                "inputName": "BlewRed_Censor_Shield",
                "newInputName": "blewred_Censor_Shield"
            })).await;
            if rename_res.is_err() {
                let _ = self.send_request("RemoveInput", json!({ "inputName": "BlewRed_Censor_Shield" })).await;
            }
            let mut map = self.shield_item_ids.lock().await;
            map.clear();
        }

        // 1. Inspect existing input in OBS Studio
        if let Ok(settings_res) = self.send_request("GetInputSettings", json!({ "inputName": "blewred_Censor_Shield" })).await {
            let kind = settings_res["inputKind"].as_str().unwrap_or("");
            if kind == "color_source_v3" {
                println!("[OBSClient] Replacing legacy black color source with gradient browser source in OBS...");
                let _ = self.send_request("RemoveInput", json!({ "inputName": "blewred_Censor_Shield" })).await;
                let mut map = self.shield_item_ids.lock().await;
                map.remove(scene_name);
            } else if kind == "browser_source" {
                // Ensure local file path points to censor_shield.html
                let _ = self.send_request("SetInputSettings", json!({
                    "inputName": "blewred_Censor_Shield",
                    "inputSettings": {
                        "is_local_file": true,
                        "local_file": html_path,
                        "width": 1920,
                        "height": 1080,
                        "fps": 60
                    }
                })).await;

                // Ensure browser source is refreshed
                let _ = self.send_request("PressInputPropertiesButton", json!({
                    "inputName": "blewred_Censor_Shield",
                    "propertyName": "refreshnocache"
                })).await;

                if let Ok(id) = self.get_scene_item_id(scene_name, "blewred_Censor_Shield").await {
                    let _ = self.send_request("SetSceneItemTransform", json!({
                        "sceneName": scene_name,
                        "sceneItemId": id,
                        "sceneItemTransform": {
                            "positionX": 0.0,
                            "positionY": 0.0,
                            "boundsWidth": 1920.0,
                            "boundsHeight": 1080.0,
                            "boundsType": "OBS_BOUNDS_STRETCH"
                        }
                    })).await;

                    // In OBS Studio WebSocket v5, higher index means higher/top layer!
                    let _ = self.send_request("SetSceneItemIndex", json!({
                        "sceneName": scene_name,
                        "sceneItemId": id,
                        "sceneItemIndex": 999
                    })).await;

                    return Ok(id);
                } else {
                    // Source exists in OBS globally, but not in this scene -> attach it!
                    if let Ok(item_res) = self.send_request("CreateSceneItem", json!({
                        "sceneName": scene_name,
                        "sourceName": "blewred_Censor_Shield"
                    })).await {
                        if let Some(created_id) = item_res["sceneItemId"].as_i64() {
                            let mut map = self.shield_item_ids.lock().await;
                            map.insert(scene_name.to_string(), created_id);

                            let _ = self.send_request("SetSceneItemTransform", json!({
                                "sceneName": scene_name,
                                "sceneItemId": created_id,
                                "sceneItemTransform": {
                                    "positionX": 0.0,
                                    "positionY": 0.0,
                                    "boundsWidth": 1920.0,
                                    "boundsHeight": 1080.0,
                                    "boundsType": "OBS_BOUNDS_STRETCH"
                                }
                            })).await;

                            let _ = self.send_request("SetSceneItemIndex", json!({
                                "sceneName": scene_name,
                                "sceneItemId": created_id,
                                "sceneItemIndex": 999
                            })).await;

                            return Ok(created_id);
                        }
                    }
                }
            }
        }

        // 2. Create as modern browser_source with gradient canvas
        println!("[OBSClient] Creating blewred_Censor_Shield (browser_source gradient) in scene '{}'...", scene_name);
        let res = self.send_request("CreateInput", json!({
            "sceneName": scene_name,
            "inputName": "blewred_Censor_Shield",
            "inputKind": "browser_source",
            "inputSettings": {
                "is_local_file": true,
                "local_file": html_path,
                "width": 1920,
                "height": 1080,
                "fps": 60,
                "restart_when_active": false,
                "reroute_audio": false
            },
            "sceneItemEnabled": false
        })).await;

        let item_id = match res {
            Ok(data) => data["sceneItemId"].as_i64(),
            Err(e) => {
                println!("[OBSClient] CreateInput note: {}. Checking items...", e);
                None
            }
        };

        let id = match item_id {
            Some(id) => id,
            None => self.get_scene_item_id(scene_name, "blewred_Censor_Shield").await?,
        };

        {
            let mut map = self.shield_item_ids.lock().await;
            map.insert(scene_name.to_string(), id);
        }

        // Fit to full 1920x1080 screen transform
        let _ = self.send_request("SetSceneItemTransform", json!({
            "sceneName": scene_name,
            "sceneItemId": id,
            "sceneItemTransform": {
                "positionX": 0.0,
                "positionY": 0.0,
                "boundsWidth": 1920.0,
                "boundsHeight": 1080.0,
                "boundsType": "OBS_BOUNDS_STRETCH"
            }
        })).await;

        // Position on top of all layers (OBS clamps 999 to topmost index)
        let _ = self.send_request("SetSceneItemIndex", json!({
            "sceneName": scene_name,
            "sceneItemId": id,
            "sceneItemIndex": 999
        })).await;

        let _ = self.send_request("PressInputPropertiesButton", json!({
            "inputName": "blewred_Censor_Shield",
            "propertyName": "refreshnocache"
        })).await;

        println!("[OBSClient] blewred_Censor_Shield ready in '{}' (id: {}) on top layer", scene_name, id);
        Ok(id)
    }

    pub async fn refresh_censor_shield(&self) {
        let _ = self.send_request("PressInputPropertiesButton", json!({
            "inputName": "blewred_Censor_Shield",
            "propertyName": "refreshnocache"
        })).await;
    }

    pub async fn update_capture_and_video_telemetry(&self) {
        // 1. Get real video canvas resolution & FPS
        if let Ok(settings) = self.send_request("GetVideoSettings", json!({})).await {
            let base_w = settings["baseWidth"].as_i64().unwrap_or(1920);
            let base_h = settings["baseHeight"].as_i64().unwrap_or(1080);
            let fps_num = settings["fpsNumerator"].as_f64().unwrap_or(60.0);
            let fps_den = settings["fpsDenominator"].as_f64().unwrap_or(1.0);
            let fps = (fps_num / fps_den).round() as i64;
            *self.video_resolution.lock().await = format!("{}×{} @ {} FPS", base_w, base_h, fps);
        }

        // 2. Get target scene
        let scene = {
            let sel = self.selected_scene.lock().await;
            sel.clone().unwrap_or_else(|| "Scene".to_string())
        };

        // 3. Inspect items in this scene to check if capture is ACTUALLY active (eye open)
        if let Ok(res) = self.send_request("GetSceneItemList", json!({ "sceneName": scene })).await {
            if let Some(items) = res["sceneItems"].as_array() {
                let mut found_active = false;
                let mut candidate_name = String::new();

                for item in items {
                    let source_name = item["sourceName"].as_str().unwrap_or("");
                    let kind = item["inputKind"].as_str().unwrap_or("");
                    let enabled = item["sceneItemEnabled"].as_bool().unwrap_or(false);

                    if source_name == "blewred_Censor_Shield" || source_name == "BlewRed_Censor_Shield" {
                        if source_name == "BlewRed_Censor_Shield" {
                            let _ = self.send_request("SetInputName", json!({
                                "inputName": "BlewRed_Censor_Shield",
                                "newInputName": "blewred_Censor_Shield"
                            })).await;
                        }
                        if let Some(id) = item["sceneItemId"].as_i64() {
                            let mut map = self.shield_item_ids.lock().await;
                            map.insert(scene.clone(), id);
                        }
                        continue;
                    }

                    // Check for video/screen capture kinds
                    let is_capture = kind.contains("capture")
                        || kind.contains("monitor")
                        || kind.contains("game")
                        || kind.contains("window")
                        || kind.contains("display")
                        || kind.contains("dshow");

                    if is_capture {
                        if enabled {
                            found_active = true;
                            candidate_name = source_name.to_string();
                            break;
                        } else if candidate_name.is_empty() {
                            candidate_name = format!("{} (hidden)", source_name);
                        }
                    }
                }

                *self.capture_active.lock().await = found_active;
                *self.capture_source_name.lock().await = if found_active {
                    candidate_name
                } else if !candidate_name.is_empty() {
                    candidate_name
                } else {
                    "No capture source in scene".to_string()
                };
            }
        }
    }

    pub async fn trigger_visual_censor(&self, duration_ms: u64) {
        let shield_allowed = crate::ws_bridge::is_censor_shield_enabled();
        let trigger_id = self.censor_trigger_seq.fetch_add(1, Ordering::SeqCst) + 1;
        println!("[OBSClient] Visual Censor Triggered (seq: {}) for {}ms (shield_banner_allowed: {})", trigger_id, duration_ms, shield_allowed);

        if shield_allowed {
            self.set_censor_shield_enabled(true).await;
            self.mute_all_audio(true).await;
        } else {
            self.set_censor_shield_enabled(false).await;
        }

        let client = self.clone_handle();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(duration_ms)).await;
            // Only deactivate if no new violation arrived during this duration
            if client.censor_trigger_seq.load(Ordering::SeqCst) == trigger_id {
                client.set_censor_shield_enabled(false).await;
                client.mute_all_audio(false).await;
                println!("[OBSClient] Visual Censor Shield deactivated, stream restored (seq: {})", trigger_id);
            } else {
                println!("[OBSClient] Censor Shield auto-restore skipped: newer violation active");
            }
        });
    }

    pub async fn set_censor_shield_enabled(&self, enabled: bool) {
        let connected = *self.is_connected.lock().await;
        if !connected {
            return;
        }

        if enabled {
            self.is_shield_visible.store(true, Ordering::Relaxed);

            let scene = match self.get_current_program_scene().await {
                Ok(sc) => {
                    *self.selected_scene.lock().await = Some(sc.clone());
                    sc
                }
                Err(_) => {
                    let sel = self.selected_scene.lock().await;
                    sel.clone().unwrap_or_else(|| "Scene".to_string())
                }
            };

            match self.ensure_censor_shield_in_scene(&scene).await {
                Ok(item_id) => {
                    // Bring to absolute top layer when activating
                    let _ = self.send_request("SetSceneItemIndex", json!({
                        "sceneName": scene,
                        "sceneItemId": item_id,
                        "sceneItemIndex": 999
                    })).await;

                    let req = json!({
                        "sceneName": scene,
                        "sceneItemId": item_id,
                        "sceneItemEnabled": true
                    });
                    let _ = self.send_request("SetSceneItemEnabled", req).await;
                    println!("[OBSClient] SUCCESS: Censor Shield ENABLED in '{}' (id: {})", scene, item_id);
                }
                Err(e) => {
                    eprintln!("[OBSClient] Failed to ensure Censor Shield in '{}': {}", scene, e);
                }
            }
        } else {
            // OPTIMIZED & ROBUST DEACTIVATION:
            // 1. Immediately mark shield as not visible
            self.is_shield_visible.store(false, Ordering::Relaxed);

            // 2. Concurrently disable shield in all known scenes without sequential blocking
            let cached_scenes = {
                let map = self.shield_item_ids.lock().await;
                map.clone()
            };

            if !cached_scenes.is_empty() {
                let mut disable_futures = Vec::new();
                for (sc_name, sc_id) in cached_scenes {
                    let req = json!({
                        "sceneName": sc_name,
                        "sceneItemId": sc_id,
                        "sceneItemEnabled": false
                    });
                    disable_futures.push(self.send_request("SetSceneItemEnabled", req));
                }
                let _ = futures_util::future::join_all(disable_futures).await;
            }

            // 3. Immediately restore audio across all discovered inputs with zero delay
            self.mute_all_audio(false).await;
            println!("[OBSClient] SUCCESS: Censor Shield DEACTIVATED across all scenes and audio restored");
        }
    }

    /// Discovers all audio inputs (microphones, auxiliary, desktop audio) for comprehensive mute safety
    pub async fn refresh_audio_inputs(&self) -> Vec<String> {
        let mut audio_list: Vec<String> = Vec::new();

        // 1. Check special inputs from OBS
        if let Ok(special) = self.send_request("GetSpecialInputs", json!({})).await {
            for key in ["desktop1", "desktop2", "mic1", "mic2", "mic3", "mic4"] {
                if let Some(name) = special.get(key).and_then(|v| v.as_str()) {
                    if !name.trim().is_empty() && !audio_list.contains(&name.to_string()) {
                        audio_list.push(name.to_string());
                    }
                }
            }
        }

        // 2. Check all inputs in scene collection
        if let Ok(inputs_res) = self.send_request("GetInputList", json!({})).await {
            if let Some(inputs) = inputs_res["inputs"].as_array() {
                for inp in inputs {
                    let name = inp["inputName"].as_str().unwrap_or("").to_string();
                    let kind = inp["inputKind"].as_str().unwrap_or("").to_lowercase();
                    let unversioned = inp["unversionedInputKind"].as_str().unwrap_or("").to_lowercase();
                    if kind.contains("audio") || kind.contains("wasapi") || kind.contains("mic")
                        || unversioned.contains("audio") || unversioned.contains("wasapi") {
                        if !name.trim().is_empty() && !audio_list.contains(&name) {
                            audio_list.push(name);
                        }
                    }
                }
            }
        }

        if audio_list.is_empty() {
            audio_list.push("Mic/Aux".to_string());
            audio_list.push("Desktop Audio".to_string());
        }

        {
            let mut disc = self.discovered_audio_inputs.lock().await;
            *disc = audio_list.clone();
        }

        audio_list
    }

    /// Automatically attaches the blewred GPU Censor Filter (`blewred_filter`)
    /// to detected screen/game/window/display capture sources via OBS WebSocket v5 `CreateSourceFilter`
    pub async fn attach_censor_filters_to_captures(&self, scenes: &[String]) -> Vec<String> {
        let mut attached = Vec::new();
        let mut scanned_sources = std::collections::HashSet::new();

        for scene in scenes {
            if let Ok(res) = self.send_request("GetSceneItemList", json!({ "sceneName": scene })).await {
                if let Some(items) = res["sceneItems"].as_array() {
                    // Prioritize enabled capture items first
                    let mut sorted_items = items.clone();
                    sorted_items.sort_by(|a, b| {
                        let a_en = a["sceneItemEnabled"].as_bool().unwrap_or(false);
                        let b_en = b["sceneItemEnabled"].as_bool().unwrap_or(false);
                        b_en.cmp(&a_en)
                    });

                    for item in sorted_items {
                        let source_name = item["sourceName"].as_str().unwrap_or("").to_string();
                        let kind = item["inputKind"].as_str().unwrap_or("").to_lowercase();
                        let enabled = item["sceneItemEnabled"].as_bool().unwrap_or(false);

                        if source_name.is_empty() || source_name == "blewred_Censor_Shield" || source_name == "BlewRed_Censor_Shield" {
                            continue;
                        }

                        let is_capture = kind.contains("capture")
                            || kind.contains("monitor")
                            || kind.contains("game")
                            || kind.contains("window")
                            || kind.contains("display")
                            || kind.contains("dshow");

                        if is_capture && scanned_sources.insert(source_name.clone()) {
                            // Cleanly remove any dead/zombie blewred_filter on this source first
                            if let Ok(flt_res) = self.send_request("GetSourceFilterList", json!({ "sourceName": source_name })).await {
                                if let Some(flts) = flt_res["filters"].as_array() {
                                    for f in flts {
                                        let fname = f["filterName"].as_str().unwrap_or("");
                                        let fkind = f["filterKind"].as_str().unwrap_or("");
                                        if fkind == "blewred_filter" || fname.to_lowercase().contains("blewred") {
                                            let _ = self.send_request("RemoveSourceFilter", json!({
                                                "sourceName": source_name,
                                                "filterName": fname
                                            })).await;
                                        }
                                    }
                                }
                            }

                            if !enabled {
                                continue;
                            }

                            let create_res = self.send_request("CreateSourceFilter", json!({
                                "sourceName": source_name,
                                "filterName": "blewred_GPU_Censor",
                                "filterKind": "blewred_filter",
                                "filterSettings": {
                                    "censor_mode": 0
                                }
                            })).await;

                            match create_res {
                                Ok(_) => {
                                    println!("[OBSClient] Successfully attached blewred_filter to active source '{}'", source_name);
                                    attached.push(format!("{} (active)", source_name));
                                }
                                Err(e) => {
                                    println!("[OBSClient] Note attaching blewred_filter to '{}': {}", source_name, e);
                                }
                            }

                            // Attached to the active capture source
                            break;
                        }
                    }
                }
            }
        }

        attached
    }

    pub async fn auto_setup(&self) -> Result<ObsSetupReport, String> {
        // 1. Ensure OBS WebSocket configuration is active and dynamically reload port/password
        let (disc_port, disc_pass) = crate::paths::PathResolver::ensure_obs_websocket_configured();
        if disc_port != 0 {
            *self.port.lock().await = disc_port;
            *self.password.lock().await = disc_pass;
        }

        // 2. Deploy obs-blewred.dll to OBS plugin directories (user profile + system)
        let install_res = crate::paths::PathResolver::install_obs_plugin();
        let install_ok = install_res.is_ok();
        let install_status = match &install_res {
            Ok(loc) => format!("Plugin files: deployed ({})", loc),
            Err(e) => format!("Plugin files: ERROR — {}", e),
        };

        // 3. Connect to OBS Studio (or auto-launch if closed)
        let mut connected = *self.is_connected.lock().await;
        if !connected {
            connected = self.connect().await;
        }

        if !connected {
            let is_running = crate::injector::ObsInjector::find_obs_process().is_some();
            if !is_running {
                if let Some(obs_exe) = crate::paths::PathResolver::find_obs_executable() {
                    println!("[OBSClient] Launching OBS Studio from: {:?}", obs_exe);
                    let working_dir = obs_exe.parent().unwrap_or(std::path::Path::new("C:\\"));
                    let _ = std::process::Command::new(&obs_exe)
                        .current_dir(working_dir)
                        .spawn();
                }
            }

            // Wait up to 15 seconds for OBS to initialize WebSocket server
            for _ in 0..30 {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                if self.connect().await {
                    connected = true;
                    break;
                }
            }
        }

        if !connected {
            return Err("OBS Studio is not responding on port 4455. Start OBS Studio and ensure the server is enabled in 'Tools' -> 'WebSocket Server Settings'.".to_string());
        }

        // 4. Dynamic Live DLL Injection into running obs64.exe process
        let injection_res = crate::paths::PathResolver::inject_obs_plugin_live();
        let injection_ok = injection_res.is_ok();
        let injection_status = match &injection_res {
            Ok(msg) => format!("DLL Injection: success ({})", msg),
            Err(e) => format!("DLL Injection: ERROR — {}", e),
        };

        // 5. Discover scenes and deploy Censor Shield in ALL scenes
        let (current_scene, scenes) = self.refresh_scenes().await?;
        let mut protected_scenes = Vec::new();
        for sc in &scenes {
            match self.ensure_censor_shield_in_scene(sc).await {
                Ok(id) => protected_scenes.push(format!("{}(id:{})", sc, id)),
                Err(e) => eprintln!("[OBSClient] Shield deploy notice for scene '{}': {}", sc, e),
            }
        }

        // 6. Discover Audio Inputs for mute protection
        let audio_inputs = self.refresh_audio_inputs().await;

        // 7. Auto-attach blewred_filter GPU shader to video/screen capture sources
        let attached_filters = self.attach_censor_filters_to_captures(&scenes).await;

        // 8. Update telemetry and capture status
        self.update_capture_and_video_telemetry().await;

        let res_video = self.video_resolution.lock().await.clone();
        let capture_name = self.capture_source_name.lock().await.clone();

        // Plugin is only usable when the DLL was either injected live or deployed to disk
        // (disk deployment requires an OBS restart to load the plugin).
        let success = install_ok || injection_ok;
        let restart_hint = if !injection_ok && install_ok {
            "\n• NOTE: DLL installed to disk, but not loaded into running OBS. Fully restart OBS Studio for the plugin to activate."
        } else {
            ""
        };
        let failure_hint = if !success {
            "\n• Plugin NOT installed: failed to copy DLL to OBS folders or perform live injection. Check access permissions (run as Administrator) and ensure obs64.exe is running."
        } else {
            ""
        };

        let message = format!(
            "OBS Auto-configuration complete!\n• {}\n• {}{}\n• Scene protection ({}): {}\n• Audio channels ({}): {}\n• GPU filters on sources ({}): {}\n• Video: {} | Active source: '{}'",
            install_status,
            injection_status,
            format_args!("{}{}", restart_hint, failure_hint),
            protected_scenes.len(),
            if protected_scenes.is_empty() { current_scene } else { protected_scenes.join(", ") },
            audio_inputs.len(),
            if audio_inputs.is_empty() { "Mic/Aux, Desktop Audio".to_string() } else { audio_inputs.join(", ") },
            attached_filters.len(),
            if attached_filters.is_empty() { "no capture sources".to_string() } else { attached_filters.join(", ") },
            res_video,
            capture_name
        );

        Ok(ObsSetupReport {
            success,
            install_ok,
            injection_ok,
            message,
        })
    }

    pub async fn mute_input(&self, input_name: &str, duration_ms: u64) {
        let deadline = Instant::now() + Duration::from_millis(duration_ms);
        let input_str = input_name.to_string();

        {
            let mut deadlines = self.mute_deadlines.lock().await;
            deadlines.insert(input_str.clone(), deadline);
        }

        self.send_mute_command(&input_str, true).await;

        let deadlines_clone = self.mute_deadlines.clone();
        let client_clone = self.clone_handle();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(duration_ms)).await;
            let mut dl = deadlines_clone.lock().await;
            if let Some(&scheduled) = dl.get(&input_str) {
                if Instant::now() >= scheduled {
                    dl.remove(&input_str);
                    drop(dl);
                    client_clone.send_mute_command(&input_str, false).await;
                }
            }
        });
    }

    pub async fn send_mute_command(&self, input_name: &str, muted: bool) {
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "SetInputMute",
                "requestId": format!("mute_{}", input_name),
                "requestData": {
                    "inputName": input_name,
                    "inputMuted": muted
                }
            }
        });
        self.send_raw(payload).await;
    }

    pub async fn send_raw(&self, payload: serde_json::Value) {
        let mut tx_guard = self.tx.lock().await;
        if let Some(ref mut sink) = *tx_guard {
            let msg = Message::Text(payload.to_string().into());
            let _ = sink.send(msg).await;
        }
    }

    pub async fn mute_all_audio(&self, muted: bool) {
        let inputs = {
            let list = self.discovered_audio_inputs.lock().await;
            list.clone()
        };
        for input in inputs {
            self.send_mute_command(&input, muted).await;
        }
    }

    pub fn clone_handle(&self) -> Self {
        Self {
            host: self.host.clone(),
            port: self.port.clone(),
            password: self.password.clone(),
            is_connected: self.is_connected.clone(),
            mute_deadlines: self.mute_deadlines.clone(),
            tx: self.tx.clone(),
            pending_requests: self.pending_requests.clone(),
            selected_scene: self.selected_scene.clone(),
            cached_scenes: self.cached_scenes.clone(),
            shield_item_ids: self.shield_item_ids.clone(),
            video_resolution: self.video_resolution.clone(),
            capture_active: self.capture_active.clone(),
            capture_source_name: self.capture_source_name.clone(),
            is_shield_visible: self.is_shield_visible.clone(),
            censor_trigger_seq: self.censor_trigger_seq.clone(),
            discovered_audio_inputs: self.discovered_audio_inputs.clone(),
        }
    }
}
