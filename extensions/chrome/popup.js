const WS_URL = "ws://127.0.0.1:51789";

let ws = null;
let currentMode = null;
let lastModeClickTime = 0;
let lastPlayerSyncData = null;
let currentLang = "en";

const I18N_POPUP = {
  ru: {
    ext_badge_offline: "Оффлайн",
    ext_badge_active: "Активен",
    ext_label_bridge: "Ядро (Порт 51789):",
    ext_core_not_running: "blewred не запущен",
    ext_core_connected: "Подключен (Порт 51789)",
    ext_label_mode: "Режим защиты:",
    ext_mode_detecting: "Определение...",
    ext_mode_disconnected: "Нет связи",
    ext_label_player_det: "Обнаружение плеера:",
    ext_player_detected: "Обнаружен",
    ext_player_not_detected: "Не обнаружен",
    ext_label_player_name: "Название плеера:",
    ext_player_waiting: "Ожидание...",
    ext_label_player_sync: "Текущий статус:",
    ext_status_waiting: "Ожидание плеера...",
    ext_label_models: "Нейросети (GPU):",
    ext_models_waiting: "Ожидание ядра",
    ext_models_active: "NudeNet + ViT активны",
    ext_models_loading: "Загрузка моделей...",
    ext_mode_control: "Управление режимом защиты:",
    ext_btn_mode_1: "Плеер (1)",
    ext_btn_mode_0: "Гибрид (0)",
    ext_btn_mode_2: "Экран (2)",
    ext_btn_mode_3: "Выкл (3)",
    mode_1_badge: "Анализ плеера (1)",
    mode_1_hint: "Экран выключен • CPU < 1%",
    mode_2_badge: "Анализ экрана (2)",
    mode_2_hint: "Игры / Экран • Плеер спит",
    mode_0_badge: "Гибридный (0)",
    mode_0_hint: "Экран + Плеер одновременно",
    mode_3_badge: "Защита выключена (3)",
    mode_3_hint: "Все режимы на паузе (Standby)",
    ext_btn_lookahead_window: "Окно упреждающего видео",
    state_playing: "Играет",
    state_paused: "Пауза"
  },
  en: {
    ext_badge_offline: "Offline",
    ext_badge_active: "Active",
    ext_label_bridge: "Core (Port 51789):",
    ext_core_not_running: "blewred not running",
    ext_core_connected: "Connected (Port 51789)",
    ext_label_mode: "Protection Mode:",
    ext_mode_detecting: "Detecting...",
    ext_mode_disconnected: "Disconnected",
    ext_label_player_det: "Player Detection:",
    ext_player_detected: "Detected",
    ext_player_not_detected: "Not Detected",
    ext_label_player_name: "Player Name:",
    ext_player_waiting: "Waiting...",
    ext_label_player_sync: "Current Status:",
    ext_status_waiting: "Waiting for player...",
    ext_label_models: "Neural Models (GPU):",
    ext_models_waiting: "Waiting for core",
    ext_models_active: "NudeNet + ViT Active",
    ext_models_loading: "Downloading models...",
    ext_mode_control: "Protection Mode Control:",
    ext_btn_mode_1: "Player (1)",
    ext_btn_mode_0: "Hybrid (0)",
    ext_btn_mode_2: "Screen (2)",
    ext_btn_mode_3: "Off (3)",
    mode_1_badge: "Player Analysis (1)",
    mode_1_hint: "Screen capture paused • CPU < 1%",
    mode_2_badge: "Screen Analysis (2)",
    mode_2_hint: "Games / Display • Player idle",
    mode_0_badge: "Hybrid (0)",
    mode_0_hint: "Screen + Player simultaneously",
    mode_3_badge: "Protection Off (3)",
    mode_3_hint: "All pipelines paused (Standby)",
    ext_btn_lookahead_window: "Lookahead Video Window",
    state_playing: "Playing",
    state_paused: "Paused"
  }
};

const bridgeBadge = document.getElementById("bridge-badge");
const bridgeStatusText = document.getElementById("bridge-status-text");
const currentModeBadge = document.getElementById("current-mode-badge");
const playerDetectedBadge = document.getElementById("player-detected-badge");
const playerNameText = document.getElementById("player-name-text");
const playerSyncStatusBadge = document.getElementById("player-sync-status-badge");
const modelsStatusBadge = document.getElementById("models-status-badge");
const modeHintText = document.getElementById("mode-hint-text");

const btnMode1 = document.getElementById("btn-mode-1");
const btnMode0 = document.getElementById("btn-mode-0");
const btnMode2 = document.getElementById("btn-mode-2");
const btnMode3 = document.getElementById("btn-mode-3");

const btnLangRu = document.getElementById("ext-lang-ru");
const btnLangEn = document.getElementById("ext-lang-en");

function t(key) {
  const dict = I18N_POPUP[currentLang] || I18N_POPUP.ru;
  return dict[key] || key;
}

function setLanguage(lang) {
  currentLang = (lang === "en") ? "en" : "ru";
  try {
    if (typeof chrome !== "undefined" && chrome.storage && chrome.storage.local) {
      chrome.storage.local.set({ blewred_ext_lang: currentLang });
    }
  } catch (e) {}
  try {
    localStorage.setItem("blewred_ext_lang", currentLang);
  } catch (e) {}

  if (btnLangRu) btnLangRu.className = `ext-lang-btn ${currentLang === "ru" ? "active" : ""}`;
  if (btnLangEn) btnLangEn.className = `ext-lang-btn ${currentLang === "en" ? "active" : ""}`;

  // Apply to all elements with data-i18n
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n");
    if (key && I18N_POPUP[currentLang] && I18N_POPUP[currentLang][key]) {
      el.textContent = I18N_POPUP[currentLang][key];
    }
  });

  // Re-apply dynamic states
  if (currentMode !== null) {
    applyModeUI(currentMode);
  } else if (!ws || ws.readyState !== WebSocket.OPEN) {
    setOfflineState();
  }

  if (lastPlayerSyncData) {
    updatePlayerSyncInPopup(lastPlayerSyncData);
  }
}

function initLanguage() {
  const applySaved = (saved) => {
    if (saved === "en" || saved === "ru") {
      setLanguage(saved);
    } else {
      setLanguage("en");
    }
  };

  try {
    if (typeof chrome !== "undefined" && chrome.storage && chrome.storage.local) {
      chrome.storage.local.get(["blewred_ext_lang"], (res) => {
        applySaved(res && res.blewred_ext_lang);
      });
      return;
    }
  } catch (e) {}

  try {
    applySaved(localStorage.getItem("blewred_ext_lang"));
  } catch (e) {
    setLanguage("en");
  }
}

if (btnLangRu) btnLangRu.addEventListener("click", () => setLanguage("ru"));
if (btnLangEn) btnLangEn.addEventListener("click", () => setLanguage("en"));

function formatPlayerTime(seconds) {
  const total = Math.round(seconds || 0);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return (h > 0 ? `${String(h).padStart(2, "0")}:` : "") +
    `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function updatePlayerSyncInPopup(sync) {
  lastPlayerSyncData = sync;
  if (!sync) return;
  const isDetected = !!sync.detected && sync.name && sync.name !== "Неизвестно" && sync.name !== "Unknown" && sync.name !== "Web Player" && sync.name !== "Ожидание плеера..." && sync.name !== "Waiting for player...";
  const name = isDetected ? sync.name : t("ext_player_not_detected");
  const isPlay = !!sync.is_playing;
  const timeSec = sync.time || 0;
  const timeFormatted = formatPlayerTime(timeSec);

  if (playerDetectedBadge) {
    playerDetectedBadge.className = isDetected ? "label label-success" : "label label-default";
    playerDetectedBadge.textContent = isDetected ? t("ext_player_detected") : t("ext_player_not_detected");
  }

  if (playerNameText) {
    playerNameText.textContent = name;
    playerNameText.style.color = isDetected ? "#38bdf8" : "#94a3b8";
  }

  if (playerSyncStatusBadge) {
    if (isDetected) {
      const stateStr = isPlay ? t("state_playing") : t("state_paused");
      const statusText = `${timeFormatted} / ${stateStr}`;
      playerSyncStatusBadge.textContent = statusText;
      playerSyncStatusBadge.style.borderColor = isPlay ? "#10b981" : "#3b4252";
      playerSyncStatusBadge.style.color = isPlay ? "#34d399" : "#38bdf8";
    } else {
      playerSyncStatusBadge.textContent = t("ext_status_waiting");
      playerSyncStatusBadge.style.borderColor = "#2b313c";
      playerSyncStatusBadge.style.color = "#94a3b8";
    }
  }
}

function applyModeUI(mode) {
  currentMode = Number(mode);

  // Reset button styles
  [btnMode1, btnMode0, btnMode2, btnMode3].forEach((btn) => {
    if (btn) {
      btn.className = "btn btn-default";
    }
  });

  if (mode === 1) {
    if (currentModeBadge) {
      currentModeBadge.className = "label label-success";
      currentModeBadge.textContent = t("mode_1_badge");
    }
    if (btnMode1) {
      btnMode1.className = "btn btn-success active-mode";
    }
    if (modeHintText) {
      modeHintText.textContent = t("mode_1_hint");
    }
  } else if (mode === 2) {
    if (currentModeBadge) {
      currentModeBadge.className = "label label-warning";
      currentModeBadge.textContent = t("mode_2_badge");
    }
    if (btnMode2) {
      btnMode2.className = "btn btn-warning active-mode";
    }
    if (modeHintText) {
      modeHintText.textContent = t("mode_2_hint");
    }
  } else if (mode === 0) {
    if (currentModeBadge) {
      currentModeBadge.className = "label label-info";
      currentModeBadge.textContent = t("mode_0_badge");
    }
    if (btnMode0) {
      btnMode0.className = "btn btn-primary active-mode";
    }
    if (modeHintText) {
      modeHintText.textContent = t("mode_0_hint");
    }
  } else {
    // Mode 3: Standby / Disabled
    if (currentModeBadge) {
      currentModeBadge.className = "label label-default";
      currentModeBadge.textContent = t("mode_3_badge");
    }
    if (btnMode3) {
      btnMode3.className = "btn btn-danger active-mode";
    }
    if (modeHintText) {
      modeHintText.textContent = t("mode_3_hint");
    }
  }
}

function connectBridge() {
  try {
    ws = new WebSocket(WS_URL);

    ws.onopen = () => {
      if (bridgeBadge) {
        bridgeBadge.className = "label label-success";
        bridgeBadge.textContent = t("ext_badge_active");
      }
      if (bridgeStatusText) {
        bridgeStatusText.textContent = t("ext_core_connected");
        bridgeStatusText.style.color = "#10b981";
      }

      ws.send(JSON.stringify({
        type: "register",
        client: "extension_popup"
      }));

      ws.send(JSON.stringify({
        type: "get_operation_mode"
      }));
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);

        if (data.type === "init_state") {
          if (data.operation_mode !== undefined) {
            applyModeUI(data.operation_mode);
          }
          if (data.player_sync) {
            updatePlayerSyncInPopup(data.player_sync);
          }
          if (data.nsfw_model_ready !== undefined && modelsStatusBadge) {
            if (data.nsfw_model_ready) {
              modelsStatusBadge.className = "label label-success";
              modelsStatusBadge.textContent = t("ext_models_active");
            } else {
              modelsStatusBadge.className = "label label-warning";
              modelsStatusBadge.textContent = t("ext_models_loading");
            }
          }
        } else if (data.type === "telemetry") {
          if (data.player_sync) {
            updatePlayerSyncInPopup(data.player_sync);
          }
          if (data.operation_mode !== undefined && currentMode === null) {
            applyModeUI(data.operation_mode);
          }
        } else if (data.type === "operation_mode_changed") {
          if (data.operation_mode !== undefined) {
            applyModeUI(data.operation_mode);
          }
        }
      } catch (e) {}
    };

    ws.onerror = () => {
      setOfflineState();
    };

    ws.onclose = () => {
      setOfflineState();
      setTimeout(connectBridge, 2500);
    };
  } catch (e) {
    setOfflineState();
  }
}

function setOfflineState() {
  if (bridgeBadge) {
    bridgeBadge.className = "label label-danger";
    bridgeBadge.textContent = t("ext_badge_offline");
  }
  if (bridgeStatusText) {
    bridgeStatusText.textContent = t("ext_core_not_running");
    bridgeStatusText.style.color = "#ff6b6b";
  }
  if (currentModeBadge) {
    currentModeBadge.className = "label label-default";
    currentModeBadge.textContent = t("ext_mode_disconnected");
  }
  if (modelsStatusBadge) {
    modelsStatusBadge.className = "label label-default";
    modelsStatusBadge.textContent = t("ext_models_waiting");
  }
  if (modeHintText) {
    modeHintText.textContent = "";
  }
}

function switchMode(targetMode) {
  const now = Date.now();
  if (now - lastModeClickTime < 120) return; // Debounce
  lastModeClickTime = now;

  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({
      type: "set_operation_mode",
      mode: targetMode
    }));
    applyModeUI(targetMode);
  }
}

function handleModeSwitch(targetMode) {
  // If clicking currently active mode, toggle it off into mode 3 (Standby)
  if (currentMode === targetMode) {
    switchMode(3);
  } else {
    switchMode(targetMode);
  }
}

if (btnMode1) btnMode1.addEventListener("click", () => handleModeSwitch(1));
if (btnMode0) btnMode0.addEventListener("click", () => handleModeSwitch(0));
if (btnMode2) btnMode2.addEventListener("click", () => handleModeSwitch(2));
if (btnMode3) btnMode3.addEventListener("click", () => switchMode(3));

const btnOpenPreview = document.getElementById("btn-open-preview-window");
if (btnOpenPreview) {
  btnOpenPreview.addEventListener("click", () => {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "open_lookahead_preview" }));
    }
  });
}

// Direct active tab query on popup open
try {
  if (typeof chrome !== "undefined" && chrome.tabs && chrome.tabs.query) {
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      if (tabs && tabs[0] && tabs[0].id) {
        chrome.tabs.sendMessage(tabs[0].id, { type: "get_player_status" }, (res) => {
          if (chrome.runtime.lastError || !res || !res.detected) return;
          updatePlayerSyncInPopup(res);
        });
      }
    });
  }
} catch (e) {}

initLanguage();
connectBridge();
