/**
 * blewred Stream Compliance Control Center
 * Real-time WebSocket connection to local engine daemon (port 51789)
 * Strict Video / Screen OCR compliance & OBS Studio v5 Shield integration.
 */

// ==============================================================================
// TOP-LEVEL APPLICATION STATE & PREFERENCES (Eliminate TDZ)
// ==============================================================================
let currentLanguage = (typeof window !== "undefined" && window.I18N && window.I18N.getLanguage())
  || (typeof localStorage !== "undefined" && localStorage.getItem("blewred_lang"))
  || "en";
let isObsConnected = false;
let isOcrEnabled = true;
let lastSetupGuideData = null;

const WS_URL = "ws://127.0.0.1:51789";
let socket = null;
let isBoosted = false;
let cachedSceneJson = "";

/**
 * Dispatch message over daemon WebSocket connection with safe error boundary.
 * Supports passing either an action type string with optional payload data,
 * or a fully formed message object.
 */
function sendWs(typeOrPayload, optionalData) {
  if (!socket || socket.readyState !== WebSocket.OPEN) return false;
  try {
    const payload = typeof typeOrPayload === "string"
      ? { type: typeOrPayload, ...optionalData }
      : typeOrPayload;
    socket.send(JSON.stringify(payload));
    return true;
  } catch (err) {
    console.error("[blewred WS] Message dispatch failed:", err);
    return false;
  }
}

// Safe DOM text setter
function setSafeText(el, val) {
  if (el && el.textContent !== val) el.textContent = val;
}

// DOM Elements - Status Pills
const obsPill = document.getElementById("obs-pill");
const obsStatusText = document.getElementById("obs-status-text");
const obsPluginPill = document.getElementById("obs-plugin-pill");
const obsPluginDot = document.getElementById("obs-plugin-dot");
const obsPluginStatusText = document.getElementById("obs-plugin-status-text");
const screenPill = document.getElementById("screen-pill");
const screenStatusText = document.getElementById("screen-status-text");
const shieldPill = document.getElementById("shield-pill");
const shieldPillStatus = document.getElementById("shield-pill-status");
const extensionPill = document.getElementById("extension-pill");
const extensionDot = document.getElementById("extension-dot");
const extensionStatusText = document.getElementById("extension-status-text");
const fpsBadge = document.getElementById("fps-badge");
const visionModeText = document.getElementById("vision-mode-text");

// Unified Status Ribbon Pill Setter: strictly white (#ffffff) when ON, gray (#94a3b8) when OFF
function setPillStatus(textEl, dotEl, isOn) {
  if (textEl) {
    textEl.textContent = isOn ? "ON" : "OFF";
    textEl.style.color = isOn ? "#ffffff" : "#94a3b8";
    if (isOn) {
      textEl.classList.remove("off");
      textEl.classList.add("on");
    } else {
      textEl.classList.remove("on");
      textEl.classList.add("off");
    }
  }
  if (dotEl) {
    dotEl.className = isOn ? "status-dot active" : "status-dot";
  }
}

function updateObsStatus(connected) {
  isObsConnected = !!connected;
  const isConnected = !!connected;
  const dot = obsPill ? obsPill.querySelector(".status-dot") : null;
  setPillStatus(obsStatusText, dot, isConnected);
  if (shieldPill) {
    const shieldDot = shieldPill.querySelector(".status-dot");
    const isShieldOn = isConnected && (lastShieldState ? !!lastShieldState.enabled : true);
    setPillStatus(shieldPillStatus, shieldDot, isShieldOn);
  }
}

function updateObsPluginStatusUI(obsPluginActive, obsConnected) {
  if (!obsPluginPill || !obsPluginStatusText) return;
  const isActive = !!obsPluginActive;
  if (isActive) {
    obsPluginPill.className = "status-pill active-mode";
  } else {
    obsPluginPill.className = "status-pill";
  }
  setPillStatus(obsPluginStatusText, obsPluginDot, isActive);
}

function updateExtensionStatusUI(connected) {
  const isConnected = !!connected;
  setPillStatus(extensionStatusText, extensionDot, isConnected);
}

// Censor Test Result Elements
const censorResultBanner = document.getElementById("censor-result-banner");
const censorStatusBadge = document.getElementById("censor-status-badge");
const badgeStatusText = document.getElementById("badge-status-text");
const censorScoreText = document.getElementById("censor-score-text");
const censorDetailText = document.getElementById("censor-detail-text");
const ocrTestInput = document.getElementById("ocr-test-input");

// OBS Auto-Setup Prompt Modal Elements
const modalObsSetupPrompt = document.getElementById("modal-obs-setup-prompt");
const modalObsSetupPromptBackdrop = document.getElementById("modal-obs-setup-prompt-backdrop");
const btnCloseObsSetupPromptX = document.getElementById("btn-close-obs-setup-prompt-x");
const btnCancelObsSetupPrompt = document.getElementById("btn-cancel-obs-setup-prompt");
const btnConfirmObsSetupContinue = document.getElementById("btn-confirm-obs-setup-continue");
const btnModalLaunchObs = document.getElementById("btn-modal-launch-obs");
const obsSetupPromptStatusBadge = document.getElementById("obs-setup-prompt-status-badge");
const obsSetupPromptHint = document.getElementById("obs-setup-prompt-hint");
const obsSetupResultAlert = document.getElementById("obs-setup-result-alert");
const obsSetupResultMsg = document.getElementById("obs-setup-result-msg");

// Hotkeys Modal Elements
const modalHotkeys = document.getElementById("modal-hotkeys");
const modalHotkeysBackdrop = document.getElementById("modal-hotkeys-backdrop");
const btnOpenHotkeysModal = document.getElementById("btn-open-hotkeys-modal");
const btnCloseHotkeys = document.getElementById("btn-close-hotkeys");
const btnCloseHotkeysX = document.getElementById("btn-close-hotkeys-x");

// DOM Elements - Card 1 Video Telemetry & Scene Control
const monitorSelect = document.getElementById("monitor-select");
const monitorActiveBadge = document.getElementById("monitor-active-badge");
const obsSceneSelect = document.getElementById("obs-scene-select");
const btnRefreshScenes = document.getElementById("btn-refresh-scenes");
const videoModeTag = document.getElementById("video-mode-tag");
const screenStatVal = document.getElementById("screen-stat-val");
const obsCanvasStat = document.getElementById("obs-canvas-stat");
const obsShieldStat = document.getElementById("obs-shield-stat");
const fpsStatVal = document.getElementById("fps-stat-val");
const btnAutoSetupObs = document.getElementById("btn-auto-setup-obs") || document.getElementById("btn-trigger-obs-setup");
const gpuStatVal = document.getElementById("gpu-stat-val");
// Live event journal container (element may be absent in some views — fall back to console)
const eventFeed = document.getElementById("event-feed");
let cachedMonitorsJson = "";
let cachedHudMonitorsJson = "";

// Real-time Guard & Sensitivity Slider Elements
const toggleRealtimeGuard = document.getElementById("toggle-realtime-guard");
const radarPulse = document.getElementById("radar-pulse");
const nsfwThresholdSlider = document.getElementById("nsfw-threshold-slider");
const thresholdValBadge = document.getElementById("threshold-val-badge");
const sensitivityDescText = document.getElementById("sensitivity-desc-text");
const toggleCensorShield = document.getElementById("toggle-censor-shield");
const toggleShieldDonate = document.getElementById("toggle-shield-donate");
const modalDisableSupportConfirm = document.getElementById("modal-disable-support-confirm");
const modalDisableSupportBackdrop = document.getElementById("modal-disable-support-backdrop");
const btnConfirmDisableSupport = document.getElementById("btn-confirm-disable-support");
const btnCancelDisableSupport = document.getElementById("btn-cancel-disable-support");
const btnCloseDisableSupportX = document.getElementById("btn-close-disable-support-x");

// Scheduled Cues & Guide Elements (hoisted to prevent TDZ)
const toggleCueAutocensor = document.getElementById("toggle-cue-autocensor");
const labelCueAutocensorStatus = document.getElementById("label-cue-autocensor-status");
const toggleCueNotifications = document.getElementById("toggle-cue-notifications");
const labelCueNotificationsStatus = document.getElementById("label-cue-notifications-status");
const cuePrewarnSecondsInput = document.getElementById("cue-prewarn-seconds-input");
const chkHideSetupGuide = document.getElementById("chk-hide-setup-guide");
const streamerSetupGuide = document.getElementById("streamer-setup-guide");
const btnTriggerObsSetup = document.getElementById("btn-trigger-obs-setup");
const stepCardObs = document.getElementById("step-card-obs");
const badgeStepObs = document.getElementById("badge-step-obs");
const cuesCountBadge = document.getElementById("cues-count-badge");

// Streamer Lookahead HUD Elements
const lookaheadCard = document.getElementById("lookahead-card");
const lookaheadSeverityBadge = document.getElementById("lookahead-severity-badge");
const lookaheadPlayerBadge = document.getElementById("lookahead-player-badge");
const lookaheadPreviewImg = document.getElementById("lookahead-preview-img");
const lookaheadBoxesCanvas = document.getElementById("lookahead-boxes-canvas");
const lookaheadReasonTitle = document.getElementById("lookahead-reason-title");
const lookaheadEtaClock = document.getElementById("lookahead-eta-clock");
const lookaheadProgressFill = document.getElementById("lookahead-progress-fill");
const btnLookaheadBlock = document.getElementById("btn-lookahead-block");
const btnLookaheadAllow = document.getElementById("btn-lookahead-allow");
const hudMonitorSelect = document.getElementById("hud-monitor-select");
const btnTestLookaheadHud = document.getElementById("btn-test-lookahead-hud");
const btnOpenLookaheadWindow = document.getElementById("btn-open-lookahead-window");
const btnToggleLookaheadPreview = document.getElementById("btn-toggle-lookahead-preview");
const btnLookaheadDetach = document.getElementById("btn-lookahead-detach");

// Operation Mode Elements (0 = Hybrid, 1 = PlayerOnly, 2 = ScreenOnly, 3 = Standby/Disabled)
const modeBtnPlayer = document.getElementById("mode-btn-player");
const modeBtnScreen = document.getElementById("mode-btn-screen");
const modeBtnHybrid = document.getElementById("mode-btn-hybrid");
const modeBtnOff = document.getElementById("mode-btn-off");
const modeEfficiencyBadge = document.getElementById("mode-efficiency-badge");
const guardActiveBadge = document.getElementById("guard-active-badge");
let currentOperationMode = 0;

function updateOperationModeUI(mode) {
  currentOperationMode = mode;
  const buttons = [
    { el: modeBtnPlayer, id: 1 },
    { el: modeBtnScreen, id: 2 },
    { el: modeBtnHybrid, id: 0 },
    { el: modeBtnOff, id: 3 },
  ];

  buttons.forEach(({ el, id }) => {
    if (el) {
      if (id === mode) {
        el.classList.add("active");
      } else {
        el.classList.remove("active");
      }
    }
  });

  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  if (guardActiveBadge) {
    if (mode === 3) {
      guardActiveBadge.className = "label label-default guard-active-badge guard-inactive";
      guardActiveBadge.innerHTML = `<span class="pulsing-radar-dot gray"></span> ${isEn ? "PROTECTION DISABLED" : "ЗАЩИТА ВЫКЛ"}`;
    } else {
      guardActiveBadge.className = "label label-success guard-active-badge";
      guardActiveBadge.innerHTML = `<span class="pulsing-radar-dot"></span> ${isEn ? "PROTECTION ACTIVE" : "ЗАЩИТА АКТИВНА"}`;
    }
  }

  if (modeEfficiencyBadge) {
    if (mode === 1) {
      modeEfficiencyBadge.textContent = isEn ? "CPU < 1% • Screen paused" : "CPU < 1% • Экран на паузе";
      modeEfficiencyBadge.style.color = "#38bdf8";
      modeEfficiencyBadge.style.background = "rgba(0, 180, 216, 0.14)";
      modeEfficiencyBadge.style.borderColor = "rgba(0, 180, 216, 0.35)";
    } else if (mode === 2) {
      modeEfficiencyBadge.textContent = isOcrEnabled
        ? (isEn ? "Screen active • OCR enabled" : "Экран активен • OCR включен")
        : (isEn ? "Screen active • OCR disabled" : "Экран активен • OCR отключен");
      modeEfficiencyBadge.style.color = "#4ade80";
      modeEfficiencyBadge.style.background = "rgba(0, 230, 118, 0.14)";
      modeEfficiencyBadge.style.borderColor = "rgba(0, 230, 118, 0.35)";
    } else if (mode === 3) {
      modeEfficiencyBadge.textContent = isEn ? "Protection disabled • Standby mode" : "Защита отключена • Режим ожидания (Standby)";
      modeEfficiencyBadge.style.color = "#ef4444";
      modeEfficiencyBadge.style.background = "rgba(239, 68, 68, 0.14)";
      modeEfficiencyBadge.style.borderColor = "rgba(239, 68, 68, 0.35)";
    } else {
      modeEfficiencyBadge.textContent = isOcrEnabled
        ? (isEn ? "Balanced mode" : "Сбалансированный режим")
        : (isEn ? "Balanced (OCR off)" : "Сбалансированный (OCR выкл)");
      modeEfficiencyBadge.style.color = "#fbbf24";
      modeEfficiencyBadge.style.background = "rgba(255, 183, 3, 0.14)";
      modeEfficiencyBadge.style.borderColor = "rgba(255, 183, 3, 0.35)";
    }
  }

  try {
    localStorage.setItem("blewred_operation_mode", mode.toString());
  } catch (e) { }

  applyRealtimeFpsState({
    opMode: mode,
    operation_mode: mode,
    analysis_paused: mode === 1 || mode >= 3,
    real_fps: (mode === 1 || mode >= 3) ? 0 : currentRealFps
  });
}

let lastUserModeChangeTime = 0;
let lastModeClickTimestamp = 0;

function setOperationMode(mode) {
  mode = Number(mode);
  if (isNaN(mode) || mode < 0 || mode > 3) mode = 0;
  lastUserModeChangeTime = Date.now();
  updateOperationModeUI(mode);
  const _isEnMode = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  renderEventItem({
    type: mode === 3 ? "warning" : "system",
    timestamp: new Date().toTimeString().split(" ")[0],
    message: _isEnMode
      ? (mode === 1
        ? "[Mode] PLAYER ANALYSIS: Screen capture disabled, CPU < 1%. Extension video analysis active."
        : (mode === 2
          ? `[Mode] SCREEN ANALYSIS: Continuous desktop monitoring${isOcrEnabled ? " and OCR active" : " (OCR disabled)"}.`
          : (mode === 3
            ? "[Mode] PROTECTION OFF (Standby): Monitoring paused, active locks cleared."
            : `[Mode] HYBRID MODE: Screen and player protected${isOcrEnabled ? "" : " (OCR disabled)"}.`)))
      : (mode === 1
        ? "[Режим работы] АНАЛИЗ ПЛЕЕРА: Захват экрана отключен, CPU < 1%. Анализ видео через расширение активен."
        : (mode === 2
          ? `[Режим работы] АНАЛИЗ ЭКРАНА: Непрерывный мониторинг рабочего стола${isOcrEnabled ? " и OCR активны" : " (OCR отключен)"}.`
          : (mode === 3
            ? "[Режим работы] ЗАЩИТА ОТКЛЮЧЕНА (Standby): Мониторинг приостановлен, активные блокировки сняты."
            : `[Режим работы] ГИБРИДНЫЙ РЕЖИМ: Экран и плеер под защитой${isOcrEnabled ? "" : " (OCR отключен)"}.`)))
  });

  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: "set_operation_mode", mode: mode }));
  }
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("set_operation_mode", { mode: mode }).catch(() => { });
  }
}

function handleModeButtonClick(targetMode) {
  const now = Date.now();
  if (now - lastModeClickTimestamp < 120) {
    return;
  }
  lastModeClickTimestamp = now;

  if (currentOperationMode === targetMode) {
    // Clicking the active mode toggles it off into Mode 3 (Standby)
    // Clicking Mode 3 while off returns to default Hybrid (Mode 0)
    setOperationMode(targetMode === 3 ? 0 : 3);
  } else {
    setOperationMode(targetMode);
  }
}

if (modeBtnPlayer) modeBtnPlayer.addEventListener("click", () => handleModeButtonClick(1));
if (modeBtnScreen) modeBtnScreen.addEventListener("click", () => handleModeButtonClick(2));
if (modeBtnHybrid) modeBtnHybrid.addEventListener("click", () => handleModeButtonClick(0));
if (modeBtnOff) modeBtnOff.addEventListener("click", () => handleModeButtonClick(3));

// OCR Detection State and Controls
const toggleOcrEnabled = document.getElementById("toggle-ocr-enabled");
const footerOcrStackText = document.getElementById("footer-ocr-stack-text");
// isOcrEnabled hoisted to top
let lastUserOcrChangeTime = 0;

function updateOcrStateUI(enabled) {
  isOcrEnabled = !!enabled;
  if (toggleOcrEnabled && toggleOcrEnabled.checked !== isOcrEnabled) {
    toggleOcrEnabled.checked = isOcrEnabled;
  }
  try {
    localStorage.setItem("blewred_ocr_enabled", isOcrEnabled ? "true" : "false");
  } catch (e) { }

  if (liveOcrCard) {
    if (!isOcrEnabled) {
      liveOcrCard.classList.add("ocr-disabled");
    } else {
      liveOcrCard.classList.remove("ocr-disabled");
    }
  }

  if (liveOcrRadarDot) {
    if (!isOcrEnabled) {
      liveOcrRadarDot.className = "pulse-live-dot gray";
    } else {
      liveOcrRadarDot.className = "pulse-live-dot blue";
    }
  }

  const _isEnOcr = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (liveOcrStatusPill) {
    if (!isOcrEnabled) {
      liveOcrStatusPill.className = "live-status-pill disabled";
      liveOcrStatusPill.textContent = _isEnOcr ? "DISABLED" : "ОТКЛЮЧЕН";
    } else {
      liveOcrStatusPill.className = "live-status-pill safe";
      liveOcrStatusPill.textContent = _isEnOcr ? "TEXT CLEAN" : "ТЕКСТ ЧИСТ";
    }
  }

  if (liveOcrPreviewText && !isOcrEnabled) {
    liveOcrPreviewText.textContent = _isEnOcr ? "(Screen text not scanned)" : "(Текст на экране не сканируется)";
  }

  if (liveOcrWordsWrap && !isOcrEnabled) {
    liveOcrWordsWrap.innerHTML = '<span class="badge-none">—</span>';
  }

  if (liveOcrTime && !isOcrEnabled) {
    liveOcrTime.textContent = "";
  }

  if (footerOcrStackText) {
    footerOcrStackText.textContent = isOcrEnabled ? "WinRT OCR (Windows)" : (_isEnOcr ? "WinRT OCR (Off)" : "WinRT OCR (Выкл)");
  }

  if (modeEfficiencyBadge && currentOperationMode === 2) {
    modeEfficiencyBadge.textContent = isOcrEnabled
      ? (_isEnOcr ? "Screen active • OCR enabled" : "Экран активен • OCR включен")
      : (_isEnOcr ? "Screen active • OCR disabled" : "Экран активен • OCR отключен");
  }
}

function setOcrEnabled(enabled) {
  lastUserOcrChangeTime = Date.now();
  updateOcrStateUI(enabled);
  const _isEnSetOcr = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  renderEventItem({
    type: enabled ? "success" : "warning",
    timestamp: new Date().toTimeString().split(" ")[0],
    message: _isEnSetOcr
      ? `[OCR Detector] Text and stopword recognition: ${enabled ? "ENABLED" : "DISABLED"}`
      : `[OCR Детектор] Распознавание текста и стоп-слов: ${enabled ? "ВКЛЮЧЕНО" : "ОТКЛЮЧЕНО"}`
  });

  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "toggle_ocr",
      enabled: enabled
    }));
  }
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("toggle_ocr", { enabled: enabled }).catch(() => { });
  }
}

if (toggleOcrEnabled) {
  toggleOcrEnabled.addEventListener("change", (e) => {
    setOcrEnabled(e.target.checked);
  });
}

let activeLookaheadTicket = null;
let lookaheadCountdownInterval = null;

// Audio warning chime (universfield-warning-alert-132471.mp3)
const warningAlertAudio = new Audio("universfield-warning-alert-132471.mp3");
warningAlertAudio.volume = 0.8;

function playLookaheadChime() {
  try {
    warningAlertAudio.currentTime = 0;
    const playPromise = warningAlertAudio.play();
    if (playPromise !== undefined) {
      playPromise.catch(() => playLookaheadChimeFallback());
    }
  } catch (e) {
    playLookaheadChimeFallback();
  }
}

function playLookaheadChimeFallback() {
  try {
    const AudioCtx = window.AudioContext || window.webkitAudioContext;
    if (!AudioCtx) return;
    const ctx = new AudioCtx();
    const now = ctx.currentTime;

    const osc1 = ctx.createOscillator();
    const gain1 = ctx.createGain();
    osc1.type = "sine";
    osc1.frequency.setValueAtTime(587.33, now);
    osc1.frequency.exponentialRampToValueAtTime(880.0, now + 0.15);
    gain1.gain.setValueAtTime(0.25, now);
    gain1.gain.exponentialRampToValueAtTime(0.001, now + 0.35);

    osc1.connect(gain1);
    gain1.connect(ctx.destination);
    osc1.start(now);
    osc1.stop(now + 0.35);

    const osc2 = ctx.createOscillator();
    const gain2 = ctx.createGain();
    osc2.type = "sine";
    osc2.frequency.setValueAtTime(880.0, now + 0.18);
    osc2.frequency.exponentialRampToValueAtTime(1174.66, now + 0.38);
    gain2.gain.setValueAtTime(0.2, now + 0.18);
    gain2.gain.exponentialRampToValueAtTime(0.001, now + 0.55);

    osc2.connect(gain2);
    gain2.connect(ctx.destination);
    osc2.start(now + 0.18);
    osc2.stop(now + 0.55);
  } catch (e) { }
}

function drawLookaheadBoxes(boxes) {
  if (!lookaheadBoxesCanvas) return;
  const ctx = lookaheadBoxesCanvas.getContext("2d");
  ctx.clearRect(0, 0, lookaheadBoxesCanvas.width, lookaheadBoxesCanvas.height);
  if (!boxes || !Array.isArray(boxes)) return;

  const w = lookaheadBoxesCanvas.width;
  const h = lookaheadBoxesCanvas.height;

  boxes.forEach(b => {
    const x1 = (b.x1 !== undefined ? b.x1 : (b.x || 0)) * w;
    const y1 = (b.y1 !== undefined ? b.y1 : (b.y || 0)) * h;
    const bw = (b.x2 !== undefined ? (b.x2 - b.x1) : (b.width || 0.2)) * w;
    const bh = (b.y2 !== undefined ? (b.y2 - b.y1) : (b.height || 0.2)) * h;

    ctx.strokeStyle = "#ffe600";
    ctx.lineWidth = 3;
    ctx.shadowColor = "rgba(0,0,0,0.9)";
    ctx.shadowBlur = 4;
    ctx.strokeRect(x1, y1, bw, bh);

    ctx.fillStyle = "rgba(255, 230, 0, 0.18)";
    ctx.fillRect(x1, y1, bw, bh);
  });
}

function renderLookaheadTicket(ticket) {
  if (!ticket || !lookaheadCard) return;
  activeLookaheadTicket = ticket;
  playLookaheadChime();

  lookaheadCard.style.display = "block";

  const _isEnLh = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (lookaheadPlayerBadge) {
    lookaheadPlayerBadge.textContent = ticket.player_type || (_isEnLh ? "Alloha Player" : "Плеер Alloha");
  }

  const isObvious = (ticket.severity === "high") || (ticket.score >= 0.70);
  if (lookaheadSeverityBadge) {
    if (isObvious) {
      lookaheadSeverityBadge.className = "label label-danger";
      lookaheadSeverityBadge.textContent = _isEnLh ? "OBVIOUS NSFW" : "ОЧЕВИДНЫЙ NSFW";
    } else {
      lookaheadSeverityBadge.className = "label label-warning";
      lookaheadSeverityBadge.textContent = _isEnLh ? "SUSPICIOUS CONTENT (SUBTLE)" : "ПОДОЗРИТЕЛЬНЫЙ КОНТЕНТ (НЕОЧЕВИДНЫЙ)";
    }
  }

  if (lookaheadReasonTitle) {
    const pct = Math.round((ticket.score || 0.8) * 100);
    lookaheadReasonTitle.textContent = `${ticket.reason || (_isEnLh ? "Inappropriate Content" : "Непристойный контент")} (${pct}%)`;
  }

  if (lookaheadPreviewImg) {
    if (ticket.raw_image_base64 && ticket.raw_image_base64.length > 50) {
      const prefix = ticket.raw_image_base64.startsWith("data:") ? "" : "data:image/jpeg;base64,";
      lookaheadPreviewImg.src = prefix + ticket.raw_image_base64;
    } else {
      lookaheadPreviewImg.src = "logo.png";
    }
    lookaheadPreviewImg.onload = () => {
      if (ticket && ticket.boxes) {
        drawLookaheadBoxes(ticket.boxes);
      }
    };
  }

  drawLookaheadBoxes(ticket.boxes);
  startLookaheadCountdown(ticket);
}

function startLookaheadCountdown(ticket) {
  if (lookaheadCountdownInterval) clearInterval(lookaheadCountdownInterval);

  const initialEta = Math.max(ticket.eta_seconds || 8.0, 1.0);
  const targetTimeMs = ticket.execute_at_ms || (Date.now() + initialEta * 1000);

  function tick() {
    const remainingMs = targetTimeMs - Date.now();
    const remSec = Math.max(0, remainingMs / 1000);

    if (lookaheadEtaClock) {
      const _isEnClock = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
      lookaheadEtaClock.textContent = (remSec < 10 ? "0" : "") + remSec.toFixed(1) + (_isEnClock ? " s" : " сек");
    }
    if (lookaheadProgressFill) {
      const pct = Math.min(100, Math.max(0, (remSec / initialEta) * 100));
      lookaheadProgressFill.style.width = pct + "%";
    }

    if (remSec <= 0) {
      clearInterval(lookaheadCountdownInterval);
      dismissLookaheadTicket();
    }
  }

  tick();
  lookaheadCountdownInterval = setInterval(tick, 100);
}

function sendLookaheadDecision(action) {
  if (!activeLookaheadTicket) return;
  const tid = activeLookaheadTicket.ticket_id;

  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "lookahead_action",
      ticket_id: tid,
      action: action
    }));
  }

  dismissLookaheadTicket();
}

function dismissLookaheadTicket() {
  if (lookaheadCountdownInterval) clearInterval(lookaheadCountdownInterval);
  activeLookaheadTicket = null;
  if (lookaheadCard) {
    lookaheadCard.style.display = "none";
  }
}

if (btnLookaheadBlock) {
  btnLookaheadBlock.addEventListener("click", () => sendLookaheadDecision("block"));
}
if (btnLookaheadAllow) {
  btnLookaheadAllow.addEventListener("click", () => sendLookaheadDecision("allow"));
}

function formatSensitivityUI(val) {
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  let modeName = isEn ? "Balanced" : "Сбалансированная";
  let desc = isEn
    ? "Balanced mode (Twitch / YouTube standard). Filters explicit nudity without false triggers."
    : "Сбалансированный режим (стандарт Twitch / YouTube). Отсекает явную наготу без ложных срабатываний.";
  if (val < 46) {
    modeName = isEn ? "Soft" : "Мягкая";
    desc = isEn
      ? "Soft mode: reacts only to 100% explicit nudity (exposed anatomy). Minimal load, zero false alarms."
      : "Мягкий режим: реагирует только на 100% явную наготу (открытые органы). Минимальная нагрузка, ноль ложных тревог.";
  } else if (val > 75) {
    modeName = isEn ? "Strict" : "Строгая";
    desc = isEn
      ? "Strict mode: maximum sensitivity. Blurs questionable angles, partial nudity, and silhouettes."
      : "Строгий режим: максимальная чувствительность. Размывает спорные ракурсы, частичную наготу и тени.";
  }
  if (thresholdValBadge) {
    thresholdValBadge.textContent = `${val}% • ${modeName}`;
  }
  if (sensitivityDescText) {
    sensitivityDescText.textContent = desc;
  }
}

function updateGuardPulse(active) {
  if (radarPulse) {
    radarPulse.className = active ? "pulsing-radar-dot" : "pulsing-radar-dot inactive";
  }
}

// Emergency Censor & Countdown Line Elements
const muteProgressWrap = document.getElementById("mute-progress-wrap");
const muteProgressLine = document.getElementById("mute-progress-line");
const muteCountdownText = document.getElementById("mute-countdown-text");

// Live Real-Time Stream Monitor Elements
const liveNsfwCard = document.getElementById("live-nsfw-card");
const liveNsfwRadarDot = document.getElementById("live-nsfw-radar-dot");
const liveNsfwStatusPill = document.getElementById("live-nsfw-status-pill");
const liveNsfwClassTag = document.getElementById("live-nsfw-class-tag");
const liveNsfwPctVal = document.getElementById("live-nsfw-pct-val");
const liveNsfwMeterFill = document.getElementById("live-nsfw-meter-fill");
const liveThresholdMarker = document.getElementById("live-threshold-marker");
const meterThresholdLabel = document.getElementById("meter-threshold-label");
const liveNsfwTime = document.getElementById("live-nsfw-time");



const liveStage1Val = document.getElementById("live-stage1-val");
const liveStage1Label = document.getElementById("live-stage1-label");
const liveStage1Bar = document.getElementById("live-stage1-bar");

const liveBoxesBadge = document.getElementById("live-boxes-badge");
const liveStage2Label = document.getElementById("live-stage2-label");
const liveStage2Bar = document.getElementById("live-stage2-bar");

const liveOcrCard = document.getElementById("live-ocr-card");
const liveOcrRadarDot = document.getElementById("live-ocr-radar-dot");
const liveOcrStatusPill = document.getElementById("live-ocr-status-pill");
const liveOcrPreviewText = document.getElementById("live-ocr-preview-text");
const liveOcrWordsWrap = document.getElementById("live-ocr-words-wrap");
const liveOcrTime = document.getElementById("live-ocr-time");

// Incidents / Audit Log Elements & Statistics
const incidentsTbody = document.getElementById("incidents-tbody");
const incidentsEmptyRow = document.getElementById("incidents-empty-row");
const btnClearIncidents = document.getElementById("btn-clear-incidents");
const btnRunNsfwSim = document.getElementById("btn-run-nsfw-sim");

const statTotalIncidents = document.getElementById("stat-total-incidents");
const statNsfwIncidents = document.getElementById("stat-nsfw-incidents");
const statOcrIncidents = document.getElementById("stat-ocr-incidents");
const statCuesIncidents = document.getElementById("stat-cues-incidents");
const countAll = document.getElementById("count-all");
const countNsfw = document.getElementById("count-nsfw");
const countOcr = document.getElementById("count-ocr");
const countCue = document.getElementById("count-cue");
const countActive = document.getElementById("count-active");

let currentThreshold = 65;
let incidentCounter = 0;
let totalIncidents = 0;
let nsfwIncidents = 0;
let ocrIncidents = 0;
let cuesIncidents = 0;
let activeIncidents = 0;
let activeAuditFilter = "all";

function updateAuditStats() {
  if (statTotalIncidents) statTotalIncidents.textContent = totalIncidents;
  if (statNsfwIncidents) statNsfwIncidents.textContent = nsfwIncidents;
  if (statOcrIncidents) statOcrIncidents.textContent = ocrIncidents;
  if (statCuesIncidents) statCuesIncidents.textContent = cuesIncidents;
  if (countAll) countAll.textContent = totalIncidents;
  if (countNsfw) countNsfw.textContent = nsfwIncidents;
  if (countOcr) countOcr.textContent = ocrIncidents;
  if (countCue) countCue.textContent = cuesIncidents;
  if (countActive) countActive.textContent = activeIncidents;
}

function applyAuditFilter(filter) {
  activeAuditFilter = filter;
  document.querySelectorAll(".audit-tab").forEach(tab => {
    tab.classList.toggle("active", tab.dataset.filter === filter);
  });

  if (!incidentsTbody) return;
  const rows = incidentsTbody.querySelectorAll("tr:not(.empty-row)");
  let visibleCount = 0;
  rows.forEach(row => {
    const isNsfw = row.dataset.type === "nsfw";
    const isOcr = row.dataset.type === "ocr";
    const isCue = row.dataset.type === "cue";
    const isActive = row.dataset.active === "true";

    let show = true;
    if (filter === "nsfw") show = isNsfw;
    else if (filter === "ocr") show = isOcr;
    else if (filter === "cue") show = isCue;
    else if (filter === "active") show = isActive;

    row.style.display = show ? "" : "none";
    if (show) visibleCount++;
  });

  if (incidentsEmptyRow) {
    incidentsEmptyRow.style.display = (rows.length === 0 || visibleCount === 0) ? "" : "none";
  }
}

// Attach filter tabs click listeners
document.querySelectorAll(".audit-tab").forEach(tab => {
  tab.addEventListener("click", () => {
    applyAuditFilter(tab.dataset.filter || "all");
  });
});

// NOTE: The primary slider event listeners for nsfw-threshold-slider
// are registered below (see "NSFW Sensitivity Threshold Slider Listeners").
// This block intentionally left empty to avoid duplicate event handlers.

// NOTE: The primary censor category checkbox listeners are registered below
// (see "Selective Censor Category Badges Listeners").
// This block intentionally left empty to avoid duplicate event handlers.
const categoryCheckboxIds = [
  "cat-genitalia",
  "cat-breasts",
  "cat-buttocks",
  "cat-underwear",
  "cat-body-exposed"
];

function sendCensorCategories() {
  const cats = {
    genitalia: !!document.getElementById("cat-genitalia")?.checked,
    breasts: !!document.getElementById("cat-breasts")?.checked,
    buttocks: !!document.getElementById("cat-buttocks")?.checked,
    underwear: !!document.getElementById("cat-underwear")?.checked,
    body_exposed: !!document.getElementById("cat-body-exposed")?.checked,
  };
  try {
    localStorage.setItem("blewred_censor_categories", JSON.stringify(cats));
  } catch (e) { }
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "set_censor_categories",
      categories: cats
    }));
  }
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("set_censor_categories", { categories: cats }).catch(() => { });
  }
}

function updateCensorCategoriesUI(cats) {
  if (!cats) return;
  const map = {
    "cat-genitalia": cats.genitalia,
    "cat-breasts": cats.breasts,
    "cat-buttocks": cats.buttocks,
    "cat-underwear": cats.underwear,
    "cat-body-exposed": cats.body_exposed,
  };
  for (const [id, checked] of Object.entries(map)) {
    const el = document.getElementById(id);
    if (el) {
      el.checked = !!checked;
      el.closest(".censor-chip")?.classList.toggle("active", !!checked);
    }
  }
}

function setThresholdMarker(val) {
  currentThreshold = val;
  if (liveThresholdMarker) {
    liveThresholdMarker.style.left = `${val}%`;
  }
  if (meterThresholdLabel) {
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    meterThresholdLabel.textContent = isEn ? `Censor Threshold: ${val}%` : `Порог цензуры: ${val}%`;
  }
}

let lastLiveFrameData = null;

function localizeCategory(cat, isEn) {
  if (!cat) return isEn ? "NEUTRAL" : "НЕЙТРАЛЬНО";
  const upper = cat.toUpperCase();
  if (upper.includes("NEUTRAL") || upper.includes("НЕЙТРАЛЬН") || upper.includes("ЧИСТ")) {
    return isEn ? "NEUTRAL" : "НЕЙТРАЛЬНО";
  }
  if (upper.includes("BREAST") || upper.includes("ГРУДЬ")) {
    return isEn ? "EXPOSED BREASTS" : "ОТКРЫТАЯ ГРУДЬ";
  }
  if (upper.includes("GENITALIA") || upper.includes("ГЕНИТАЛИ")) {
    return isEn ? "GENITALIA" : "ГЕНИТАЛИИ";
  }
  if (upper.includes("BUTTOCK") || upper.includes("ANUS") || upper.includes("ЯГОДИЦ") || upper.includes("АНУС") || upper.includes("АНОРЕКТ")) {
    return isEn ? "BUTTOCKS / ANUS" : "ЯГОДИЦЫ / АНУС";
  }
  if (upper.includes("UNDERWEAR") || upper.includes("БЕЛЬ")) {
    return isEn ? "UNDERWEAR" : "БЕЛЬЕ / БИКИНИ";
  }
  if (upper.includes("TORSO") || upper.includes("ТОРС")) {
    return isEn ? "MALE TORSO" : "МУЖСКОЙ ТОРС";
  }
  if (upper.includes("PORN") || upper.includes("ПОРНО")) {
    return isEn ? "PORNOGRAPHY" : "ПОРНОГРАФИЯ";
  }
  return cat.toUpperCase();
}

function localizeStage1(label, isEn) {
  if (!label) return isEn ? "Neutral (100%)" : "Нейтрально (100%)";
  if (label.toLowerCase().includes("нейтральн") || label.toLowerCase().includes("neutral")) {
    return label.replace(/нейтрально/i, isEn ? "Neutral" : "Нейтрально");
  }
  if (label.toLowerCase().includes("порнограф") || label.toLowerCase().includes("porn")) {
    return label.replace(/порнография/i, isEn ? "Pornography" : "Порнография");
  }
  return label;
}

function updateLiveFrameMetrics(data) {
  if (!data) return;
  lastLiveFrameData = data;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  // 1. Update NSFW live radar
  if (liveNsfwClassTag) {
    liveNsfwClassTag.textContent = data.nsfw_category
      ? localizeCategory(data.nsfw_category, isEn)
      : (isEn ? "NEUTRAL" : "НЕЙТРАЛЬНО");
    liveNsfwClassTag.className = data.nsfw_violation ? "live-class-tag danger-tag" : "live-class-tag";
  }

  if (liveNsfwPctVal) {
    liveNsfwPctVal.textContent = data.nsfw_pct || `${(data.nsfw_score * 100).toFixed(1)}%`;
    liveNsfwPctVal.className = data.nsfw_violation ? "live-pct-number danger-text" : "live-pct-number";
  }

  if (liveNsfwMeterFill) {
    const pct = Math.min(100, Math.max(1, (data.nsfw_score || 0) * 100));
    liveNsfwMeterFill.style.width = `${pct}%`;
    liveNsfwMeterFill.className = data.nsfw_violation ? "meter-track-fill danger" : "meter-track-fill safe";
  }

  if (liveNsfwStatusPill) {
    liveNsfwStatusPill.textContent = data.censor_active
      ? (isEn ? "CENSOR ACTIVE" : "ЦЕНЗУРА АКТИВНА")
      : (isEn ? "STREAM SAFE" : "ПОТОК БЕЗОПАСЕН");
    liveNsfwStatusPill.className = data.censor_active ? "live-status-pill danger" : "live-status-pill safe";
  }

  if (liveNsfwCard) {
    liveNsfwCard.className = data.censor_active ? "live-monitor-card violation-active" : "live-monitor-card";
  }

  if (liveNsfwRadarDot) {
    liveNsfwRadarDot.className = data.censor_active ? "pulse-live-dot red" : "pulse-live-dot";
  }

  if (liveNsfwTime && data.timestamp) {
    liveNsfwTime.textContent = isEn ? `Frame: ${data.timestamp}` : `Кадр: ${data.timestamp}`;
  }

  // 2. Update OCR live monitor
  if (!isOcrEnabled) {
    if (liveOcrPreviewText) liveOcrPreviewText.textContent = isEn ? "(Screen text not scanned)" : "(Текст на экране не сканируется)";
    if (liveOcrWordsWrap) liveOcrWordsWrap.innerHTML = '<span class="badge-none">—</span>';
    if (liveOcrStatusPill) {
      liveOcrStatusPill.textContent = isEn ? "DISABLED" : "ОТКЛЮЧЕН";
      liveOcrStatusPill.className = "live-status-pill disabled";
    }
    if (liveOcrRadarDot) liveOcrRadarDot.className = "pulse-live-dot gray";
    if (liveOcrCard) liveOcrCard.className = "live-monitor-card ocr-disabled";
    if (liveOcrTime) liveOcrTime.textContent = "";
  } else {
    if (liveOcrPreviewText) {
      liveOcrPreviewText.textContent = data.ocr_snippet && data.ocr_snippet.trim().length > 0
        ? data.ocr_snippet
        : (isEn ? "(no text detected on selected monitor)" : "(текст на выбранном мониторе не обнаружен)");
    }

    if (liveOcrWordsWrap) {
      if (data.banned_words && data.banned_words.length > 0) {
        liveOcrWordsWrap.innerHTML = data.banned_words.map(w => `<span class="badge-banned-word">${w.toUpperCase()}</span>`).join(" ");
      } else {
        liveOcrWordsWrap.innerHTML = isEn
          ? '<span class="badge-none">NO STOPWORDS IN FRAME</span>'
          : '<span class="badge-none">НЕТ СТОП-СЛОВ В КАДРЕ</span>';
      }
    }

    if (liveOcrStatusPill) {
      liveOcrStatusPill.textContent = data.ocr_violation
        ? (isEn ? "STOPWORD IN FRAME" : "СТОП-СЛОВО В КАДРЕ")
        : (isEn ? "TEXT CLEAN" : "ТЕКСТ ЧИСТ");
      liveOcrStatusPill.className = data.ocr_violation ? "live-status-pill danger" : "live-status-pill safe";
    }

    if (liveOcrRadarDot) {
      liveOcrRadarDot.className = data.ocr_violation ? "pulse-live-dot red" : "pulse-live-dot blue";
    }

    if (liveOcrCard) {
      liveOcrCard.className = data.ocr_violation ? "live-monitor-card violation-active" : "live-monitor-card";
    }

    if (liveOcrTime && data.timestamp) {
      liveOcrTime.textContent = isEn ? `Frame: ${data.timestamp}` : `Кадр: ${data.timestamp}`;
    }
  }

  // 3. Update OBS Plugin Status
  updateObsPluginStatusUI(!!data.obs_plugin_active, !!data.obs_connected);

  // 4. Update Cascade Stage 1 (ViT Screener)
  if (liveStage1Val) {
    liveStage1Val.textContent = data.stage1_pct || `${((data.stage1_score || 0) * 100).toFixed(1)}%`;
  }
  if (liveStage1Label) {
    liveStage1Label.textContent = isEn
      ? localizeStage1(data.stage1_label, true)
      : (data.stage1_label || "Нейтрально (100%)");
  }
  if (liveStage1Bar) {
    const s1Pct = Math.min(100, Math.max(2, (data.stage1_score || 0) * 100));
    liveStage1Bar.style.width = `${s1Pct}%`;
    liveStage1Bar.className = s1Pct >= 65 ? "mini-fill danger" : (s1Pct >= 20 ? "mini-fill warn" : "mini-fill");
  }

  // 5. Update Cascade Stage 2 (640m Localizer) & Bounding Boxes
  if (liveBoxesBadge) {
    const bCount = data.box_count || 0;
    liveBoxesBadge.textContent = bCount > 0
      ? (isEn ? `${bCount} box(es)` : `${bCount} бокс(ов)`)
      : (isEn ? "0 boxes" : "0 боксов");
    liveBoxesBadge.className = bCount > 0 ? "box-badge active" : "box-badge";
  }
  if (liveStage2Label) {
    const bCount = data.box_count || 0;
    const s2Pct = data.stage2_pct || `${((data.stage2_score || 0) * 100).toFixed(0)}%`;
    liveStage2Label.textContent = bCount > 0
      ? (isEn ? `${bCount} box(es) (${s2Pct})` : `${bCount} бокс(а) (${s2Pct})`)
      : (isEn ? "No anatomy detected" : "Анатомии не найдено");
  }
  if (liveStage2Bar) {
    const s2Pct = Math.min(100, Math.max(2, (data.stage2_score || 0) * 100));
    liveStage2Bar.style.width = `${s2Pct}%`;
    liveStage2Bar.className = s2Pct >= 65 ? "mini-fill danger" : (s2Pct >= 15 ? "mini-fill warn" : "mini-fill");
  }

  // 6. Update active monitor badge if available
  if (monitorActiveBadge && data.monitor_name) {
    monitorActiveBadge.textContent = data.monitor_name;
  }

  // 7. Synchronize Live Analysis Frame Rate and Dynamic Pacing Status
  if (data.real_fps !== undefined || data.dynamic_boost_active !== undefined) {
    applyRealtimeFpsState(data);
  }
}

function recordIncident(inc) {
  if (!incidentsTbody) return;
  if (incidentsEmptyRow) {
    incidentsEmptyRow.style.display = "none";
  }

  incidentCounter++;
  totalIncidents++;

  const vType = (inc.violation_type || "").toUpperCase();
  const isCue = vType.includes("CUE") || vType.includes("LOOKAHEAD") || vType.includes("SCHEDULED");
  const isNsfw = !isCue && vType.includes("NSFW");
  const isOcr = !isCue && !isNsfw;

  if (isCue) {
    cuesIncidents++;
  } else if (isNsfw) {
    nsfwIncidents++;
  } else {
    ocrIncidents++;
  }

  const incId = inc.id || `inc-${Date.now()}-${incidentCounter}`;
  const tr = document.createElement("tr");
  tr.id = `inc-row-${incId}`;
  tr.dataset.incidentId = incId;
  tr.dataset.type = isCue ? "cue" : (isNsfw ? "nsfw" : "ocr");
  tr.dataset.active = "true";
  if (inc.cue_id) {
    tr.dataset.cueId = inc.cue_id;
  }

  const _isEnInc = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const targetClass = isCue ? "target-cue" : (isNsfw ? "target-nsfw" : "target-ocr");
  let catBadge = "";
  if (isCue) {
    const badgeLabel = _isEnInc
      ? (vType.includes("LOOKAHEAD") ? "PREEMPTIVE (PLAYER)" : "TIMING (PLAYER)")
      : (vType.includes("LOOKAHEAD") ? "ПРЕВЕНТИВНЫЙ (ПЛЕЕР)" : "ТАЙМИНГ (ПЛЕЕР)");
    catBadge = `<span class="pill-tag" style="color: #38bdf8; border-color: rgba(56, 189, 248, 0.4);">${badgeLabel}</span>`;
  } else if (isNsfw) {
    catBadge = `<span class="pill-tag" style="color: #f87171; border-color: rgba(239, 68, 68, 0.4);">${_isEnInc ? "NUDITY (NSFW)" : "НАГОТА (NSFW)"}</span>`;
  } else {
    catBadge = `<span class="pill-tag" style="color: #f59e0b; border-color: rgba(245, 158, 11, 0.4);">${_isEnInc ? "STOPWORD (OCR)" : "СТОП-СЛОВО (OCR)"}</span>`;
  }

  const defaultMonitor = _isEnInc ? "Main Monitor" : "Основной монитор";
  const defaultTarget = _isEnInc ? "VIOLATION" : "НАРУШЕНИЕ";
  const defaultAction = _isEnInc ? "OBS Shield overlay active, audio muted" : "Защитный экран в OBS включен, звук заглушен";
  const activeStatus = _isEnInc ? "ACTIVE" : "АКТИВЕН";

  tr.innerHTML = `
    <td><span class="incident-time">${inc.timestamp || new Date().toTimeString().split(" ")[0]}</span></td>
    <td><span class="incident-monitor">${inc.monitor || defaultMonitor}</span></td>
    <td>${catBadge}</td>
    <td><span class="incident-target ${targetClass}">${inc.target || defaultTarget}</span></td>
    <td>${inc.circumstances || inc.action || defaultAction}</td>
    <td><span class="incident-status-tag active" id="inc-badge-${incId}">${activeStatus}</span></td>
  `;

  incidentsTbody.insertBefore(tr, incidentsTbody.firstChild);
  activeIncidents = incidentsTbody.querySelectorAll("tr[data-active='true']").length;
  updateAuditStats();
  applyAuditFilter(activeAuditFilter);

  while (incidentsTbody.children.length > 51) {
    incidentsTbody.removeChild(incidentsTbody.lastChild);
  }
}

function resolveCueIncident(cueId, timestamp) {
  if (!incidentsTbody) return;
  const rows = incidentsTbody.querySelectorAll("tr[data-type='cue'][data-active='true']");
  rows.forEach(row => {
    if (!cueId || row.dataset.cueId === cueId || !row.dataset.cueId) {
      row.dataset.active = "false";
      const tag = row.querySelector(".incident-status-tag");
      if (tag) {
        tag.className = "incident-status-tag resolved";
        const _isEnT = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        tag.textContent = _isEnT ? "RESOLVED: TIMING EXPIRED" : "СНЯТ: ТАЙМИНГ ИСТЕК";
      }
    }
  });

  activeIncidents = incidentsTbody.querySelectorAll("tr[data-active='true']").length;
  updateAuditStats();
  applyAuditFilter(activeAuditFilter);
}

function resolveSpecificIncident(incidentId, timestamp) {
  if (!incidentsTbody) return;
  const row = incidentsTbody.querySelector(`tr[data-incident-id='${incidentId}']`) || document.getElementById(`inc-row-${incidentId}`);
  if (row && row.dataset.active === "true") {
    row.dataset.active = "false";
    const tag = row.querySelector(".incident-status-tag");
    if (tag) {
      tag.className = "incident-status-tag resolved";
      const _isEnS = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
      tag.textContent = _isEnS ? "RESOLVED: STREAM SAFE" : "СНЯТ: ПОТОК БЕЗОПАСЕН";
    }
    activeIncidents = incidentsTbody.querySelectorAll("tr[data-active='true']").length;
    updateAuditStats();
    applyAuditFilter(activeAuditFilter);
  }
}

function resolveLatestIncident(timestamp) {
  if (!incidentsTbody) return;
  const rows = incidentsTbody.querySelectorAll("tr[data-active='true']:not([data-type='cue'])");
  rows.forEach(row => {
    row.dataset.active = "false";
    const tag = row.querySelector(".incident-status-tag");
    if (tag) {
      tag.className = "incident-status-tag resolved";
      const _isEnC = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
      tag.textContent = _isEnC ? "RESOLVED: SCREEN CLEAN" : "СНЯТ: ЭКРАН ЧИСТ";
    }
  });
  activeIncidents = incidentsTbody.querySelectorAll("tr[data-active='true']").length;
  updateAuditStats();
  applyAuditFilter(activeAuditFilter);
}

// Visual Mode Hardware Toggle Switch Elements
const fpsToggleCheckbox = document.getElementById("fps-toggle-checkbox");
const labelFps5 = document.getElementById("label-fps-5");
const labelFps60 = document.getElementById("label-fps-60");

// Stopwords Editor
const stopwordsTextarea = document.getElementById("stopwords-textarea");
const btnSaveRules = document.getElementById("btn-save-rules");
const activeRulesCounter = document.getElementById("active-rules-counter");

// Connect to Local Engine WebSocket
function connectWebSocket() {
  socket = new WebSocket(WS_URL);

  socket.onopen = () => {
    socket.send(JSON.stringify({ type: "register", client: "ui" }));
    // Sync user preferences from localStorage upon connection
    try {
      const savedShield = localStorage.getItem("blewred_censor_shield_enabled");
      if (savedShield !== null) {
        const shieldVal = savedShield === "true";
        socket.send(JSON.stringify({ type: "toggle_censor_shield", enabled: shieldVal }));
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("toggle_censor_shield", { enabled: shieldVal }).catch(() => { });
        }
      }
      const savedFps = localStorage.getItem("blewred_fps_boosted");
      if (savedFps !== null) {
        const fpsVal = savedFps === "true";
        socket.send(JSON.stringify({ type: "set_vision_boost", enabled: fpsVal }));
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("set_fps_boost", { enabled: fpsVal }).catch(() => { });
        }
      }
      const savedMode = localStorage.getItem("blewred_operation_mode");
      if (savedMode !== null) {
        const modeVal = parseInt(savedMode, 10);
        if (!isNaN(modeVal)) {
          socket.send(JSON.stringify({ type: "set_operation_mode", mode: modeVal }));
          if (window.__TAURI__ && window.__TAURI__.core) {
            window.__TAURI__.core.invoke("set_operation_mode", { mode: modeVal }).catch(() => { });
          }
        }
      }
      const savedDonate = localStorage.getItem("blewred_shield_donate");
      const donateVal = savedDonate !== "false";
      socket.send(JSON.stringify({ type: "toggle_shield_donate", enabled: donateVal }));

      const savedOcr = localStorage.getItem("blewred_ocr_enabled");
      if (savedOcr !== null) {
        const ocrVal = savedOcr === "true";
        socket.send(JSON.stringify({ type: "toggle_ocr", enabled: ocrVal }));
      }

      const savedLang = localStorage.getItem("blewred_lang") || (typeof currentLanguage !== "undefined" ? currentLanguage : "ru");
      socket.send(JSON.stringify({ type: "set_language", language: savedLang }));
    } catch (e) {
      console.error("[UI] Error syncing preferences on WS open:", e);
    }
  };

  socket.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data);
      handleServerMessage(data);
    } catch (e) {
      console.error("[UI] Error parsing message:", e);
    }
  };

  socket.onclose = () => {
    updateObsStatus(false);
    updateObsPluginStatusUI(false, false);
    updateExtensionStatusUI(false);
    const screenDot = screenPill ? screenPill.querySelector(".status-dot") : null;
    setPillStatus(screenStatusText, screenDot, false);
    setTimeout(connectWebSocket, 2000);
  };

  socket.onerror = () => {
    socket.close();
  };
}

function handleServerMessage(data) {
  switch (data.type) {
    case "init_state":
      if (stopwordsTextarea) stopwordsTextarea.value = data.rules_text || "";
      if (activeRulesCounter) {
        const _isEnR = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        activeRulesCounter.innerText = `${data.rules_count || 0} ${_isEnR ? "rules" : "правил"}`;
      }
      updateObsStatus(data.obs_connected);
      if (gpuStatVal && data.gpu_info) {
        gpuStatVal.innerText = data.gpu_info;
      }
      if (data.ru_ocr_installed !== undefined || data.en_ocr_installed !== undefined) {
        updateOcrStatusUI(data.ru_ocr_installed, data.en_ocr_installed);
      }
      if (data.hotkey_scope !== undefined || data.hotkey_panic !== undefined || data.hotkey_threat !== undefined) {
        updateHotkeySettingsUI(data.hotkey_scope, data.hotkey_panic, data.hotkey_threat);
      }
      if (data.models_status) {
        updateModelsStatusUI(data.models_status);
      }
      if (data.monitors) {
        updateMonitorsDropdown(data.monitors, data.selected_monitor);
        updateHudMonitorsDropdown(data.monitors, data.hud_monitor);
      }
      if (data.ocr_enabled !== undefined) {
        let savedOcr = null;
        try { savedOcr = localStorage.getItem("blewred_ocr_enabled"); } catch (e) { }
        const targetOcr = savedOcr !== null ? (savedOcr === "true") : data.ocr_enabled;
        updateOcrStateUI(targetOcr);
        if (savedOcr !== null && (savedOcr === "true") !== data.ocr_enabled) {
          if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ type: "toggle_ocr", enabled: targetOcr }));
          }
          if (window.__TAURI__ && window.__TAURI__.core) {
            window.__TAURI__.core.invoke("toggle_ocr", { enabled: targetOcr }).catch(() => { });
          }
        }
      }
      if (data.realtime_guard !== undefined && toggleRealtimeGuard) {
        toggleRealtimeGuard.checked = !!data.realtime_guard;
        updateGuardPulse(!!data.realtime_guard);
      }
      if (data.extension_connected !== undefined) {
        updateExtensionStatusUI(data.extension_connected);
      }
      if (data.model_profiles && data.active_model_profile) {
        updateModelProfilesList(data.model_profiles, data.active_model_profile);
      }
      if (data.model_tuning) {
        applyModelTuningToUI(data.model_tuning, data.active_model_profile);
      }
      // Prioritize user's saved preference for Censor Shield (Default: ON)
      let shieldEnabled = true;
      try {
        const savedShield = localStorage.getItem("blewred_censor_shield_enabled");
        if (savedShield !== null) {
          shieldEnabled = (savedShield === "true");
        } else if (data.censor_shield_enabled !== undefined) {
          shieldEnabled = !!data.censor_shield_enabled;
        }
        if (shieldEnabled !== !!data.censor_shield_enabled) {
          if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ type: "toggle_censor_shield", enabled: shieldEnabled }));
          }
          if (window.__TAURI__ && window.__TAURI__.core) {
            window.__TAURI__.core.invoke("toggle_censor_shield", { enabled: shieldEnabled }).catch(() => { });
          }
        }
      } catch (e) { }
      if (toggleCensorShield) {
        toggleCensorShield.checked = shieldEnabled;
      }
      updateCensorShieldUI(shieldEnabled, data.is_shield_visible);

      // Prioritize user's saved preference for Dev Support on Censor Shield
      let donateEnabled = data.shield_donate_enabled !== undefined ? !!data.shield_donate_enabled : true;
      try {
        const savedDonate = localStorage.getItem("blewred_shield_donate");
        if (savedDonate !== null) {
          donateEnabled = savedDonate !== "false";
          if (donateEnabled !== !!data.shield_donate_enabled) {
            if (socket && socket.readyState === WebSocket.OPEN) {
              socket.send(JSON.stringify({ type: "toggle_shield_donate", enabled: donateEnabled }));
            }
          }
        }
      } catch (e) { }
      if (toggleShieldDonate) {
        toggleShieldDonate.checked = donateEnabled;
      }

      // Prioritize user's saved preference for 60 FPS boost
      try {
        const savedFps = localStorage.getItem("blewred_fps_boosted");
        if (savedFps !== null) {
          const fpsBoost = savedFps === "true";
          setFpsMode(fpsBoost);
        }
      } catch (e) { }

      if (data.nsfw_threshold !== undefined && nsfwThresholdSlider) {
        nsfwThresholdSlider.value = data.nsfw_threshold;
        formatSensitivityUI(data.nsfw_threshold);
        setThresholdMarker(data.nsfw_threshold);
      }
      if (data.censor_categories) {
        updateCensorCategoriesUI(data.censor_categories);
      }
      if (data.events && Array.isArray(data.events)) {
        data.events.forEach(renderEventItem);
      }
      if (typeof initSetupGuideVisibility === "function") {
        initSetupGuideVisibility(data.hide_setup_guide);
      }
      if (typeof updateSetupGuideStatus === "function") {
        updateSetupGuideStatus(data);
      }
      if (data.language && !localStorage.getItem("blewred_lang")) {
        setLanguage(data.language);
      }
      break;

    case "language_changed":
      if (data.language && data.language !== currentLanguage) {
        setLanguage(data.language);
      }
      break;

    case "hide_setup_guide_changed":
      if (chkHideSetupGuide) chkHideSetupGuide.checked = !!data.hide;
      if (streamerSetupGuide) streamerSetupGuide.style.display = data.hide ? "none" : "block";
      try { localStorage.setItem("blewred_hide_setup_guide", data.hide ? "true" : "false"); } catch (e) { }
      break;

    case "live_frame_metrics":
      updateLiveFrameMetrics(data);
      break;

    case "new_incident":
      if (data.incident) {
        recordIncident(data.incident);
        const _isEnNewInc = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        renderEventItem({
          type: "danger",
          timestamp: data.incident.timestamp,
          message: _isEnNewInc
            ? `[CENSOR INCIDENT] ${data.incident.target} on ${data.incident.monitor}. Blur activated.`
            : `[ИНЦИДЕНТ ЦЕНЗУРЫ] ${data.incident.target} на ${data.incident.monitor}. Блюр активирован.`
        });
      }
      break;

    case "incident_resolved":
      if (data.incident_id) {
        resolveSpecificIncident(data.incident_id, data.timestamp);
      } else {
        resolveLatestIncident(data.timestamp);
      }
      {
        const _isEnIncRes = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        renderEventItem({
          type: "success",
          timestamp: data.timestamp || new Date().toTimeString().split(" ")[0],
          message: _isEnIncRes ? "[INCIDENT RESOLVED] Threat cleared, stream safe." : "[ИНЦИДЕНТ СНЯТ] Угроза устранена, поток безопасен."
        });
      }
      break;

    case "model_profiles_changed":
      if (typeof updateModelProfilesList === "function") {
        updateModelProfilesList(data.profiles, data.active_model_profile);
      }
      break;

    case "model_tuning_changed":
      if (typeof applyModelTuningToUI === "function") {
        applyModelTuningToUI(data.tuning, data.active_model_profile);
      }
      break;

    case "realtime_guard_changed":
      {
        const _isEnGuard = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        renderEventItem({
          type: data.enabled ? "success" : "warning",
          timestamp: new Date().toTimeString().split(" ")[0],
          message: _isEnGuard
            ? `[Real-time Guard] Active live stream protection: ${data.enabled ? "ENABLED (Monitoring active)" : "DISABLED"}`
            : `[Real-time Guard] Активная защита стрима в реальном времени: ${data.enabled ? "ВКЛЮЧЕНА (Мониторинг активен)" : "ВЫКЛЮЧЕНА"}`
        });
      }
      break;

    case "scheduled_cue_ended":
      resolveCueIncident(data.cue_id, data.timestamp);
      {
        const _isEnCueEnd = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        renderEventItem({
          type: "success",
          timestamp: data.timestamp || new Date().toTimeString().split(" ")[0],
          message: _isEnCueEnd ? "[TIMING FINISHED] Censored segment finished, OBS screen restored." : "[ТАЙМИНГ ЗАВЕРШЕН] Отрезок цензуры завершен, экран в OBS открыт."
        });
      }
      break;

    case "censor_shield_changed":
      if (toggleCensorShield) {
        let isShieldOn = data.enabled;
        try {
          const savedShield = localStorage.getItem("blewred_censor_shield_enabled");
          if (savedShield !== null) {
            isShieldOn = savedShield === "true";
          }
        } catch (e) { }
        toggleCensorShield.checked = isShieldOn;
      }
      {
        const _isEnShield = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        renderEventItem({
          type: data.enabled ? "system" : "warning",
          timestamp: new Date().toTimeString().split(" ")[0],
          message: _isEnShield
            ? `[Censor Shield] Fullscreen overlay: ${data.enabled ? "ENABLED (Fullscreen + Mute)" : "DISABLED (OBS selective blur only)"}`
            : `[Censor Shield] Полноэкранная заставка: ${data.enabled ? "ВКЛЮЧЕНА (Полный экран + Mute)" : "ОТКЛЮЧЕНА (Только выборочный блюр в OBS)"}`
        });
      }
      break;

    case "monitor_changed":
      if (monitorSelect) monitorSelect.value = data.selected_monitor;
      if (monitorActiveBadge) {
        const _isEnMon = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        monitorActiveBadge.innerText = data.selected_monitor === -1
          ? (_isEnMon ? "All Monitors" : "Все мониторы")
          : (_isEnMon ? `Monitor ${data.selected_monitor + 1}` : `Монитор ${data.selected_monitor + 1}`);
      }
      break;

    case "hud_monitor_changed":
      if (hudMonitorSelect) {
        hudMonitorSelect.value = data.hud_monitor;
      }
      break;

    case "lookahead_ticket":
      if (data.ticket) {
        renderLookaheadTicket(data.ticket);
        const _isEnTicket = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        renderEventItem({
          type: "danger",
          timestamp: new Date().toTimeString().split(" ")[0],
          message: _isEnTicket
            ? `[PREEMPTIVE LOOKAHEAD] ${data.ticket.reason} (ETA: ${data.ticket.eta_seconds.toFixed(1)}s). Streamer alerted.`
            : `[ПРЕВЕНТИВНЫЙ АНАЛИЗ] ${data.ticket.reason} (ETA: ${data.ticket.eta_seconds.toFixed(1)}с). Оповещение выведено стримеру.`
        });
      }
      break;

    case "lookahead_cue_executed":
      if (activeLookaheadTicket && activeLookaheadTicket.ticket_id === data.ticket_id) {
        dismissLookaheadTicket();
      }
      {
        const _isEnCueExec = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        renderEventItem({
          type: "danger",
          timestamp: new Date().toTimeString().split(" ")[0],
          message: _isEnCueExec
            ? `[LOOKAHEAD AUTO-CENSOR] Playback time reached: censor activated in OBS (${data.reason})`
            : `[АВТОЦЕНЗУРА LOOKAHEAD] Время показа наступило: цензура активирована в OBS (${data.reason})`
        });
      }
      break;

    case "lookahead_preview_state":
      if (btnToggleLookaheadPreview) {
        if (data.is_open) {
          btnToggleLookaheadPreview.classList.add("active");
        } else {
          btnToggleLookaheadPreview.classList.remove("active");
        }
      }
      break;

    case "operation_mode_changed":
      if (data.operation_mode !== undefined) {
        updateOperationModeUI(data.operation_mode);
      }
      break;

    case "extension_status":
      if (data.connected !== undefined) {
        updateExtensionStatusUI(data.connected);
      }
      break;

    case "obs_status_report":
      updateObsPromptBadge(data);
      break;

    case "censor_shield_changed":
      if (toggleCensorShield && document.activeElement !== toggleCensorShield) {
        toggleCensorShield.checked = data.enabled;
      }
      updateCensorShieldUI(data.enabled, data.is_shield_visible);
      break;

    case "shield_donate_changed":
      if (toggleShieldDonate && document.activeElement !== toggleShieldDonate) {
        toggleShieldDonate.checked = !!data.enabled;
      }
      break;

    case "ocr_state_changed":
      if (data.enabled !== undefined) {
        updateOcrStateUI(data.enabled);
      }
      break;

    case "hotkey_settings_changed":
      updateHotkeySettingsUI(data.hotkey_scope, data.hotkey_panic, data.hotkey_threat);
      break;

    case "telemetry":
      updateTelemetry(data);
      break;

    case "censor_test_result":
      renderCensorTestResult(data);
      break;

    case "ocr_test_result":
      renderOcrTestResult(data.result);
      break;

    case "nsfw_test_result":
      renderNsfwTestResult(data.result);
      break;

    case "shield_extended":
      renderEventItem({
        type: "warning",
        timestamp: new Date().toTimeString().split(" ")[0],
        message: `[${(typeof currentLanguage !== "undefined" && currentLanguage === "en") ? "CENSOR EXTENDED" : "ЦЕНЗУРА ПРОДЛЕНА"}] ${data.reason}`
      });
      if (shieldPillStatus) shieldPillStatus.innerText = (typeof currentLanguage !== "undefined" && currentLanguage === "en") ? "EXTENDED" : "ПРОДЛЕНО";
      break;

    case "obs_scenes_updated":
      updateSceneDropdown(data.scenes, data.current_scene);
      break;

    case "obs_setup_result":
      handleObsSetupResult(data);
      break;

    case "banned_violation_spotted":
    case "banned_audio_spotted":
      // Render detected video/OCR/subtitles banned words
      if (data.is_banned) {
        renderBannedAlert(data);
      }
      break;

    case "new_event":
      renderEventItem(data.event);
      break;

    case "emergency_mute_triggered":
      startEmergencyMuteCountdown(data.duration_ms || 3000);
      break;

    case "test_result":
      renderTestResult(data);
      break;

    case "cues_config_updated":
      if (data.config) {
        updateCuesConfigUI(data.config);
      }
      break;

    case "recognized_cues_result":
      handleRecognizedCuesResult(data);
      break;

    case "ocr_status_result":
      if (data.status) {
        updateOcrStatusBadge(data.status);
      }
      break;

    case "models_status_result":
    case "models_status":
      if (data.status || data.models_status) {
        updateModelsStatusUI(data.status || data.models_status);
      }
      break;

    case "model_download_progress":
      handleModelDownloadProgress(data);
      break;

    case "models_download_complete":
      handleModelDownloadComplete(data);
      if (data.status) {
        updateModelsStatusUI(data.status);
      }
      break;

    case "model_download_error":
      handleModelDownloadComplete({ success: false, message: data.message || data.error });
      break;
  }
}
if (typeof window !== "undefined") window.handleServerMessage = handleServerMessage;

function updateTelemetry(data) {
  // OBS Connection Status
  updateObsStatus(data.obs_connected);

  if (gpuStatVal && data.gpu_info) {
    gpuStatVal.innerText = data.gpu_info;
  }

  if (data.ru_ocr_installed !== undefined || data.en_ocr_installed !== undefined) {
    updateOcrStatusUI(data.ru_ocr_installed, data.en_ocr_installed);
  }

  // Sync OCR state from backend telemetry, respecting user's saved preference
  if (data.ocr_enabled !== undefined && (Date.now() - lastUserOcrChangeTime > 2500)) {
    let savedOcr = null;
    try {
      savedOcr = localStorage.getItem("blewred_ocr_enabled");
    } catch (e) { }

    if (savedOcr !== null) {
      const userChoice = (savedOcr === "true");
      if (userChoice !== data.ocr_enabled) {
        if (socket && socket.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ type: "toggle_ocr", enabled: userChoice }));
        }
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("toggle_ocr", { enabled: userChoice }).catch(() => { });
        }
      }
      updateOcrStateUI(userChoice);
    } else {
      updateOcrStateUI(data.ocr_enabled);
    }
  }

  // Sync Operation Mode from backend telemetry or re-assert user preference if backend is out of sync
  if (data.operation_mode !== undefined && data.operation_mode !== currentOperationMode && (Date.now() - lastUserModeChangeTime > 2500)) {
    let savedMode = null;
    try {
      savedMode = localStorage.getItem("blewred_operation_mode");
    } catch (e) { }

    if (savedMode !== null) {
      const targetMode = parseInt(savedMode, 10);
      if (!isNaN(targetMode) && targetMode !== data.operation_mode) {
        // Backend is out of sync with user's saved choice; re-assert user's choice
        if (socket && socket.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ type: "set_operation_mode", mode: targetMode }));
        }
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("set_operation_mode", { mode: targetMode }).catch(() => { });
        }
        updateOperationModeUI(targetMode);
      } else if (!isNaN(targetMode)) {
        updateOperationModeUI(targetMode);
      }
    } else {
      updateOperationModeUI(data.operation_mode);
    }
  }

  // Sync Real-time FPS, Dynamic Threat Escalation, and Hardware Switch State
  if (data.fps_boosted !== undefined && document.activeElement !== fpsToggleCheckbox) {
    isBoosted = !!data.fps_boosted;
    try {
      localStorage.setItem("blewred_fps_boosted", isBoosted ? "true" : "false");
    } catch (e) { }
  }
  applyRealtimeFpsState(data);

  // Guard pulse indicator is permanently active
  updateGuardPulse(true);

  // Browser Extension connection pulse
  if (data.extension_connected !== undefined) {
    updateExtensionStatusUI(data.extension_connected);
  }

  if (data.nsfw_threshold !== undefined && nsfwThresholdSlider && document.activeElement !== nsfwThresholdSlider) {
    nsfwThresholdSlider.value = data.nsfw_threshold;
    if (thresholdValBadge) thresholdValBadge.textContent = `${data.nsfw_threshold}%`;
  }
  // Sync Censor Shield state from backend telemetry, giving priority to user's saved preference
  if (data.censor_shield_enabled !== undefined && (Date.now() - lastUserShieldChangeTime > 2500)) {
    let savedShield = null;
    try {
      savedShield = localStorage.getItem("blewred_censor_shield_enabled");
    } catch (e) { }

    if (savedShield !== null) {
      const userChoice = (savedShield === "true");
      if (userChoice !== data.censor_shield_enabled) {
        if (socket && socket.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ type: "toggle_censor_shield", enabled: userChoice }));
        }
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("toggle_censor_shield", { enabled: userChoice }).catch(() => { });
        }
      }
      if (toggleCensorShield && document.activeElement !== toggleCensorShield) {
        toggleCensorShield.checked = userChoice;
      }
      updateCensorShieldUI(userChoice, data.is_shield_visible);
    } else {
      const defaultChoice = data.censor_shield_enabled !== undefined ? !!data.censor_shield_enabled : true;
      if (toggleCensorShield && document.activeElement !== toggleCensorShield) {
        toggleCensorShield.checked = defaultChoice;
      }
      updateCensorShieldUI(defaultChoice, data.is_shield_visible);
    }
  } else {
    updateCensorShieldUI(toggleCensorShield ? toggleCensorShield.checked : false, data.is_shield_visible);
  }

  // Real OBS Canvas Resolution & FPS
  if (obsCanvasStat && data.obs_resolution) {
    obsCanvasStat.innerText = data.obs_resolution;
  }

  // Real Screen Capture Status (detects whether eye is crossed out in OBS)
  if (screenStatVal) {
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    if (data.obs_connected) {
      if (data.capture_active) {
        screenStatVal.innerText = `${data.capture_source || (isEn ? "Active" : "Активен")}`;
        screenStatVal.className = "stat-value stat-success";
        setPillStatus(screenStatusText, screenPill ? screenPill.querySelector(".status-dot") : null, true);
      } else {
        screenStatVal.innerText = `${data.capture_source || (isEn ? "Hidden in OBS" : "Скрыт в OBS")}`;
        screenStatVal.className = "stat-value stat-warning";
        setPillStatus(screenStatusText, screenPill ? screenPill.querySelector(".status-dot") : null, false);
      }
    } else {
      screenStatVal.innerText = isEn ? "Waiting for OBS" : "Ожидание OBS";
      screenStatVal.className = "stat-value";
      setPillStatus(screenStatusText, screenPill ? screenPill.querySelector(".status-dot") : null, false);
    }
  }

  // Update scene list in dropdown
  if (data.scenes && Array.isArray(data.scenes) && data.scenes.length > 0) {
    updateSceneDropdown(data.scenes, data.current_scene);
  }

  // Update physical monitors in dropdown
  if (data.monitors && Array.isArray(data.monitors) && data.monitors.length > 0) {
    updateMonitorsDropdown(data.monitors, data.selected_monitor);
    updateHudMonitorsDropdown(data.monitors, data.hud_monitor);
  }

  // Update Scheduled Cues Count & Live Player Sync
  if (data.cues_count !== undefined && cuesCountBadge) {
    cuesCountBadge.textContent = data.cues_count;
  }
  if (data.player_sync) {
    updatePlayerSyncUI(data.player_sync);
  }
  if (typeof updateSetupGuideStatus === "function") {
    updateSetupGuideStatus(data);
  }
}

let lastMonitorsList = null;
let lastSelectedMonitor = null;
let lastSelectedHudMonitor = null;

function formatMonitorName(m, isEn) {
  if (!m) return isEn ? "Monitor 1 (Primary)" : "Монитор 1 (Основной)";
  const idx = (m.index !== undefined && m.index !== null) ? (m.index + 1) : 1;
  const primStr = m.is_primary ? (isEn ? " (Primary)" : " (Основной)") : "";
  const resStr = (m.width && m.height) ? ` — ${m.width}×${m.height}` : "";
  return isEn ? `Monitor ${idx}${primStr}${resStr}` : `Монитор ${idx}${primStr}${resStr}`;
}

function updateMonitorsDropdown(monitors, selectedMonitor) {
  if (!monitorSelect) return;
  if (monitors) lastMonitorsList = monitors;
  if (selectedMonitor !== undefined && selectedMonitor !== null) lastSelectedMonitor = selectedMonitor;

  // Auto-heal: If selected monitor no longer exists in current displays, fallback to Primary Monitor
  if (monitors && monitors.length > 0) {
    const exists = monitors.some(m => m.index === lastSelectedMonitor);
    if (!exists) {
      const prim = monitors.find(m => m.is_primary) || monitors[0];
      lastSelectedMonitor = prim.index;
      try {
        localStorage.setItem("blewred_selected_monitor", prim.index.toString());
      } catch (err) { }
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: "set_monitor", monitor_index: prim.index }));
      }
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("set_monitor", { index: prim.index }).catch(() => { });
      }
    }
  }

  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const signature = JSON.stringify({ monitors, selectedMonitor: lastSelectedMonitor, isEn });
  if (signature === cachedMonitorsJson) return;
  cachedMonitorsJson = signature;

  const currentSelection = monitorSelect.value;
  monitorSelect.innerHTML = "";

  // Physical monitors ONLY (Strictly 1 of the connected physical monitors)
  if (monitors && monitors.length > 0) {
    monitors.forEach(m => {
      const opt = document.createElement("option");
      opt.value = m.index;
      opt.innerText = formatMonitorName(m, isEn);
      if (lastSelectedMonitor !== undefined && lastSelectedMonitor !== null && lastSelectedMonitor >= 0) {
        if (m.index === lastSelectedMonitor) {
          opt.selected = true;
        }
      } else if (m.is_primary) {
        opt.selected = true;
      }
      monitorSelect.appendChild(opt);
    });
  } else {
    const optDefault = document.createElement("option");
    optDefault.value = 0;
    optDefault.innerText = isEn ? "Monitor 1 (Primary)" : "Монитор 1 (Основной)";
    optDefault.selected = true;
    monitorSelect.appendChild(optDefault);
  }

  if (monitorActiveBadge) {
    const monLabel = isEn ? "Monitor" : "Монитор";
    if (monitors && monitors.length > 0) {
      const found = monitors.find(m => m.index === lastSelectedMonitor);
      setSafeText(monitorActiveBadge, found ? `${monLabel} ${found.index + 1}` : (lastSelectedMonitor !== undefined && lastSelectedMonitor !== null && lastSelectedMonitor >= 0 ? `${monLabel} ${lastSelectedMonitor + 1}` : `${monLabel} 1`));
    } else {
      setSafeText(monitorActiveBadge, lastSelectedMonitor !== undefined && lastSelectedMonitor !== null && lastSelectedMonitor >= 0 ? `${monLabel} ${lastSelectedMonitor + 1}` : `${monLabel} 1`);
    }
  }
}

// Monitor selector change listener
if (monitorSelect) {
  monitorSelect.addEventListener("change", (e) => {
    const idx = parseInt(e.target.value, 10);
    const validIdx = isNaN(idx) || idx < 0 ? 0 : idx;
    lastSelectedMonitor = validIdx;
    try {
      localStorage.setItem("blewred_selected_monitor", validIdx.toString());
    } catch (err) { }
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "set_monitor",
        monitor_index: validIdx
      }));
    }
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("set_monitor", { index: validIdx }).catch(() => { });
    }
    if (monitorActiveBadge) {
      const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
      monitorActiveBadge.innerText = isEn ? `Monitor ${validIdx + 1}` : `Монитор ${validIdx + 1}`;
    }
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    renderEventItem({
      type: "info",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn ? `Selected Monitor ${validIdx + 1} for OCR and NSFW analysis` : `Выбран Монитор ${validIdx + 1} для анализа OCR и NSFW`
    });
  });
}

function updateHudMonitorsDropdown(monitors, selectedHudMonitor) {
  if (!hudMonitorSelect || !monitors || monitors.length === 0) return;
  if (monitors) lastMonitorsList = monitors;
  if (selectedHudMonitor !== undefined && selectedHudMonitor !== null) lastSelectedHudMonitor = selectedHudMonitor;

  // Do not disrupt user if dropdown is focused
  if (document.activeElement === hudMonitorSelect) return;

  const savedHud = localStorage.getItem("blewred_hud_monitor");
  let targetVal = -1;
  if (lastSelectedHudMonitor !== undefined && lastSelectedHudMonitor !== null) {
    targetVal = lastSelectedHudMonitor;
  } else if (savedHud !== null) {
    targetVal = parseInt(savedHud, 10);
  }

  // Auto-heal HUD monitor: if saved index does not exist, reset to auto (-1)
  if (targetVal !== -1 && monitors && !monitors.some(m => m.index === targetVal)) {
    targetVal = -1;
    lastSelectedHudMonitor = -1;
    try {
      localStorage.setItem("blewred_hud_monitor", "-1");
    } catch (err) { }
  }

  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const signature = JSON.stringify({ monitors, targetVal, isEn });
  if (signature === cachedHudMonitorsJson) return;
  cachedHudMonitorsJson = signature;

  hudMonitorSelect.innerHTML = "";
  const optAuto = document.createElement("option");
  optAuto.value = -1;
  optAuto.innerText = isEn ? "Auto (Streamer Screen)" : "Авто (Экран стримера)";
  if (targetVal === -1) optAuto.selected = true;
  hudMonitorSelect.appendChild(optAuto);

  monitors.forEach(m => {
    const opt = document.createElement("option");
    opt.value = m.index;
    opt.innerText = formatMonitorName(m, isEn);
    if (m.index === targetVal) {
      opt.selected = true;
    }
    hudMonitorSelect.appendChild(opt);
  });
  hudMonitorSelect.value = targetVal;
}

// HUD Monitor selector change listener
if (hudMonitorSelect) {
  hudMonitorSelect.addEventListener("change", (e) => {
    const idx = parseInt(e.target.value, 10);
    const validIdx = isNaN(idx) ? -1 : idx;
    try {
      localStorage.setItem("blewred_hud_monitor", validIdx.toString());
    } catch (err) { }
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "set_hud_monitor",
        monitor_index: validIdx
      }));
    }
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("set_hud_monitor", { index: validIdx }).catch(() => { });
    }
  });
}

// Test HUD Alert Button Listener
if (btnTestLookaheadHud) {
  btnTestLookaheadHud.addEventListener("click", () => {
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("test_lookahead_alert").catch(() => { });
    } else if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "test_lookahead_alert" }));
    }
  });
}

// Lookahead Separate Preview Window Controls
function openLookaheadWindow() {
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("show_lookahead_preview").catch(() => { });
  } else if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: "open_lookahead_preview" }));
  }
}

function toggleLookaheadWindow() {
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("toggle_lookahead_preview").catch(() => { });
  } else if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: "toggle_lookahead_preview" }));
  }
}

if (btnOpenLookaheadWindow) {
  btnOpenLookaheadWindow.addEventListener("click", openLookaheadWindow);
}
if (btnToggleLookaheadPreview) {
  btnToggleLookaheadPreview.addEventListener("click", toggleLookaheadWindow);
}
if (btnLookaheadDetach) {
  btnLookaheadDetach.addEventListener("click", openLookaheadWindow);
}

function updateSceneDropdown(scenes, currentScene) {
  if (!obsSceneSelect || !scenes || scenes.length === 0) return;
  const signature = JSON.stringify({ scenes, currentScene });
  if (signature === cachedSceneJson) return;
  cachedSceneJson = signature;

  const currentSelection = obsSceneSelect.value;
  obsSceneSelect.innerHTML = "";

  scenes.forEach(scene => {
    const opt = document.createElement("option");
    opt.value = scene;
    opt.innerText = scene;
    if (scene === currentScene || (!currentScene && scene === currentSelection)) {
      opt.selected = true;
    }
    obsSceneSelect.appendChild(opt);
  });
}

// Scene selector change listener
if (obsSceneSelect) {
  obsSceneSelect.addEventListener("change", (e) => {
    const sceneName = e.target.value;
    if (sceneName && socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "set_obs_scene",
        scene_name: sceneName
      }));
      renderEventItem({
        type: "info",
        timestamp: new Date().toTimeString().split(" ")[0],
        message: (window.I18N && window.I18N.getLanguage() === "en") ? `Selected OBS scene: ${sceneName}` : `Выбрана сцена OBS: ${sceneName}`
      });
    }
    if (sceneName && window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("set_obs_scene", { scene: sceneName }).catch(() => { });
    }
  });
}

// Refresh scenes button listener
if (btnRefreshScenes) {
  btnRefreshScenes.addEventListener("click", () => {
    btnRefreshScenes.disabled = true;
    setTimeout(() => { btnRefreshScenes.disabled = false; }, 1000);
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "refresh_obs_scenes" }));
    }
  });
}

// Visual Boost Mode Handler
function setFpsMode(boosted) {
  isBoosted = !!boosted;
  if (fpsToggleCheckbox) {
    fpsToggleCheckbox.checked = isBoosted;
  }
  updateVisionUI(isBoosted);
  try {
    localStorage.setItem("blewred_fps_boosted", isBoosted ? "true" : "false");
  } catch (e) { }
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "set_vision_boost",
      enabled: isBoosted
    }));
  }
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("set_fps_boost", { enabled: isBoosted }).catch(() => { });
  }
}

function updateVisionUI(boosted) {
  isBoosted = !!boosted;
  if (fpsToggleCheckbox) fpsToggleCheckbox.checked = isBoosted;
  if (boosted) {
    if (labelFps5) labelFps5.classList.remove("active");
    if (labelFps60) labelFps60.className = "fps-switch-label active";
    if (fpsBadge) {
      fpsBadge.innerText = "60 FPS";
      fpsBadge.style.color = "#f59e0b";
      fpsBadge.style.borderColor = "rgba(245, 158, 11, 0.4)";
    }
    const _isEnVis = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    if (visionModeText) visionModeText.innerText = _isEnVis ? "Boosted" : "Усиленный";
    if (videoModeTag) videoModeTag.innerText = _isEnVis ? "60 FPS (Danger Zone)" : "60 FPS (Зона риска)";
    if (fpsStatVal) fpsStatVal.innerText = _isEnVis ? "60 fps" : "60 кадр/сек";
  } else {
    if (labelFps60) labelFps60.className = "fps-switch-label";
    if (labelFps5) labelFps5.className = "fps-switch-label active";
    if (fpsBadge) {
      fpsBadge.innerText = "5 FPS";
      fpsBadge.style.color = "#38bdf8";
      fpsBadge.style.borderColor = "rgba(56, 189, 248, 0.4)";
    }
    const _isEnVis = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    if (visionModeText) visionModeText.innerText = _isEnVis ? "Basic" : "Базовый";
    if (videoModeTag) videoModeTag.innerText = _isEnVis ? "5 FPS (Power Saver)" : "5 FPS (Энергосбережение)";
    if (fpsStatVal) fpsStatVal.innerText = _isEnVis ? "5 fps" : "5 кадр/сек";
  }
}

// Visual Boost Mode & Real-time FPS Pacing State
let currentRealFps = 5.0;
let currentDynamicBoostActive = false;

function applyRealtimeFpsState(data) {
  if (!data) return;

  const opMode = data.operation_mode !== undefined ? data.operation_mode : currentOperationMode;
  const isPaused = data.analysis_paused !== undefined
    ? !!data.analysis_paused
    : (opMode === 1 || opMode >= 3);

  const boosted = data.fps_boosted !== undefined
    ? !!data.fps_boosted
    : (data.boosted !== undefined ? !!data.boosted : isBoosted);
  isBoosted = boosted;

  const dynamicBoost = data.dynamic_boost_active !== undefined
    ? !!data.dynamic_boost_active
    : currentDynamicBoostActive;
  currentDynamicBoostActive = dynamicBoost;

  if (typeof data.real_fps === 'number') {
    currentRealFps = data.real_fps;
  }

  // 1. Synchronize Hardware Toggle Switch & Labels
  if (fpsToggleCheckbox && document.activeElement !== fpsToggleCheckbox && fpsToggleCheckbox.checked !== boosted) {
    fpsToggleCheckbox.checked = boosted;
  }

  if (labelFps5 && labelFps60) {
    if (isPaused) {
      labelFps5.className = "fps-switch-label";
      labelFps60.className = "fps-switch-label";
    } else if (boosted) {
      labelFps5.classList.remove("active");
      labelFps60.className = "fps-switch-label active";
    } else if (dynamicBoost) {
      labelFps5.classList.remove("active");
      labelFps60.className = "fps-switch-label dynamic-active";
    } else {
      labelFps60.className = "fps-switch-label";
      labelFps5.className = "fps-switch-label active";
    }
  }

  lastFpsStateData = data;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  // 2. Synchronize Status Ribbon Pill (#vision-pill, #fps-badge, #vision-mode-text)
  if (fpsBadge && visionModeText) {
    if (isPaused) {
      fpsBadge.className = "mode-badge badge-paused";
      fpsBadge.textContent = "0 FPS";
      visionModeText.textContent = opMode === 1
        ? (isEn ? "Player Lookahead" : "Плеер Lookahead")
        : (isEn ? "Standby (Paused)" : "Standby (Пауза)");
    } else if (dynamicBoost) {
      fpsBadge.className = "mode-badge badge-dynamic-boost";
      const displayFps = currentRealFps > 0 ? Math.round(currentRealFps) : 60;
      fpsBadge.textContent = `${displayFps} FPS`;
      visionModeText.textContent = isEn ? "Dynamic Boost" : "Динамический разгон";
    } else if (boosted) {
      fpsBadge.className = "mode-badge badge-boosted";
      const displayFps = currentRealFps > 0 ? Math.round(currentRealFps) : 60;
      fpsBadge.textContent = `${displayFps} FPS`;
      visionModeText.textContent = isEn ? "Boosted (60 FPS)" : "Усиленный (60 FPS)";
    } else {
      fpsBadge.className = "mode-badge badge-idle";
      const displayFps = currentRealFps > 0 ? Math.round(currentRealFps) : 5;
      fpsBadge.textContent = `${displayFps} FPS`;
      visionModeText.textContent = isEn ? "Idle Scan (5 FPS)" : "Фоновый скан (5 FPS)";
    }
  }

  // 3. Synchronize Telemetry Stat Item (#fps-stat-val)
  if (fpsStatVal) {
    if (isPaused) {
      fpsStatVal.textContent = opMode === 1
        ? (isEn ? "0 fps (Paused — Player Analysis)" : "0 кадр/сек (Пауза — Анализ плеера)")
        : (isEn ? "0 fps (Paused — Standby)" : "0 кадр/сек (Пауза — Standby)");
      fpsStatVal.className = "stat-value stat-muted";
    } else if (dynamicBoost) {
      const fpsStr = currentRealFps > 0 ? currentRealFps.toFixed(1) : "60.0";
      fpsStatVal.textContent = isEn ? `${fpsStr} fps (AI Boost 60 FPS)` : `${fpsStr} кадр/сек (Разгон ИИ 60 FPS)`;
      fpsStatVal.className = "stat-value stat-danger";
    } else if (boosted) {
      const fpsStr = currentRealFps > 0 ? currentRealFps.toFixed(1) : "60.0";
      fpsStatVal.textContent = isEn ? `${fpsStr} fps (Boosted 60 FPS)` : `${fpsStr} кадр/сек (Усиленный 60 FPS)`;
      fpsStatVal.className = "stat-value stat-warning";
    } else {
      const fpsStr = currentRealFps > 0 ? currentRealFps.toFixed(1) : "5.0";
      fpsStatVal.textContent = isEn ? `${fpsStr} fps (Idle 5 FPS)` : `${fpsStr} кадр/сек (Фоновый 5 FPS)`;
      fpsStatVal.className = "stat-value stat-primary";
    }
  }
}

// Emergency Visual Censor 3-second Countdown (Line Shrinks Right to Left)
let muteCountdownTimer = null;
function startEmergencyMuteCountdown(durationMs = 3000) {
  if (muteCountdownTimer) {
    clearInterval(muteCountdownTimer);
  }

  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (shieldPillStatus) shieldPillStatus.innerText = isEn ? "CENSOR!" : "ЦЕНЗУРА!";

  // Restart CSS animation smoothly
  muteProgressWrap.classList.remove("active");
  void muteProgressLine.offsetWidth; // reflow
  muteProgressWrap.classList.add("active");

  const startTime = Date.now();
  const endTime = startTime + durationMs;

  const updateText = () => {
    const now = Date.now();
    const remaining = Math.max(0, endTime - now);
    const _isEnMute = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    muteCountdownText.innerText = _isEnMute ? `Censor: ${(remaining / 1000).toFixed(1)}s` : `Цензура: ${(remaining / 1000).toFixed(1)}с`;

    if (remaining <= 0) {
      clearInterval(muteCountdownTimer);
      muteCountdownTimer = null;
      const _isEnRest = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
      muteCountdownText.innerText = _isEnRest ? "Restored" : "Восстановлено";
      if (shieldPillStatus) {
        const isShieldOn = lastShieldState ? !!lastShieldState.enabled : true;
        const shieldDot = shieldPill ? shieldPill.querySelector(".status-dot") : null;
        setPillStatus(shieldPillStatus, shieldDot, isShieldOn);
      }
      setTimeout(() => {
        muteProgressWrap.classList.remove("active");
      }, 600);
    }
  };

  updateText();
  muteCountdownTimer = setInterval(updateText, 50);
}

// Render Censor Test Result (Conditional vs Forced)
function renderCensorTestResult(data) {
  const analysis = data.analysis || {};
  const scorePercent = Math.round((analysis.nsfw_score || 0) * 100);

  if (censorResultBanner) {
    censorResultBanner.style.display = "flex";
  }

  if (data.should_censor) {
    if (censorStatusBadge) {
      censorStatusBadge.className = "censor-status-badge badge-danger";
    }
    if (badgeStatusText) {
      badgeStatusText.innerText = "NSFW DETECTED";
    }
    if (censorScoreText) {
      const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
      censorScoreText.innerText = _isEn ? `Risk: ${scorePercent}%` : `Риск: ${scorePercent}%`;
    }
    if (censorDetailText) {
      const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
      censorDetailText.innerText = _isEn
        ? `${analysis.status_message || "Explicit content detected"} • Screen blocked by gradient in OBS for 3 sec!`
        : `${analysis.status_message || "Обнаружен непристойный контент"} • Экран перекрыт градиентом в OBS на 3 сек!`;
    }
    startEmergencyMuteCountdown(3000);
    {
      const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
      renderEventItem({
        type: "warning",
        timestamp: new Date().toTimeString().split(" ")[0],
        message: _isEn
          ? `[!] NSFW DETECTED: ${analysis.status_message || "Violation"} (Risk: ${scorePercent}%) • Screen blocked in OBS`
          : `[!] NSFW DETECTED: ${analysis.status_message || "Нарушение"} (Риск: ${scorePercent}%) • Экран перекрыт в OBS`
      });
    }
  } else {
    if (censorStatusBadge) {
      censorStatusBadge.className = "censor-status-badge badge-clean";
    }
    if (badgeStatusText) {
      badgeStatusText.innerText = "NO NSFW DETECTED";
    }
    if (censorScoreText) {
      const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
      censorScoreText.innerText = _isEn ? `Risk: ${scorePercent}%` : `Риск: ${scorePercent}%`;
    }
    if (censorDetailText) {
      const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
      censorDetailText.innerText = _isEn
        ? `Screen not blocked • Video stream clean, no explicit content (Risk: ${scorePercent}%).`
        : `Экран не перекрыт • Видеопоток чист, непристойный контент отсутствует (Риск: ${scorePercent}%).`;
    }
    {
      const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
      renderEventItem({
        type: "success",
        timestamp: new Date().toTimeString().split(" ")[0],
        message: _isEn
          ? `[✓] NO NSFW DETECTED: Stream clean (${scorePercent}% risk) • Screen not blocked`
          : `[✓] NO NSFW DETECTED: Видеопоток чист (${scorePercent}% риск) • Экран не перекрыт`
      });
    }
  }
}

function renderTestResult(data) {
  const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
  renderEventItem({
    type: data.is_banned ? "danger" : "success",
    timestamp: new Date().toTimeString().split(" ")[0],
    message: data.is_banned
      ? (_isEn ? `[TEST] Banned words found: ${data.matches ? data.matches.length : 0}` : `[ТЕСТ] Найдено запрещенных слов: ${data.matches ? data.matches.length : 0}`)
      : (_isEn ? "[TEST] No banned words detected" : "[ТЕСТ] Запрещенных слов не обнаружено")
  });
}

// ==========================================
// TWO DEDICATED TESTS: OCR vs NSFW
// ==========================================

// Render Dedicated Test 1: OCR / Banned words
function renderOcrTestResult(res) {
  if (!res) return;
  const activated = res.activated;
  const _isEn = (window.I18N && window.I18N.getLanguage() === "en");

  if (activated) {
    startEmergencyMuteCountdown(3000);
    renderEventItem({
      type: "warning",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: _isEn
        ? `[TEST OCR: TRIGGERED] Word: "${res.matched_item}" (rule: ${res.matched_rule}) • OBS screen blocked for 3 sec`
        : `[ТЕСТ OCR: СРАБОТАЛО] Слово: "${res.matched_item}" (правило: ${res.matched_rule}) • Экран OBS перекрыт на 3 сек`
    });
  } else {
    renderEventItem({
      type: "success",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: _isEn
        ? `[TEST OCR: CLEAN] No banned words detected • OBS screen NOT blocked`
        : `[ТЕСТ OCR: ЧИСТО] Запрещенные слова не обнаружены • Экран OBS НЕ перекрыт`
    });
  }
}

// Render Dedicated Test 2: NSFW Visual AI
function renderNsfwTestResult(res) {
  if (!res) return;
  const activated = res.activated;
  const percent = Math.round((res.score || 0) * 100);
  const _isEn = (window.I18N && window.I18N.getLanguage() === "en");

  if (activated) {
    startEmergencyMuteCountdown(3000);
    renderEventItem({
      type: "warning",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: _isEn
        ? `[TEST NSFW: TRIGGERED] Class: "${res.matched_item}" (confidence: ${percent}%) • OBS screen blocked for 3 sec`
        : `[ТЕСТ NSFW: СРАБОТАЛО] Класс: "${res.matched_item}" (вероятность: ${percent}%) • Экран OBS перекрыт на 3 сек`
    });
  } else {
    renderEventItem({
      type: "success",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: _isEn
        ? `[TEST NSFW: CLEAN] Risk: ${percent}% • OBS screen NOT blocked`
        : `[ТЕСТ NSFW: ЧИСТО] Риск: ${percent}% • Экран OBS НЕ перекрыт`
    });
  }
}

// 1. Run OCR Test (checks custom text or physical screen of chosen monitor)
function runOcrTest(customText) {
  const isScreenScan = customText === null;
  const query = isScreenScan ? "" : (customText !== undefined ? customText : "");
  const monIdx = monitorSelect ? parseInt(monitorSelect.value, 10) : 0;

  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "run_ocr_test",
      text: (!isScreenScan && query) ? query : null,
      monitor_index: monIdx
    }));
  }
}

// 2. Run NSFW Test (real screen scan vs 94% simulation)
function runNsfwTest(simulate = false) {
  const monIdx = monitorSelect ? parseInt(monitorSelect.value, 10) : 0;

  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "run_nsfw_test",
      simulate: simulate,
      monitor_index: monIdx
    }));
  }
}

// Button Click Listeners for Simulation
if (btnRunNsfwSim) {
  btnRunNsfwSim.addEventListener("click", () => runNsfwTest(true));
}

if (btnClearIncidents) {
  btnClearIncidents.addEventListener("click", () => {
    if (incidentsTbody) {
      incidentsTbody.innerHTML = `
        <tr class="empty-row" id="incidents-empty-row">
          <td colspan="6">
            <div class="empty-state-box">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
              <span>${(window.I18N && window.I18N.getLanguage() === "en") ? "Journal cleared. No censorship incidents recorded." : "Журнал очищен. Инцидентов цензуры пока не зафиксировано."}</span>
            </div>
          </td>
        </tr>
      `;
    }
    totalIncidents = 0;
    nsfwIncidents = 0;
    ocrIncidents = 0;
    cuesIncidents = 0;
    activeIncidents = 0;
    updateAuditStats();
  });
}

// OCR Quick Test Chips
document.querySelectorAll(".chip-btn[data-ocr]").forEach(chip => {
  chip.addEventListener("click", () => {
    const word = chip.dataset.ocr || chip.innerText.trim();
    if (ocrTestInput) ocrTestInput.value = word;
    runOcrTest(word);
  });
});

// OBS Auto-Setup Result: renders explicit success / partial / failure status
function handleObsSetupResult(data) {
  const checkSvg = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>`;
  const warnSvg = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>`;
  const resetSvg = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 3px;"><polygon points="5 3 19 12 5 21 5 3"/></svg>`;
  const success = data.success === true;
  const running = data.plugin_running === true;
  const _isEn = (window.I18N && window.I18N.getLanguage() === "en");

  // 1. Update modal confirm button and alert
  if (btnConfirmObsSetupContinue) {
    btnConfirmObsSetupContinue.disabled = false;
    if (success && running) {
      btnConfirmObsSetupContinue.className = "btn btn-success btn-sm";
      btnConfirmObsSetupContinue.innerHTML = `${checkSvg} <span>${_isEn ? "SUCCESS: plugin ON" : "УСПЕШНО: плагин ON"}</span>`;
    } else if (success) {
      btnConfirmObsSetupContinue.className = "btn btn-warning btn-sm";
      btnConfirmObsSetupContinue.innerHTML = `${warnSvg} <span>${_isEn ? "DLL installed, plugin silent" : "DLL установлена, плагин молчит"}</span>`;
    } else {
      btnConfirmObsSetupContinue.className = "btn btn-danger btn-sm";
      btnConfirmObsSetupContinue.innerHTML = `${warnSvg} <span>${_isEn ? "ERROR — Retry" : "ОШИБКА — Повторить"}</span>`;
    }
  }

  if (btnCancelObsSetupPrompt) {
    btnCancelObsSetupPrompt.textContent = _isEn ? "Close" : "Закрыть";
  }

  if (obsSetupResultAlert && obsSetupResultMsg) {
    obsSetupResultAlert.style.display = "block";
    if (success && running) {
      obsSetupResultAlert.className = "alert alert-success";
      obsSetupResultMsg.innerHTML = _isEn
        ? "<strong>Success!</strong> blewred plugin injected into OBS Studio, Censor Shield scene created, filter active."
        : "<strong>Успешно!</strong> Плагин blewred инжектирован в OBS Studio, заставка Censor Shield создана, фильтр активен.";
    } else if (success) {
      obsSetupResultAlert.className = "alert alert-warning";
      obsSetupResultMsg.innerHTML = _isEn
        ? "<strong>Partial success:</strong> DLL installed, but plugin not responding. Restart OBS Studio completely and add filter 'blewred AI Smart Shield'."
        : "<strong>Частичный успех:</strong> DLL установлена, но плагин не отвечает. Перезапустите OBS Studio целиком и добавьте фильтр 'blewred AI Smart Shield'.";
    } else {
      obsSetupResultAlert.className = "alert alert-danger";
      const errMsg = data.message || (_isEn ? "Failed to configure OBS. Make sure OBS Studio is running and port 4455 is open." : "Не удалось настроить OBS. Убедитесь, что OBS Studio запущена и порт 4455 открыт.");
      obsSetupResultMsg.innerHTML = `<strong>${_isEn ? "Error:" : "Ошибка:"}</strong> ${errMsg}`;
    }
  }

  // 2. Update Quickstart Step 4 card & button
  if (btnTriggerObsSetup) {
    btnTriggerObsSetup.disabled = false;
    if (success && running) {
      btnTriggerObsSetup.className = "btn btn-xs btn-success";
      btnTriggerObsSetup.innerHTML = `${checkSvg} <span style="margin-left: 3px;">${_isEn ? "SUCCESS: plugin ON" : "УСПЕШНО: плагин ON"}</span>`;
    } else if (success) {
      btnTriggerObsSetup.className = "btn btn-xs btn-warning";
      btnTriggerObsSetup.innerHTML = `${warnSvg} <span style="margin-left: 3px;">${_isEn ? "DLL installed" : "DLL установлена"}</span>`;
    } else {
      btnTriggerObsSetup.className = "btn btn-xs btn-danger";
      btnTriggerObsSetup.innerHTML = `${warnSvg} <span style="margin-left: 3px;">${_isEn ? "ERROR — Retry" : "ОШИБКА — Повторить"}</span>`;
    }
    setTimeout(() => {
      if (btnTriggerObsSetup) {
        btnTriggerObsSetup.className = "btn btn-xs btn-success";
        btnTriggerObsSetup.innerHTML = `${resetSvg}<span>${_isEn ? "Auto-Setup OBS" : "Автонастройка OBS"}</span>`;
      }
    }, 8000);
  }

  if (badgeStepObs && stepCardObs) {
    if (success) {
      badgeStepObs.className = "label label-success step-status-badge";
      badgeStepObs.textContent = running ? (_isEn ? "Connected" : "Подключено") : (_isEn ? "Ready (OK)" : "Готово (OK)");
      stepCardObs.classList.add("step-completed");
      try { localStorage.setItem("blewred_setup_obs_verified", "true"); } catch (e) { }
      if (typeof updateSetupGuideStatus === "function") {
        updateSetupGuideStatus({});
      }
    }
  }

  if (btnAutoSetupObs && btnAutoSetupObs !== btnTriggerObsSetup) {
    btnAutoSetupObs.disabled = false;
    if (success && running) {
      btnAutoSetupObs.innerHTML = `${checkSvg} <span style="color:#5cb85c">${_isEn ? "SUCCESS: plugin ON" : "УСПЕШНО: плагин ON"}</span>`;
    } else if (success) {
      btnAutoSetupObs.innerHTML = `${warnSvg} <span style="color:#f0ad4e">${_isEn ? "DLL installed, plugin silent" : "DLL установлена, но плагин молчит"}</span>`;
    } else {
      btnAutoSetupObs.innerHTML = `${warnSvg} <span style="color:#ff6b6b">${_isEn ? "ERROR — Retry" : "ОШИБКА — Повторить"}</span>`;
    }
  }

  if (success && socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: "refresh_obs_scenes" }));
  }

  if (data.message) {
    data.message.split("\n").forEach(line => {
      if (line.trim()) {
        renderEventItem({
          type: success ? (running ? "success" : "warning") : "danger",
          timestamp: new Date().toTimeString().split(" ")[0],
          message: line.trim()
        });
      }
    });
  }
  if (success && !running) {
    renderEventItem({
      type: "warning",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: _isEn
        ? "Plugin not responding (heartbeat 51798 missing): restart OBS Studio completely and add filter 'blewred AI Smart Shield' to capture source"
        : "Плагин не отвечает (heartbeat 51798 отсутствует): перезапустите OBS Studio целиком и добавьте на источник захвата фильтр 'blewred AI Smart Shield'"
    });
  }
  if (!success && !data.message) {
    renderEventItem({
      type: "danger",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: _isEn ? "Auto-Setup OBS FAILED: plugin not installed and not injected" : "Автонастройка OBS FAILED: плагин не установлен и не инжектирован"
    });
  }
}

let obsCheckInterval = null;

async function checkObsRunningState() {
  let isConnected = (obsStatusText && obsStatusText.innerText === "ON");
  let isRunning = false;
  let exeFound = true;

  if (window.__TAURI__ && window.__TAURI__.core) {
    try {
      const status = await window.__TAURI__.core.invoke("check_obs_status");
      if (status) {
        isConnected = !!status.is_connected;
        isRunning = !!status.is_running;
        exeFound = !!status.exe_found;
      }
    } catch (e) { }
  } else if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: "check_obs_status" }));
  }

  return { isConnected, isRunning, exeFound };
}

function updateObsPromptBadge(state) {
  if (!obsSetupPromptStatusBadge) return;
  const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
  const isUp = !!(state.isConnected || state.is_connected || state.isRunning || state.is_running);
  if (isUp) {
    obsSetupPromptStatusBadge.className = "label label-success";
    obsSetupPromptStatusBadge.textContent = (state.isConnected || state.is_connected)
      ? (_isEn ? "OBS connected (WebSocket ON)" : "OBS подключена (WebSocket ON)")
      : (_isEn ? "OBS Studio running" : "OBS Studio запущена");
    if (obsSetupPromptHint) {
      obsSetupPromptHint.textContent = _isEn ? "Ready to configure! Click \"Continue auto-setup\"" : "Готово к настройке! Нажмите «Продолжить автонастройку»";
      obsSetupPromptHint.style.color = "#4ade80";
    }
  } else {
    obsSetupPromptStatusBadge.className = "label label-default";
    obsSetupPromptStatusBadge.textContent = _isEn ? "OBS not running" : "OBS не открыта";
    if (obsSetupPromptHint) {
      obsSetupPromptHint.textContent = (state.exeFound !== false && state.exe_found !== false)
        ? (_isEn ? "or open OBS manually" : "или откройте OBS вручную")
        : (_isEn ? "launch OBS manually" : "запустите OBS вручную");
      obsSetupPromptHint.style.color = "#64748b";
    }
  }
}

// ============================================================================
// Modal Dialog Management (Bootstrap 3 Dark Edition)
// ============================================================================

function openModal(modalEl, backdropEl, onOpen) {
  if (!modalEl || !backdropEl) return;
  document.body.classList.add("modal-open");
  modalEl.style.display = "block";
  backdropEl.style.display = "block";
  void modalEl.offsetWidth;
  modalEl.classList.add("in");
  backdropEl.classList.add("in");
  if (typeof onOpen === "function") onOpen();
}

function closeModal(modalEl, backdropEl, onClose) {
  if (!modalEl || !backdropEl) return;
  modalEl.classList.remove("in");
  backdropEl.classList.remove("in");
  setTimeout(() => {
    modalEl.style.display = "none";
    backdropEl.style.display = "none";
    if (!document.querySelector(".modal.in")) {
      document.body.classList.remove("modal-open");
    }
    if (typeof onClose === "function") onClose();
  }, 200);
}

function openObsSetupPrompt() {
  openModal(modalObsSetupPrompt, modalObsSetupPromptBackdrop, async () => {
    const state = await checkObsRunningState();
    updateObsPromptBadge(state);
    if (obsCheckInterval) clearInterval(obsCheckInterval);
    obsCheckInterval = setInterval(async () => {
      const s = await checkObsRunningState();
      updateObsPromptBadge(s);
    }, 1500);
  });
}

function closeObsSetupPrompt() {
  if (obsCheckInterval) { clearInterval(obsCheckInterval); obsCheckInterval = null; }
  closeModal(modalObsSetupPrompt, modalObsSetupPromptBackdrop);
}

if (btnCloseObsSetupPromptX) btnCloseObsSetupPromptX.addEventListener("click", closeObsSetupPrompt);
if (btnCancelObsSetupPrompt) btnCancelObsSetupPrompt.addEventListener("click", closeObsSetupPrompt);
if (modalObsSetupPromptBackdrop) modalObsSetupPromptBackdrop.addEventListener("click", closeObsSetupPrompt);

if (btnModalLaunchObs) {
  btnModalLaunchObs.addEventListener("click", async () => {
    btnModalLaunchObs.disabled = true;
    const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
    btnModalLaunchObs.innerHTML = `<span>${_isEn ? "Launching OBS..." : "Запуск OBS..."}</span>`;
    if (obsSetupPromptHint) {
      obsSetupPromptHint.textContent = _isEn ? "Initializing OBS Studio, please wait..." : "Инициализация OBS Studio, пожалуйста подождите...";
      obsSetupPromptHint.style.color = "#38bdf8";
    }
    if (window.__TAURI__ && window.__TAURI__.core) {
      try { await window.__TAURI__.core.invoke("launch_obs_studio"); }
      catch (err) { if (obsSetupPromptHint) { obsSetupPromptHint.textContent = (_isEn ? "Launch error: " : "Ошибка запуска: ") + err; obsSetupPromptHint.style.color = "#ef4444"; } }
    } else if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "launch_obs" }));
    }
    setTimeout(() => {
      btnModalLaunchObs.disabled = false;
      btnModalLaunchObs.innerHTML = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg><span>${_isEn ? "Launch OBS Studio" : "Запустить OBS Studio"}</span>`;
    }, 4000);
  });
}

if (btnConfirmObsSetupContinue) {
  btnConfirmObsSetupContinue.addEventListener("click", () => {
    closeObsSetupPrompt();
    executeAutoSetupObs();
  });
}

async function executeAutoSetupObs() {
  const targetBtn = btnTriggerObsSetup || btnAutoSetupObs;
  if (!targetBtn) return;
  targetBtn.disabled = true;
  const _isEn = (window.I18N && window.I18N.getLanguage() === "en");
  targetBtn.innerHTML = `<span>${_isEn ? "Setting up OBS..." : "Настройка OBS..."}</span>`;
  renderEventItem({
    type: "system",
    timestamp: new Date().toTimeString().split(" ")[0],
    message: _isEn
      ? "OBS auto-setup request: DLL injection, Censor Shield deployment and GPU filter attachment..."
      : "Запрос автонастройки OBS Studio: инъекция DLL, развертывание Censor Shield и прикрепление GPU-фильтра..."
  });
  // Prefer Tauri IPC; fall back to WebSocket daemon. Never both — double auto-setup causes
  // concurrent DLL injections into obs64.exe.
  if (window.__TAURI__ && window.__TAURI__.core) {
    try {
      const res = await window.__TAURI__.core.invoke("auto_setup_obs");
      handleObsSetupResult(typeof res === "string" ? JSON.parse(res) : res);
    } catch (err) {
      handleObsSetupResult({ success: false, message: _isEn ? "OBS setup error: " + err : "Ошибка настройки OBS: " + err });
    }
  } else if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "auto_setup_obs"
    }));
  } else {
    handleObsSetupResult({ success: false, message: _isEn ? "Neither Tauri IPC nor WebSocket daemon (51789) available — setup impossible" : "Ни Tauri IPC, ни демон WebSocket (51789) недоступны — настройка невозможна" });
  }
}

// Auto-Setup OBS Button Click: checks state first; if closed, prompts to open OBS then continue
function handleObsSetupBtnClick() {
  checkObsRunningState().then(state => {
    if (state.isConnected || state.isRunning) {
      executeAutoSetupObs();
    } else {
      openObsSetupPrompt();
    }
  });
}

if (btnTriggerObsSetup) {
  btnTriggerObsSetup.addEventListener("click", handleObsSetupBtnClick);
}
if (btnAutoSetupObs && btnAutoSetupObs !== btnTriggerObsSetup) {
  btnAutoSetupObs.addEventListener("click", handleObsSetupBtnClick);
}

// Protection is permanently active when application is running

let lastUserShieldChangeTime = 0;
let lastShieldState = null;

function updateCensorShieldUI(enabled, isVisible) {
  const visible = !!isVisible;
  lastShieldState = { enabled, isVisible: visible };
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  if (shieldPillStatus) {
    const isShieldOn = !!enabled;
    const shieldDot = shieldPill ? shieldPill.querySelector(".status-dot") : null;
    setPillStatus(shieldPillStatus, shieldDot, isShieldOn);
  }
  if (obsShieldStat) {
    if (visible) {
      obsShieldStat.textContent = isEn ? "ON — overlay active in OBS" : "ON — заставка показана в OBS";
      obsShieldStat.style.color = "#fbbf24";
    } else {
      obsShieldStat.textContent = enabled
        ? (isEn ? "READY — primed on violation" : "READY — готов к показу при нарушениях")
        : (isEn ? "OFF — disabled" : "OFF — выключен");
      obsShieldStat.style.color = enabled ? "#ffffff" : "#94a3b8";
    }
  }
}

// Fullscreen Censor Shield Toggle Switch Listener
if (toggleCensorShield) {
  try {
    const saved = localStorage.getItem("blewred_censor_shield_enabled");
    toggleCensorShield.checked = (saved === null || saved === "true");
  } catch (err) { }
  updateCensorShieldUI(toggleCensorShield.checked, false);

  toggleCensorShield.addEventListener("change", (e) => {
    const enabled = e.target.checked;
    lastUserShieldChangeTime = Date.now();
    try {
      localStorage.setItem("blewred_censor_shield_enabled", enabled ? "true" : "false");
    } catch (err) { }
    updateCensorShieldUI(enabled, false);
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    renderEventItem({
      type: enabled ? "system" : "system",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn
        ? `Censor Shield overlay: ${enabled ? "ENABLED (Primed on violation)" : "DISABLED"}`
        : `Заставка Censor Shield: ${enabled ? "ВКЛЮЧЕНА (Готова к показу при нарушениях)" : "ВЫКЛЮЧЕНА"}`
    });
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "toggle_censor_shield",
        enabled: enabled
      }));
    }
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("toggle_censor_shield", { enabled: enabled }).catch(() => { });
    }
  });
}

function applyShieldDonateChange(enabled) {
  if (toggleShieldDonate) {
    toggleShieldDonate.checked = enabled;
  }
  try {
    localStorage.setItem("blewred_shield_donate", enabled ? "true" : "false");
  } catch (err) { }
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  renderEventItem({
    type: "system",
    timestamp: new Date().toTimeString().split(" ")[0],
    message: isEn
      ? `Developer support banner on censor shield: ${enabled ? "ENABLED (web.tribute.tg/e/1dW)" : "DISABLED (hidden)"}`
      : `Поддержка разработки на заставке: ${enabled ? "ВКЛЮЧЕНА (web.tribute.tg/e/1dW)" : "ОТКЛЮЧЕНА (скрыта)"}`
  });
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "toggle_shield_donate",
      enabled: enabled
    }));
  }
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("toggle_shield_donate", { enabled: enabled }).catch(() => { });
  }
}

function openDisableSupportConfirmModal() {
  openModal(modalDisableSupportConfirm, modalDisableSupportBackdrop, () => {
    if (btnCancelDisableSupport) {
      btnCancelDisableSupport.focus();
    }
  });
}

function closeDisableSupportConfirmModal() {
  closeModal(modalDisableSupportConfirm, modalDisableSupportBackdrop);
}

function cancelDisableSupport() {
  closeDisableSupportConfirmModal();
  if (toggleShieldDonate) {
    toggleShieldDonate.checked = true;
  }
}

// Censor Shield Developer Support Link Toggle Switch Listener
if (toggleShieldDonate) {
  try {
    const saved = localStorage.getItem("blewred_shield_donate");
    if (saved !== null) {
      toggleShieldDonate.checked = (saved !== "false");
    } else {
      toggleShieldDonate.checked = true;
    }
  } catch (err) { }

  toggleShieldDonate.addEventListener("click", (e) => {
    if (!e.target.checked) {
      // User is attempting to disable developer support
      e.preventDefault();
      toggleShieldDonate.checked = true;
      openDisableSupportConfirmModal();
      return;
    }
    // User is enabling developer support
    applyShieldDonateChange(true);
  });
}

if (btnConfirmDisableSupport) {
  btnConfirmDisableSupport.addEventListener("click", () => {
    closeDisableSupportConfirmModal();
    applyShieldDonateChange(false);
  });
}
if (btnCancelDisableSupport) {
  btnCancelDisableSupport.addEventListener("click", cancelDisableSupport);
}
if (btnCloseDisableSupportX) {
  btnCloseDisableSupportX.addEventListener("click", cancelDisableSupport);
}
if (modalDisableSupportBackdrop) {
  modalDisableSupportBackdrop.addEventListener("click", cancelDisableSupport);
}

// NSFW Sensitivity Threshold Slider Listeners
if (nsfwThresholdSlider) {
  nsfwThresholdSlider.addEventListener("input", (e) => {
    const val = parseInt(e.target.value, 10);
    formatSensitivityUI(val);
    setThresholdMarker(val);
    // Send live updates via WebSocket while user drags the slider
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "set_nsfw_threshold",
        threshold: val
      }));
    }
  });
  nsfwThresholdSlider.addEventListener("change", (e) => {
    const val = parseInt(e.target.value, 10);
    try {
      localStorage.setItem("blewred_nsfw_threshold", val.toString());
    } catch (err) { }
    // Final commit via Tauri IPC on release
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("set_nsfw_threshold", { threshold: val }).catch(() => { });
    }
  });
}

// Selective Censor Category Badges ("Favorite Content") Listeners
document.querySelectorAll(".censor-chip input[type='checkbox']").forEach(input => {
  input.addEventListener("change", (e) => {
    e.target.closest(".censor-chip")?.classList.toggle("active", e.target.checked);
    sendCensorCategories();
  });
});

// FPS Toggle Switch Listeners
if (fpsToggleCheckbox) {
  fpsToggleCheckbox.addEventListener("change", (e) => {
    setFpsMode(e.target.checked);
  });
}

if (labelFps5) {
  labelFps5.addEventListener("click", () => setFpsMode(false));
}
if (labelFps60) {
  labelFps60.addEventListener("click", () => setFpsMode(true));
}

// Unified Dynamic Hotkeys & Multi-Key Combinations Engine
let recordingHotkeyTarget = null; // "panic" | "threat" | null
let hotkeyRecordingSession = null;

function getKeyDisplayName(e) {
  if (!e) return "F9";
  const code = e.code || "";
  const key = e.key || "";

  // Function keys F1 - F24
  if (code.startsWith("F") && code.length <= 3 && !isNaN(parseInt(code.slice(1)))) {
    return code.toUpperCase();
  }
  if (key.startsWith("F") && key.length <= 3 && !isNaN(parseInt(key.slice(1)))) {
    return key.toUpperCase();
  }

  // Letters: use code to be independent of keyboard layout (e.g. Russian vs English)
  if (code.startsWith("Key") && code.length === 4) {
    return code.slice(3).toUpperCase();
  }

  // Digits: 0-9
  if (code.startsWith("Digit") && code.length === 6) {
    return code.slice(5);
  }

  // Numpad digits: Numpad0 -> 0
  if (code.startsWith("Numpad") && !isNaN(parseInt(code.slice(6)))) {
    return code.slice(6);
  }

  // Special named keys
  switch (code) {
    case "Space": return "SPACE";
    case "Tab": return "TAB";
    case "Enter": return "ENTER";
    case "NumpadEnter": return "ENTER";
    case "Escape": return "ESC";
    case "Backspace": return "BACKSPACE";
    case "Delete": return "DELETE";
    case "Insert": return "INSERT";
    case "Home": return "HOME";
    case "End": return "END";
    case "PageUp": return "PAGEUP";
    case "PageDown": return "PAGEDOWN";
    case "Pause": return "PAUSE";
    case "ArrowUp": return "UP";
    case "ArrowDown": return "DOWN";
    case "ArrowLeft": return "LEFT";
    case "ArrowRight": return "RIGHT";
    default:
      break;
  }

  if (key === " ") return "SPACE";
  if (key.length === 1) return key.toUpperCase();
  return key.toUpperCase();
}

function normalizeKeyName(e) {
  return getKeyDisplayName(e);
}

function getDetailedKeyName(e) {
  if (!e) return "F9";
  const code = e.code || "";
  if (code === "ControlLeft") return "Left Ctrl";
  if (code === "ControlRight") return "Right Ctrl";
  if (code === "ShiftLeft") return "Left Shift";
  if (code === "ShiftRight") return "Right Shift";
  if (code === "AltLeft") return "Left Alt";
  if (code === "AltRight") return "Right Alt";
  if (code === "MetaLeft" || code === "OSLeft") return "Left Win";
  if (code === "MetaRight" || code === "OSRight") return "Right Win";

  return getKeyDisplayName(e);
}

function isModifierEvent(e) {
  if (!e) return false;
  const k = (e.key || "").toUpperCase();
  const c = (e.code || "").toUpperCase();
  return k === "CONTROL" || k === "SHIFT" || k === "ALT" || k === "META" || k === "OS" ||
    c.startsWith("CONTROL") || c.startsWith("SHIFT") || c.startsWith("ALT") || c.startsWith("META") || c.startsWith("OS");
}

function parseHotkeyCombo(combo) {
  const res = { ctrl: false, alt: false, shift: false, meta: false, key: "F9", raw: combo };
  if (!combo) return res;
  const parts = String(combo).replace(/-/g, "+").split("+").map(s => s.trim()).filter(Boolean);
  if (parts.length === 0) return res;

  if (parts.length === 1) {
    const single = parts[0];
    const upper = single.toUpperCase();
    if (upper === "LEFT CTRL" || upper === "LCTRL" || upper === "L-CTRL" || upper === "CONTROLLEFT") {
      res.ctrl = true;
      res.key = "Left Ctrl";
    } else if (upper === "RIGHT CTRL" || upper === "RCTRL" || upper === "R-CTRL" || upper === "CONTROLRIGHT") {
      res.ctrl = true;
      res.key = "Right Ctrl";
    } else if (upper === "CTRL" || upper === "CONTROL") {
      res.ctrl = true;
      res.key = "Ctrl";
    } else if (upper === "LEFT SHIFT" || upper === "LSHIFT" || upper === "SHIFTLEFT") {
      res.shift = true;
      res.key = "Left Shift";
    } else if (upper === "RIGHT SHIFT" || upper === "RSHIFT" || upper === "SHIFTRIGHT") {
      res.shift = true;
      res.key = "Right Shift";
    } else if (upper === "SHIFT") {
      res.shift = true;
      res.key = "Shift";
    } else if (upper === "LEFT ALT" || upper === "LALT" || upper === "ALTLEFT") {
      res.alt = true;
      res.key = "Left Alt";
    } else if (upper === "RIGHT ALT" || upper === "RALT" || upper === "ALTRIGHT") {
      res.alt = true;
      res.key = "Right Alt";
    } else if (upper === "ALT") {
      res.alt = true;
      res.key = "Alt";
    } else if (upper === "LEFT WIN" || upper === "LWIN" || upper === "OSLEFT" || upper === "METALEFT") {
      res.meta = true;
      res.key = "Left Win";
    } else if (upper === "RIGHT WIN" || upper === "RWIN" || upper === "OSRIGHT" || upper === "METARIGHT") {
      res.meta = true;
      res.key = "Right Win";
    } else if (upper === "WIN" || upper === "META") {
      res.meta = true;
      res.key = "Win";
    } else {
      res.key = upper === "FN" ? "F9" : single;
    }
    return res;
  }

  let baseKey = "";
  for (const part of parts) {
    const p = part.toUpperCase();
    if (p === "CTRL" || p === "CONTROL" || p === "LEFT CTRL" || p === "RIGHT CTRL" || p === "LCTRL" || p === "RCTRL") {
      res.ctrl = true;
    } else if (p === "ALT" || p === "LEFT ALT" || p === "RIGHT ALT" || p === "LALT" || p === "RALT") {
      res.alt = true;
    } else if (p === "SHIFT" || p === "LEFT SHIFT" || p === "RIGHT SHIFT" || p === "LSHIFT" || p === "RSHIFT") {
      res.shift = true;
    } else if (p === "WIN" || p === "META" || p === "SUPER" || p === "OS" || p === "LEFT WIN" || p === "RIGHT WIN") {
      res.meta = true;
    } else {
      baseKey = part.toUpperCase();
      if (baseKey === "FN") baseKey = "F9";
    }
  }
  res.key = baseKey || parts[parts.length - 1];
  return res;
}

function formatCanonicalCombo(parsed) {
  if (!parsed) return "F9";
  if (typeof parsed === "string") {
    parsed = parseHotkeyCombo(parsed);
  }
  const keyUpper = (parsed.key || "").toUpperCase();
  if (keyUpper === "LEFT CTRL" || keyUpper === "RIGHT CTRL" || keyUpper === "CTRL" ||
    keyUpper === "LEFT SHIFT" || keyUpper === "RIGHT SHIFT" || keyUpper === "SHIFT" ||
    keyUpper === "LEFT ALT" || keyUpper === "RIGHT ALT" || keyUpper === "ALT" ||
    keyUpper === "LEFT WIN" || keyUpper === "RIGHT WIN" || keyUpper === "WIN") {
    const parts = [];
    if (parsed.ctrl && !keyUpper.includes("CTRL")) parts.push("Ctrl");
    if (parsed.alt && !keyUpper.includes("ALT")) parts.push("Alt");
    if (parsed.shift && !keyUpper.includes("SHIFT")) parts.push("Shift");
    if (parsed.meta && !keyUpper.includes("WIN") && !keyUpper.includes("META")) parts.push("Win");
    parts.push(parsed.key);
    return parts.join(" + ");
  }
  const parts = [];
  if (parsed.ctrl) parts.push("Ctrl");
  if (parsed.alt) parts.push("Alt");
  if (parsed.shift) parts.push("Shift");
  if (parsed.meta) parts.push("Win");
  parts.push(parsed.key || "F9");
  return parts.join(" + ");
}

function renderKeyComboBadge(combo) {
  if (!combo) return '<span class="btn-kbd">F9</span>';
  const rawStr = typeof combo === "string" ? combo : formatCanonicalCombo(combo);
  const parts = rawStr.replace(/-/g, "+").split("+").map(s => s.trim()).filter(Boolean);
  return parts
    .map(p => `<span class="btn-kbd">${p}</span>`)
    .join(' <span class="hotkey-combo-sep">+</span> ');
}

function matchesHotkey(e, targetCombo) {
  if (!targetCombo) return false;
  const parsed = parseHotkeyCombo(targetCombo);
  const targetKeyUpper = (parsed.key || "").toUpperCase();

  // Standalone modifier check
  if (targetKeyUpper === "LEFT CTRL" || targetKeyUpper === "LCTRL" || targetKeyUpper === "CONTROLLEFT") {
    return e.code === "ControlLeft";
  }
  if (targetKeyUpper === "RIGHT CTRL" || targetKeyUpper === "RCTRL" || targetKeyUpper === "CONTROLRIGHT") {
    return e.code === "ControlRight";
  }
  if (targetKeyUpper === "CTRL" || targetKeyUpper === "CONTROL") {
    return e.code === "ControlLeft" || e.code === "ControlRight" || e.key === "Control";
  }
  if (targetKeyUpper === "LEFT SHIFT" || targetKeyUpper === "SHIFTLEFT") {
    return e.code === "ShiftLeft";
  }
  if (targetKeyUpper === "RIGHT SHIFT" || targetKeyUpper === "SHIFTRIGHT") {
    return e.code === "ShiftRight";
  }
  if (targetKeyUpper === "SHIFT") {
    return e.code === "ShiftLeft" || e.code === "ShiftRight" || e.key === "Shift";
  }
  if (targetKeyUpper === "LEFT ALT" || targetKeyUpper === "ALTLEFT") {
    return e.code === "AltLeft";
  }
  if (targetKeyUpper === "RIGHT ALT" || targetKeyUpper === "ALTRIGHT") {
    return e.code === "AltRight";
  }
  if (targetKeyUpper === "ALT") {
    return e.code === "AltLeft" || e.code === "AltRight" || e.key === "Alt";
  }
  if (targetKeyUpper === "LEFT WIN" || targetKeyUpper === "OSLEFT" || targetKeyUpper === "METALEFT") {
    return e.code === "MetaLeft" || e.code === "OSLeft";
  }
  if (targetKeyUpper === "RIGHT WIN" || targetKeyUpper === "OSRIGHT" || targetKeyUpper === "METARIGHT") {
    return e.code === "MetaRight" || e.code === "OSRight";
  }
  if (targetKeyUpper === "WIN" || targetKeyUpper === "META") {
    return e.code === "MetaLeft" || e.code === "MetaRight" || e.code === "OSLeft" || e.code === "OSRight";
  }

  // Multi-modifier combo where the last key is a modifier (e.g. "Ctrl + Shift")
  if (targetKeyUpper === "SHIFT" || targetKeyUpper === "LEFT SHIFT" || targetKeyUpper === "RIGHT SHIFT") {
    if (parsed.ctrl && !e.ctrlKey) return false;
    if (parsed.alt && !e.altKey) return false;
    if (parsed.meta && !e.metaKey) return false;
    return e.code.startsWith("Shift") || e.key === "Shift";
  }
  if (targetKeyUpper === "CTRL" || targetKeyUpper === "LEFT CTRL" || targetKeyUpper === "RIGHT CTRL") {
    if (parsed.shift && !e.shiftKey) return false;
    if (parsed.alt && !e.altKey) return false;
    if (parsed.meta && !e.metaKey) return false;
    return e.code.startsWith("Control") || e.key === "Control";
  }
  if (targetKeyUpper === "ALT" || targetKeyUpper === "LEFT ALT" || targetKeyUpper === "RIGHT ALT") {
    if (parsed.ctrl && !e.ctrlKey) return false;
    if (parsed.shift && !e.shiftKey) return false;
    if (parsed.meta && !e.metaKey) return false;
    return e.code.startsWith("Alt") || e.key === "Alt";
  }

  // Multi-key combination
  if (!!e.ctrlKey !== parsed.ctrl) return false;
  if (!!e.altKey !== parsed.alt) return false;
  if (!!e.shiftKey !== parsed.shift) return false;
  if (!!e.metaKey !== parsed.meta) return false;

  const pressedKey = normalizeKeyName(e).toUpperCase();
  return pressedKey === targetKeyUpper;
}

function buildComboString(session) {
  if (!session) return "";
  const parts = [];
  if (session.modifiers.ctrl) parts.push("Ctrl");
  if (session.modifiers.alt) parts.push("Alt");
  if (session.modifiers.shift) parts.push("Shift");
  if (session.modifiers.meta) parts.push("Win");

  if (session.primaryKey) {
    parts.push(session.primaryKey);
    return parts.join(" + ");
  }

  if (parts.length > 1) {
    return parts.join(" + ");
  }
  if (parts.length === 1) {
    return session.singleModifierName || parts[0];
  }

  return "";
}

function updateRecordingBadge(session) {
  if (!session || !session.target) return;
  const targetBadge = session.target === "panic"
    ? document.getElementById("badge-hotkey-panic")
    : document.getElementById("badge-hotkey-threat");
  if (!targetBadge || !session.latestCombo) return;

  const parts = session.latestCombo.replace(/-/g, "+").split("+").map(s => s.trim()).filter(Boolean);
  const badgesHtml = parts.map(p => `<span class="btn-kbd" style="border-color: #f59e0b; color: #fbbf24; background: #292010;">${p}</span>`).join(' <span class="hotkey-combo-sep">+</span> ');
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const releaseHint = isEn ? "release any key to save" : "отпустите любую клавишу";
  const hintHtml = ` <span class="text-warning" style="font-size: 10.5px; margin-left: 6px;">(${releaseHint})</span>`;
  targetBadge.innerHTML = badgesHtml + hintHtml;
}

function startHotkeyRecording(target) {
  if (recordingHotkeyTarget === target) {
    cancelHotkeyRecording();
    return;
  }
  cancelHotkeyRecording();

  recordingHotkeyTarget = target;
  hotkeyRecordingSession = {
    target: target,
    modifiers: {
      ctrl: false,
      alt: false,
      shift: false,
      meta: false
    },
    singleModifierName: null,
    primaryKey: null,
    keysPressedCount: 0,
    latestCombo: ""
  };

  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const recText = isEn ? "Recording..." : "Запись...";
  const promptText = isEn
    ? "Press key combination (e.g. Ctrl+Shift+F9)..."
    : "Нажмите комбинацию (напр. Ctrl+Shift+F9)...";

  if (target === "panic") {
    if (btnRebindPanic) btnRebindPanic.classList.add("btn-warning");
    if (btnRebindThreat) btnRebindThreat.classList.remove("btn-warning");
    const lblPanic = document.getElementById("lbl-rebind-panic-text");
    if (lblPanic) lblPanic.innerText = recText;
    if (badgeHotkeyPanic) {
      badgeHotkeyPanic.innerHTML = `<span class="label label-warning" style="font-size: 11px;"><i class="glyphicon glyphicon-record" style="font-size: 9px; margin-right: 4px;"></i>${promptText}</span>`;
    }
  } else if (target === "threat") {
    if (btnRebindThreat) btnRebindThreat.classList.add("btn-warning");
    if (btnRebindPanic) btnRebindPanic.classList.remove("btn-warning");
    const lblThreat = document.getElementById("lbl-rebind-threat-text");
    if (lblThreat) lblThreat.innerText = recText;
    if (badgeHotkeyThreat) {
      badgeHotkeyThreat.innerHTML = `<span class="label label-warning" style="font-size: 11px;"><i class="glyphicon glyphicon-record" style="font-size: 9px; margin-right: 4px;"></i>${promptText}</span>`;
    }
  }
}

function cancelHotkeyRecording() {
  if (!recordingHotkeyTarget && !hotkeyRecordingSession) return;
  hotkeyRecordingSession = null;
  recordingHotkeyTarget = null;

  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const recordText = isEn ? "Record" : "Записать";

  if (btnRebindPanic) btnRebindPanic.classList.remove("btn-warning");
  if (btnRebindThreat) btnRebindThreat.classList.remove("btn-warning");
  const lblPanic = document.getElementById("lbl-rebind-panic-text");
  const lblThreat = document.getElementById("lbl-rebind-threat-text");
  if (lblPanic) lblPanic.innerText = recordText;
  if (lblThreat) lblThreat.innerText = recordText;

  updateHotkeySettingsUI(currentHotkeyScope, currentHotkeyPanic, currentHotkeyThreat);
}

function finalizeHotkeyRecording() {
  if (!hotkeyRecordingSession || !recordingHotkeyTarget) return;

  const target = recordingHotkeyTarget;
  const combo = hotkeyRecordingSession.latestCombo;

  if (combo) {
    if (target === "panic") {
      currentHotkeyPanic = combo;
    } else if (target === "threat") {
      currentHotkeyThreat = combo;
    }
  }

  cancelHotkeyRecording();
}

async function toggleEmergencyShield(reason = "Manual Hotkey Trigger") {
  try {
    if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === "function") {
      const active = await window.__TAURI__.core.invoke("toggle_emergency_shield", { reason });
      console.log("[Hotkeys] toggle_emergency_shield invoked via Tauri:", active);
      return active;
    }
  } catch (err) {
    console.warn("[Hotkeys] Tauri invoke toggle_emergency_shield failed:", err);
  }

  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "toggle_emergency_shield",
      reason: reason
    }));
  }
}

window.addEventListener("keydown", (e) => {
  if (recordingHotkeyTarget && hotkeyRecordingSession) {
    e.preventDefault();
    e.stopPropagation();

    // Cancel recording on Escape if no other keys were pressed
    if (e.key === "Escape" && hotkeyRecordingSession.keysPressedCount === 0) {
      cancelHotkeyRecording();
      return;
    }

    if (e.repeat) return;

    hotkeyRecordingSession.keysPressedCount++;

    const code = e.code || "";
    const isCtrl = e.ctrlKey || code.startsWith("Control");
    const isAlt = e.altKey || code.startsWith("Alt");
    const isShift = e.shiftKey || code.startsWith("Shift");
    const isMeta = e.metaKey || code.startsWith("Meta") || code.startsWith("OS");

    if (isCtrl) hotkeyRecordingSession.modifiers.ctrl = true;
    if (isAlt) hotkeyRecordingSession.modifiers.alt = true;
    if (isShift) hotkeyRecordingSession.modifiers.shift = true;
    if (isMeta) hotkeyRecordingSession.modifiers.meta = true;

    if (isModifierEvent(e)) {
      hotkeyRecordingSession.singleModifierName = getDetailedKeyName(e);
    } else {
      hotkeyRecordingSession.primaryKey = getKeyDisplayName(e);
    }

    hotkeyRecordingSession.latestCombo = buildComboString(hotkeyRecordingSession);
    updateRecordingBadge(hotkeyRecordingSession);
    return;
  }

  // Active Key Triggering with Modifier Support
  if (matchesHotkey(e, currentHotkeyPanic)) {
    e.preventDefault();
    if (activeLookaheadTicket) {
      sendLookaheadDecision("block");
    } else {
      toggleEmergencyShield("Manual Hotkey Trigger");
    }
  } else if (matchesHotkey(e, currentHotkeyThreat)) {
    e.preventDefault();
    if (activeLookaheadTicket) {
      sendLookaheadDecision("allow");
    } else {
      setFpsMode(!isBoosted);
    }
  }
});

// Keyup listener: finalize recording immediately when ANY key is released
window.addEventListener("keyup", (e) => {
  if (recordingHotkeyTarget && hotkeyRecordingSession) {
    e.preventDefault();
    e.stopPropagation();

    if (hotkeyRecordingSession.keysPressedCount > 0 && hotkeyRecordingSession.latestCombo) {
      finalizeHotkeyRecording();
    }
  }
});

// Cancel recording if window loses focus
window.addEventListener("blur", () => {
  if (recordingHotkeyTarget) {
    cancelHotkeyRecording();
  }
});

/**
 * Strictly renders detected video/OCR banned words
 */
function renderBannedAlert(alert) {
  const container = document.getElementById("live-speech-box");
  if (!container) return;

  const placeholder = container.querySelector(".speech-placeholder");
  if (placeholder) {
    placeholder.remove();
  }

  const _isEnBanned = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const sourceLabel = alert.source === "ocr"
    ? (_isEnBanned ? "OCR TEXT" : "OCR ТЕКСТ")
    : (_isEnBanned ? "SCREEN CAPTURE" : "ЭКРАННЫЙ ЗАХВАТ");
  const sourceClass = alert.source === "ocr" ? "source-ocr" : "source-screen";
  const matchWords = (alert.matches || [alert.word || ""]).join(", ");

  const card = document.createElement("div");
  card.className = "speech-item banned";

  const header = document.createElement("div");
  header.className = "speech-header";

  const tag = document.createElement("span");
  tag.className = `speech-source-tag ${sourceClass}`;
  tag.innerText = sourceLabel;

  const status = document.createElement("span");
  status.className = "speech-status status-banned";
  status.innerHTML = _isEnBanned
    ? `STOPWORD: <strong>${matchWords}</strong> (CENSOR OBS)`
    : `СТОП-СЛОВО: <strong>${matchWords}</strong> (CENSOR OBS)`;

  header.appendChild(tag);
  header.appendChild(status);

  const textDiv = document.createElement("div");
  textDiv.className = "speech-text";
  textDiv.innerText = alert.text ? `"${alert.text}"` : (_isEnBanned ? `Detected in frame: ${matchWords}` : `Обнаружено в кадре: ${matchWords}`);

  card.appendChild(header);
  card.appendChild(textDiv);

  container.insertBefore(card, container.firstChild);

  while (container.children.length > 25) {
    container.removeChild(container.lastChild);
  }
}

function renderEventItem(ev) {
  if (!eventFeed) {
    console.log(`[EVENT:${ev.type || "system"}] ${ev.message || ""}`);
    return;
  }
  const item = document.createElement("div");
  item.className = `event-item ${ev.type || "system"}`;

  const timeSpan = document.createElement("span");
  timeSpan.className = "event-time";
  timeSpan.innerText = ev.timestamp || new Date().toTimeString().split(" ")[0];

  const tagSpan = document.createElement("span");
  tagSpan.className = "event-tag";
  tagSpan.innerText = (ev.type || "INFO").toUpperCase();

  const msgSpan = document.createElement("span");
  msgSpan.className = "event-msg";
  msgSpan.innerText = ev.message || "";

  item.appendChild(timeSpan);
  item.appendChild(tagSpan);
  item.appendChild(msgSpan);

  eventFeed.insertBefore(item, eventFeed.firstChild);

  while (eventFeed.children.length > 50) {
    eventFeed.removeChild(eventFeed.lastChild);
  }
}

// Stopwords Modal Controls (Bootstrap 3 Dark Edition)
const modalStopwords = document.getElementById("modal-stopwords");
const modalStopwordsBackdrop = document.getElementById("modal-stopwords-backdrop");
const btnOpenStopwordsModal = document.getElementById("btn-open-stopwords-modal");
const btnCloseStopwords = document.getElementById("btn-close-stopwords");
const btnCloseStopwordsX = document.getElementById("btn-close-stopwords-x");

function openStopwordsModal() {
  openModal(modalStopwords, modalStopwordsBackdrop, () => {
    if (stopwordsTextarea) stopwordsTextarea.focus();
  });
}

function closeStopwordsModal() {
  closeModal(modalStopwords, modalStopwordsBackdrop);
}

if (btnOpenStopwordsModal) btnOpenStopwordsModal.addEventListener("click", openStopwordsModal);
if (btnCloseStopwords) btnCloseStopwords.addEventListener("click", closeStopwordsModal);
if (btnCloseStopwordsX) btnCloseStopwordsX.addEventListener("click", closeStopwordsModal);
if (modalStopwordsBackdrop) modalStopwordsBackdrop.addEventListener("click", closeStopwordsModal);
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && modalStopwords && modalStopwords.classList.contains("in")) {
    closeStopwordsModal();
  }
});

// Save Rules
if (btnSaveRules) {
  btnSaveRules.addEventListener("click", () => {
    const text = stopwordsTextarea ? stopwordsTextarea.value : "";
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "update_rules",
        text: text
      }));
    }
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("update_rules", { text: text }).catch(() => { });
    }
  });
}

// Lookahead Hotkeys handled in unified hotkey listener above

function restoreLocalPreferences() {
  try {
    const savedShield = localStorage.getItem("blewred_censor_shield_enabled");
    if (toggleCensorShield) {
      toggleCensorShield.checked = (savedShield === null || savedShield === "true");
      updateCensorShieldUI(toggleCensorShield.checked, false);
    }
    const savedDonate = localStorage.getItem("blewred_shield_donate");
    if (toggleShieldDonate) {
      toggleShieldDonate.checked = (savedDonate === null || savedDonate !== "false");
    }
    const savedFps = localStorage.getItem("blewred_fps_boosted");
    if (savedFps !== null) {
      const isBoost = savedFps === "true";
      setFpsMode(isBoost);
    }
    const savedOcr = localStorage.getItem("blewred_ocr_enabled");
    if (savedOcr !== null) {
      const isOcr = savedOcr === "true";
      updateOcrStateUI(isOcr);
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("toggle_ocr", { enabled: isOcr }).catch(() => { });
      }
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: "toggle_ocr", enabled: isOcr }));
      }
    }
    try {
      const savedScope = localStorage.getItem("blewred_hotkey_scope");
      const savedPanic = localStorage.getItem("blewred_hotkey_panic");
      const savedThreat = localStorage.getItem("blewred_hotkey_threat");
      if (savedScope || savedPanic || savedThreat) {
        updateHotkeySettingsUI(savedScope || "global", savedPanic || "F9", savedThreat || "F8");
      }
    } catch (e) { }
    const savedHud = localStorage.getItem("blewred_hud_monitor");
    if (savedHud !== null && hudMonitorSelect) {
      const hIdx = parseInt(savedHud, 10);
      const isKnownValid = hIdx === -1 || !lastMonitorsList || lastMonitorsList.length === 0 || lastMonitorsList.some(m => m.index === hIdx);
      if (isKnownValid && !isNaN(hIdx)) {
        hudMonitorSelect.value = savedHud;
        if (socket && socket.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ type: "set_hud_monitor", monitor_index: hIdx }));
        }
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("set_hud_monitor", { index: hIdx }).catch(() => { });
        }
      }
    }
    const savedMon = localStorage.getItem("blewred_selected_monitor");
    if (savedMon !== null && monitorSelect) {
      const mIdx = parseInt(savedMon, 10);
      const isKnownValid = !lastMonitorsList || lastMonitorsList.length === 0 || lastMonitorsList.some(m => m.index === mIdx);
      if (isKnownValid && !isNaN(mIdx)) {
        monitorSelect.value = mIdx;
        if (socket && socket.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ type: "set_monitor", monitor_index: mIdx }));
        }
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("set_monitor", { index: mIdx }).catch(() => { });
        }
      }
    }
    const savedThreshold = localStorage.getItem("blewred_nsfw_threshold");
    if (savedThreshold !== null && nsfwThresholdSlider) {
      const th = parseInt(savedThreshold, 10);
      if (!isNaN(th)) {
        nsfwThresholdSlider.value = th;
        formatSensitivityUI(th);
        setThresholdMarker(th);
        if (socket && socket.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ type: "set_nsfw_threshold", threshold: th }));
        }
        if (window.__TAURI__ && window.__TAURI__.core) {
          window.__TAURI__.core.invoke("set_nsfw_threshold", { threshold: th }).catch(() => { });
        }
      }
    }
    const savedCategories = localStorage.getItem("blewred_censor_categories");
    if (savedCategories) {
      try {
        const cats = JSON.parse(savedCategories);
        updateCensorCategoriesUI(cats);
        sendCensorCategories();
      } catch (e) { }
    }
    const savedPrewarn = localStorage.getItem("blewred_cues_prewarn");
    if (savedPrewarn !== null && cuePrewarnSecondsInput) {
      const pw = parseFloat(savedPrewarn);
      if (!isNaN(pw)) {
        cuePrewarnSecondsInput.value = pw;
      }
    }
    const savedAutocensor = localStorage.getItem("blewred_cues_autocensor");
    if (savedAutocensor !== null && toggleCueAutocensor) {
      toggleCueAutocensor.checked = (savedAutocensor === "true");
      if (labelCueAutocensorStatus) {
        const _isEnCueSt = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
        labelCueAutocensorStatus.textContent = toggleCueAutocensor.checked ? (_isEnCueSt ? "Enabled (OBS + Audio)" : "Включена (OBS + звук)") : (_isEnCueSt ? "Disabled (HUD Only)" : "Отключена (Только HUD)");
        labelCueAutocensorStatus.style.color = toggleCueAutocensor.checked ? "#38bdf8" : "#94a3b8";
      }
    }
    const savedMode = localStorage.getItem("blewred_operation_mode");
    if (savedMode !== null) {
      const m = parseInt(savedMode, 10);
      if (!isNaN(m)) {
        setOperationMode(m);
      }
    }
    if (typeof initSetupGuideVisibility === "function") {
      initSetupGuideVisibility();
    }
  } catch (e) {
    console.error("[UI] Error restoring preferences:", e);
  }
}

async function loadInitialTauriData() {
  if (window.__TAURI__ && window.__TAURI__.core) {
    try {
      try {
        const userSettings = await window.__TAURI__.core.invoke("get_user_settings");
        if (userSettings && userSettings.hide_setup_guide !== undefined) {
          if (typeof initSetupGuideVisibility === "function") {
            initSetupGuideVisibility(userSettings.hide_setup_guide);
          }
        }
        if (userSettings && userSettings.shield_donate_enabled !== undefined && toggleShieldDonate) {
          const savedDonate = localStorage.getItem("blewred_shield_donate");
          if (savedDonate === null) {
            toggleShieldDonate.checked = !!userSettings.shield_donate_enabled;
          }
        }
        if (userSettings && userSettings.close_action) {
          const selClose = document.getElementById("select-close-action");
          if (selClose) {
            selClose.value = userSettings.close_action;
          }
        }
        if (userSettings && userSettings.active_model_profile) {
          activeModelProfileName = userSettings.active_model_profile;
        }
        if (userSettings && userSettings.model_tuning) {
          applyModelTuningToUI(userSettings.model_tuning, userSettings.active_model_profile);
        }
      } catch (e) { }

      try {
        const profiles = await window.__TAURI__.core.invoke("get_model_profiles");
        if (Array.isArray(profiles) && profiles.length > 0) {
          updateModelProfilesList(profiles, activeModelProfileName);
        }
      } catch (e) { }

      const telem = await window.__TAURI__.core.invoke("get_telemetry");
      if (telem) {
        const savedMode = localStorage.getItem("blewred_operation_mode");
        if (savedMode !== null) {
          const m = parseInt(savedMode, 10);
          if (!isNaN(m)) {
            telem.operation_mode = m;
            setOperationMode(m);
          }
        } else if (telem.operation_mode !== undefined) {
          updateOperationModeUI(telem.operation_mode);
        }
        if (telem.ocr_enabled !== undefined) {
          const savedOcr = localStorage.getItem("blewred_ocr_enabled");
          if (savedOcr !== null) {
            updateOcrStateUI(savedOcr === "true");
          } else {
            updateOcrStateUI(telem.ocr_enabled);
          }
        }
        updateTelemetry(telem);
        if (telem.monitors) {
          updateMonitorsDropdown(telem.monitors, telem.selected_monitor);
          updateHudMonitorsDropdown(telem.monitors, telem.hud_monitor);
        }
      }
      const rules = await window.__TAURI__.core.invoke("get_rules_text");
      if (rules && stopwordsTextarea) {
        stopwordsTextarea.value = rules;
      }
    } catch (e) {
      console.warn("[Tauri IPC] Initial telemetry load fallback to WS:", e);
    }
  }
}

// About Modal Controls (Bootstrap 3 Dark Edition)
const modalAbout = document.getElementById("modal-about");
const modalAboutBackdrop = document.getElementById("modal-about-backdrop");
const btnNavbarAbout = document.getElementById("btn-navbar-about");
const btnCloseAbout = document.getElementById("btn-close-about");
const btnCloseAboutX = document.getElementById("btn-close-about-x");
const btnAboutDonate = document.getElementById("btn-about-donate");

function openAboutModal() {
  openModal(modalAbout, modalAboutBackdrop);
}

function closeAboutModal() {
  closeModal(modalAbout, modalAboutBackdrop);
}

if (btnNavbarAbout) btnNavbarAbout.addEventListener("click", openAboutModal);
if (btnCloseAbout) btnCloseAbout.addEventListener("click", closeAboutModal);
if (btnCloseAboutX) btnCloseAboutX.addEventListener("click", closeAboutModal);
if (modalAboutBackdrop) modalAboutBackdrop.addEventListener("click", closeAboutModal);
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && modalAbout && modalAbout.classList.contains("in")) {
    closeAboutModal();
  }
});

// Close Action Selector (inside About Modal)
const selectCloseAction = document.getElementById("select-close-action");
if (selectCloseAction) {
  selectCloseAction.addEventListener("change", (e) => {
    const val = e.target.value;
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("set_close_action", { action: val }).catch((err) => {
        console.error("[Settings] Failed to save close action:", err);
      });
    }
  });
}

// ============================================================================
// System Tray & Window Close Confirmation Modal (Bootstrap 3 Dark Edition)
// ============================================================================
const modalCloseConfirm = document.getElementById("modal-close-confirm");
const modalCloseConfirmBackdrop = document.getElementById("modal-close-confirm-backdrop");
const btnActionMinimizeTray = document.getElementById("btn-action-minimize-tray");
const btnActionExitApp = document.getElementById("btn-action-exit-app");
const btnCancelCloseConfirm = document.getElementById("btn-cancel-close-confirm");
const btnCloseConfirmX = document.getElementById("btn-close-confirm-x");
const chkCloseRemember = document.getElementById("chk-close-remember");

function openCloseConfirmModal() {
  if (chkCloseRemember) chkCloseRemember.checked = false;
  openModal(modalCloseConfirm, modalCloseConfirmBackdrop);
}

function closeCloseConfirmModal() {
  closeModal(modalCloseConfirm, modalCloseConfirmBackdrop);
}

function minimizeToTray() {
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("minimize_to_tray").catch((err) => {
      console.error("[Tray] Error minimizing to tray:", err);
    });
  }
}

function exitApplication() {
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("exit_application").catch((err) => {
      console.error("[App] Error exiting application:", err);
    });
  }
}


// Close Dialog Action 1: Minimize to tray (Recommended)
if (btnActionMinimizeTray) {
  btnActionMinimizeTray.addEventListener("click", async () => {
    if (chkCloseRemember && chkCloseRemember.checked) {
      if (window.__TAURI__ && window.__TAURI__.core) {
        await window.__TAURI__.core.invoke("set_close_action", { action: "tray" }).catch(() => { });
      }
      if (selectCloseAction) selectCloseAction.value = "tray";
    }
    closeCloseConfirmModal();
    minimizeToTray();
  });
}

// Close Dialog Action 2: Exit Completely
if (btnActionExitApp) {
  btnActionExitApp.addEventListener("click", async () => {
    if (chkCloseRemember && chkCloseRemember.checked) {
      if (window.__TAURI__ && window.__TAURI__.core) {
        await window.__TAURI__.core.invoke("set_close_action", { action: "exit" }).catch(() => { });
      }
      if (selectCloseAction) selectCloseAction.value = "exit";
    }
    closeCloseConfirmModal();
    exitApplication();
  });
}

if (btnCancelCloseConfirm) btnCancelCloseConfirm.addEventListener("click", closeCloseConfirmModal);
if (btnCloseConfirmX) btnCloseConfirmX.addEventListener("click", closeCloseConfirmModal);
if (modalCloseConfirmBackdrop) modalCloseConfirmBackdrop.addEventListener("click", closeCloseConfirmModal);

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && modalCloseConfirm && modalCloseConfirm.classList.contains("in")) {
    closeCloseConfirmModal();
  }
  if (e.key === "Escape" && modalDisableSupportConfirm && modalDisableSupportConfirm.classList.contains("in")) {
    cancelDisableSupport();
  }
});

// Listen for Tauri backend close request event
if (window.__TAURI__ && window.__TAURI__.event) {
  window.__TAURI__.event.listen("blewred://request-close", () => {
    console.log("[blewred] Window close requested by user: opening confirmation modal.");
    openCloseConfirmModal();
  });

  window.__TAURI__.event.listen("blewred://operation-mode-changed", (event) => {
    const mode = event.payload;
    console.log("[Tray] Operation mode changed event received:", mode);
    if (typeof setOperationMode === "function") {
      setOperationMode(mode);
    }
  });

  window.__TAURI__.event.listen("blewred://monitor-changed", (event) => {
    const idx = event.payload;
    console.log("[Tray] Screen capture monitor changed event received:", idx);
    lastSelectedMonitor = idx;
    if (monitorSelect) {
      monitorSelect.value = idx.toString();
    }
    try {
      localStorage.setItem("blewred_selected_monitor", idx.toString());
    } catch (e) { }
    if (typeof updateMonitorsDropdown === "function") {
      updateMonitorsDropdown(lastMonitorsList, idx);
    }
  });

  window.__TAURI__.event.listen("blewred://hud-monitor-changed", (event) => {
    const idx = event.payload;
    console.log("[Tray] HUD monitor changed event received:", idx);
    lastSelectedHudMonitor = idx;
    if (hudMonitorSelect) {
      hudMonitorSelect.value = idx.toString();
    }
    try {
      localStorage.setItem("blewred_hud_monitor", idx.toString());
    } catch (e) { }
    if (typeof updateHudMonitorsDropdown === "function") {
      updateHudMonitorsDropdown(lastMonitorsList, idx);
    }
  });
}


// Hotkeys Modal Controls (Bootstrap 3 Dark Edition)
function openHotkeysModal() {
  openModal(modalHotkeys, modalHotkeysBackdrop);
}

function closeHotkeysModal() {
  cancelHotkeyRecording();
  closeModal(modalHotkeys, modalHotkeysBackdrop);
}

if (btnOpenHotkeysModal) btnOpenHotkeysModal.addEventListener("click", openHotkeysModal);
if (btnCloseHotkeys) btnCloseHotkeys.addEventListener("click", closeHotkeysModal);
if (btnCloseHotkeysX) btnCloseHotkeysX.addEventListener("click", closeHotkeysModal);
if (modalHotkeysBackdrop) modalHotkeysBackdrop.addEventListener("click", closeHotkeysModal);

// External browser link for developer support
if (btnAboutDonate) {
  btnAboutDonate.addEventListener("click", () => {
    const donateUrl = "https://web.tribute.tg/e/1dW";
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("open_external_url", { url: donateUrl }).catch(() => {
        window.open(donateUrl, "_blank");
      });
    } else {
      window.open(donateUrl, "_blank");
    }
  });
}

// WinRT OCR Diagnostics & Installation Controls (Selective: RU pack for RU language, EN pack for EN)
const ruOcrMissingAlert = document.getElementById("ru-ocr-missing-alert");
const btnInstallRuOcr = document.getElementById("btn-install-ru-ocr");
const btnOpenOcrSettings = document.getElementById("btn-open-ocr-settings");
const ocrAlertTitle = document.getElementById("ocr-alert-title");
const ocrAlertDesc1 = document.getElementById("ocr-alert-desc1");
const ocrAlertDesc2 = document.getElementById("ocr-alert-desc2");
const btnInstallOcrLabel = document.getElementById("btn-install-ocr-label");

let lastRuOcrInstalled = true;
let lastEnOcrInstalled = true;

function updateOcrStatusUI(ruInstalled, enInstalled) {
  if (ruInstalled !== undefined) lastRuOcrInstalled = !!ruInstalled;
  if (enInstalled !== undefined) lastEnOcrInstalled = !!enInstalled;

  if (!ruOcrMissingAlert) return;

  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  if (!isEn) {
    if (!lastRuOcrInstalled) {
      ruOcrMissingAlert.style.display = "flex";
      if (ocrAlertTitle && window.I18N) ocrAlertTitle.innerText = window.I18N.t("ru_ocr_missing_title");
      if (ocrAlertDesc1 && window.I18N) ocrAlertDesc1.innerText = window.I18N.t("ru_ocr_missing_desc1");
      if (ocrAlertDesc2 && window.I18N) ocrAlertDesc2.innerText = window.I18N.t("ru_ocr_missing_desc2");
      if (btnInstallOcrLabel && window.I18N) btnInstallOcrLabel.innerText = window.I18N.t("btn_install_ru_ocr");
    } else {
      ruOcrMissingAlert.style.display = "none";
    }
  } else {
    // English interface: check English OCR pack only (almost always present on Windows)
    if (!lastEnOcrInstalled) {
      ruOcrMissingAlert.style.display = "flex";
      if (ocrAlertTitle && window.I18N) ocrAlertTitle.innerText = window.I18N.t("en_ocr_missing_title");
      if (ocrAlertDesc1 && window.I18N) ocrAlertDesc1.innerText = window.I18N.t("en_ocr_missing_desc1");
      if (ocrAlertDesc2 && window.I18N) ocrAlertDesc2.innerText = window.I18N.t("en_ocr_missing_desc2");
      if (btnInstallOcrLabel && window.I18N) btnInstallOcrLabel.innerText = window.I18N.t("btn_install_en_ocr");
    } else {
      ruOcrMissingAlert.style.display = "none";
    }
  }
}

if (btnInstallRuOcr) {
  btnInstallRuOcr.addEventListener("click", () => {
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    btnInstallRuOcr.disabled = true;
    if (btnInstallOcrLabel) {
      btnInstallOcrLabel.innerText = isEn ? "Installing (UAC)..." : "Установка (UAC)...";
    }
    const msgType = isEn ? "install_en_ocr" : "install_ru_ocr";
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: msgType }));
    }
    renderEventItem({
      type: "system",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn
        ? "[Windows OCR] Installation of WinRT OCR package requested. Confirm Administrator privileges in the system UAC dialog."
        : "[Windows OCR] Запрошена установка языкового пакета WinRT OCR. Подтвердите права Администратора в системном окне UAC."
    });

    // Reset button after 4 seconds
    setTimeout(() => {
      btnInstallRuOcr.disabled = false;
      if (btnInstallOcrLabel && window.I18N) {
        btnInstallOcrLabel.innerText = window.I18N.t(isEn ? "btn_install_en_ocr" : "btn_install_ru_ocr");
      }
    }, 4000);
  });
}

if (btnOpenOcrSettings) {
  btnOpenOcrSettings.addEventListener("click", () => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "open_windows_language_settings" }));
    }
  });
}

// Hotkey Configuration & Multi-Key Remapping Logic
let currentHotkeyScope = "global";
let currentHotkeyPanic = "F9";
let currentHotkeyThreat = "F8";

const radioScopeGlobal = document.getElementById("hotkey-scope-global");
const radioScopeLocal = document.getElementById("hotkey-scope-local");
const btnRebindPanic = document.getElementById("btn-rebind-panic");
const btnRebindThreat = document.getElementById("btn-rebind-threat");
const lblHotkeyPanic = document.getElementById("lbl-hotkey-panic");
const lblHotkeyThreat = document.getElementById("lbl-hotkey-threat");
const badgeHotkeyPanic = document.getElementById("badge-hotkey-panic");
const badgeHotkeyThreat = document.getElementById("badge-hotkey-threat");
const lblRebindPanicText = document.getElementById("lbl-rebind-panic-text");
const lblRebindThreatText = document.getElementById("lbl-rebind-threat-text");

const btnResetHotkeys = document.getElementById("btn-reset-hotkeys");
const btnSaveHotkeys = document.getElementById("btn-save-hotkeys");

function updateLookaheadHudButtons() {
  const btnBlock = document.getElementById("btn-lookahead-block");
  const btnAllow = document.getElementById("btn-lookahead-allow");
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (btnBlock) {
    btnBlock.innerHTML = `${renderKeyComboBadge(currentHotkeyPanic)} ${isEn ? "BLOCK SCREEN IN OBS" : "ЗАБЛОКИРОВАТЬ ЭКРАН В OBS"}`;
  }
  if (btnAllow) {
    btnAllow.innerHTML = `${renderKeyComboBadge(currentHotkeyThreat)} ${isEn ? "ALLOW (SAFE)" : "ПРОПУСТИТЬ (БЕЗОПАСНО)"}`;
  }
}

function updateHotkeySettingsUI(scope, panic, threat) {
  if (scope) currentHotkeyScope = scope;
  if (panic) currentHotkeyPanic = formatCanonicalCombo(parseHotkeyCombo(panic));
  if (threat) currentHotkeyThreat = formatCanonicalCombo(parseHotkeyCombo(threat));

  if (radioScopeGlobal && radioScopeLocal) {
    if (currentHotkeyScope === "local") {
      radioScopeLocal.checked = true;
    } else {
      radioScopeGlobal.checked = true;
    }
  }

  // Synchronize Panic Mute / Shield badge
  const pParsed = parseHotkeyCombo(currentHotkeyPanic);
  if (badgeHotkeyPanic) {
    badgeHotkeyPanic.innerHTML = renderKeyComboBadge(pParsed);
  }

  // Synchronize Threat Boost / Skip badge
  const tParsed = parseHotkeyCombo(currentHotkeyThreat);
  if (badgeHotkeyThreat) {
    badgeHotkeyThreat.innerHTML = renderKeyComboBadge(tParsed);
  }

  updateLookaheadHudButtons();
}

if (btnRebindPanic) {
  btnRebindPanic.addEventListener("click", () => {
    startHotkeyRecording("panic");
  });
}

if (btnRebindThreat) {
  btnRebindThreat.addEventListener("click", () => {
    startHotkeyRecording("threat");
  });
}

if (btnSaveHotkeys) {
  btnSaveHotkeys.addEventListener("click", () => {
    const scope = (radioScopeLocal && radioScopeLocal.checked) ? "local" : "global";
    currentHotkeyScope = scope;
    try {
      localStorage.setItem("blewred_hotkey_scope", currentHotkeyScope);
      localStorage.setItem("blewred_hotkey_panic", currentHotkeyPanic);
      localStorage.setItem("blewred_hotkey_threat", currentHotkeyThreat);
    } catch (e) { }

    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "set_hotkey_settings",
        hotkey_scope: currentHotkeyScope,
        hotkey_panic: currentHotkeyPanic,
        hotkey_threat: currentHotkeyThreat
      }));
    }

    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    renderEventItem({
      type: "system",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn
        ? `[Hotkeys] Settings saved: Scope=${scope.toUpperCase()}, Panic=${currentHotkeyPanic}, Threat=${currentHotkeyThreat}`
        : `[Горячие клавиши] Настройки сохранены: Область=${scope === "global" ? "Глобально" : "Локально"}, Паника=${currentHotkeyPanic}, Угроза=${currentHotkeyThreat}`
    });

    closeHotkeysModal();
  });
}

if (btnResetHotkeys) {
  btnResetHotkeys.addEventListener("click", () => {
    updateHotkeySettingsUI("global", "F9", "F8");
  });
}

// AI Models Download Modal Controls (Bootstrap 3 Dark Edition)
const modalModels = document.getElementById("modal-models-download");
const modalModelsBackdrop = document.getElementById("modal-models-backdrop");
const btnOpenModelsModal = document.getElementById("btn-open-models-modal");
const btnCloseModels = document.getElementById("btn-close-models");
const btnCloseModelsX = document.getElementById("btn-close-models-x");
const btnStartModelsDownload = document.getElementById("btn-start-models-download");

const statusBadgeVit = document.getElementById("status-badge-vit");
const progBarVit = document.getElementById("prog-bar-vit");
const detailVit = document.getElementById("detail-vit");
const pctVit = document.getElementById("pct-vit");

const statusBadge640m = document.getElementById("status-badge-640m");
const progBar640m = document.getElementById("prog-bar-640m");
const detail640m = document.getElementById("detail-640m");
const pct640m = document.getElementById("pct-640m");

const modelsDownloadTelemetry = document.getElementById("models-download-telemetry");
const downloadSpeedVal = document.getElementById("download-speed-val");
const downloadEtaVal = document.getElementById("download-eta-val");
const modelsDownloadAlert = document.getElementById("models-download-alert");
const modelsAlertMsg = document.getElementById("models-alert-msg");

let isDownloadingModels = false;

function openModelsModal() {
  if (lastModelsStatus) {
    updateModelsStatusUI(lastModelsStatus);
  }
  openModal(modalModels, modalModelsBackdrop, () => {
    sendWs("check_models_status");
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("check_models_status").then(res => {
        if (res) updateModelsStatusUI(res);
      }).catch(() => { });
    }
  });
}

function closeModelsModal() {
  closeModal(modalModels, modalModelsBackdrop);
}

let lastModelsStatus = null;

function updateModelsStatusUI(status) {
  if (!status) return;
  lastModelsStatus = status;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  const vit = (status.models && status.models.find(m => m.filename === "vit_nsfw.onnx")) || {
    valid: !!status.vit_valid,
    exists: !!status.vit_exists
  };
  const m640 = (status.models && status.models.find(m => m.filename === "640m.onnx")) || {
    valid: !!status.m640_valid,
    exists: !!status.m640_exists
  };
  const allReady = status.all_ready !== undefined ? !!status.all_ready : (vit.valid && m640.valid);

  if (vit.valid) {
    if (statusBadgeVit) {
      statusBadgeVit.className = "label label-success";
      statusBadgeVit.textContent = isEn ? "Ready (OK)" : "Готово (OK)";
    }
    if (progBarVit) progBarVit.style.width = "100%";
    if (pctVit) pctVit.textContent = "100%";
    if (detailVit) detailVit.textContent = isEn ? "vit_nsfw.onnx (Verified SHA-256)" : "vit_nsfw.onnx (Проверено SHA-256)";
  } else if (vit.exists) {
    if (statusBadgeVit) {
      statusBadgeVit.className = "label label-warning";
      statusBadgeVit.textContent = isEn ? "Corrupted" : "Поврежден";
    }
    if (detailVit) detailVit.textContent = isEn ? "vit_nsfw.onnx (Hash mismatch)" : "vit_nsfw.onnx (Хеш не совпадает)";
  } else {
    if (statusBadgeVit) {
      statusBadgeVit.className = "label label-danger";
      statusBadgeVit.textContent = isEn ? "Missing" : "Отсутствует";
    }
    if (progBarVit) progBarVit.style.width = "0%";
    if (pctVit) pctVit.textContent = "0%";
    if (detailVit) detailVit.textContent = isEn ? "Download required" : "Требуется загрузка";
  }

  if (m640.valid) {
    if (statusBadge640m) {
      statusBadge640m.className = "label label-success";
      statusBadge640m.textContent = isEn ? "Ready (OK)" : "Готово (OK)";
    }
    if (progBar640m) progBar640m.style.width = "100%";
    if (pct640m) pct640m.textContent = "100%";
    if (detail640m) detail640m.textContent = isEn ? "640m.onnx (Verified SHA-256)" : "640m.onnx (Проверено SHA-256)";
  } else if (m640.exists) {
    if (statusBadge640m) {
      statusBadge640m.className = "label label-warning";
      statusBadge640m.textContent = isEn ? "Corrupted" : "Поврежден";
    }
    if (detail640m) detail640m.textContent = isEn ? "640m.onnx (Hash mismatch)" : "640m.onnx (Хеш не совпадает)";
  } else {
    if (statusBadge640m) {
      statusBadge640m.className = "label label-danger";
      statusBadge640m.textContent = isEn ? "Missing" : "Отсутствует";
    }
    if (progBar640m) progBar640m.style.width = "0%";
    if (pct640m) pct640m.textContent = "0%";
    if (detail640m) detail640m.textContent = isEn ? "Download required" : "Требуется загрузка";
  }

  if (allReady) {
    if (btnStartModelsDownload) {
      btnStartModelsDownload.disabled = false;
      btnStartModelsDownload.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 4px;"><path d="M20 6 9 17l-5-5"/></svg> ${isEn ? "Verified (OK)" : "Проверено (OK)"}`;
    }
    if (modelsDownloadAlert) {
      modelsDownloadAlert.className = "alert alert-success";
      modelsDownloadAlert.style.display = "block";
      if (modelsAlertMsg) modelsAlertMsg.textContent = isEn ? "All neural network models installed and verified (SHA-256)." : "Все нейросетевые модели установлены и прошли верификацию SHA-256.";
    }
  } else {
    if (btnStartModelsDownload && !isDownloadingModels) {
      btnStartModelsDownload.disabled = false;
      btnStartModelsDownload.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 4px;"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg> ${isEn ? "Download missing" : "Загрузить недостающие"}`;
    }
    if (modelsDownloadAlert && !isDownloadingModels) {
      modelsDownloadAlert.className = "alert alert-warning";
      modelsDownloadAlert.style.display = "block";
      if (modelsAlertMsg) modelsAlertMsg.textContent = isEn ? "Missing model files detected. Click 'Download missing'." : "Обнаружены отсутствующие файлы моделей. Нажмите «Загрузить недостающие».";
    }
  }

  if (typeof updateSetupGuideStatus === "function") {
    updateSetupGuideStatus({ models_status: status });
  }
}

function handleModelDownloadProgress(data) {
  isDownloadingModels = true;
  if (modelsDownloadTelemetry) modelsDownloadTelemetry.style.display = "block";
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  const p = data.progress || data;
  const bytesDown = p.downloaded_bytes ?? p.bytes_downloaded ?? 0;
  const totalBytes = p.total_bytes || 1;
  const speedBytes = p.speed_bytes_per_sec || 0;
  const percentVal = p.percent ?? (totalBytes > 0 ? (bytesDown / totalBytes) * 100 : 0);

  const mbDownloaded = (bytesDown / (1024 * 1024)).toFixed(1);
  const mbTotal = (totalBytes / (1024 * 1024)).toFixed(1);
  const pct = Math.min(100, Math.max(0, Math.round(percentVal)));

  if (downloadSpeedVal) {
    const mbSpeed = (speedBytes / (1024 * 1024)).toFixed(1);
    downloadSpeedVal.textContent = isEn ? `${mbSpeed} MB/s` : `${mbSpeed} МБ/с`;
  }
  if (downloadEtaVal) {
    downloadEtaVal.textContent = p.eta_seconds > 0
      ? (isEn ? `~${Math.round(p.eta_seconds)} sec` : `~${Math.round(p.eta_seconds)} сек`)
      : (isEn ? "Completing..." : "Завершение...");
  }

  if (btnStartModelsDownload) {
    btnStartModelsDownload.disabled = true;
    btnStartModelsDownload.textContent = isEn ? `Downloading: ${pct}%...` : `Загрузка: ${pct}%...`;
  }

  const stage = data.stage ?? ((p.filename && p.filename.includes("vit")) ? 1 : 2);

  if (stage === 1) {
    if (statusBadgeVit) {
      statusBadgeVit.className = "label label-info";
      statusBadgeVit.textContent = isEn ? "Downloading..." : "Загрузка...";
    }
    if (progBarVit) progBarVit.style.width = `${pct}%`;
    if (pctVit) pctVit.textContent = `${pct}%`;
    if (detailVit) detailVit.textContent = isEn ? `${mbDownloaded} / ${mbTotal} MB (${pct}%)` : `${mbDownloaded} / ${mbTotal} МБ (${pct}%)`;
  } else if (stage === 2) {
    if (statusBadgeVit) {
      statusBadgeVit.className = "label label-success";
      statusBadgeVit.textContent = isEn ? "Ready (OK)" : "Готово (OK)";
    }
    if (progBarVit) progBarVit.style.width = "100%";
    if (pctVit) pctVit.textContent = "100%";

    if (statusBadge640m) {
      statusBadge640m.className = "label label-info";
      statusBadge640m.textContent = isEn ? "Downloading..." : "Загрузка...";
    }
    if (progBar640m) progBar640m.style.width = `${pct}%`;
    if (pct640m) pct640m.textContent = `${pct}%`;
    if (detail640m) detail640m.textContent = isEn ? `${mbDownloaded} / ${mbTotal} MB (${pct}%)` : `${mbDownloaded} / ${mbTotal} МБ (${pct}%)`;
  }
}

function handleModelDownloadComplete(data) {
  isDownloadingModels = false;
  if (modelsDownloadTelemetry) modelsDownloadTelemetry.style.display = "none";
  const isSuccess = !!(data && (data.success || data.all_ready));
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  if (isSuccess) {
    if (statusBadgeVit) {
      statusBadgeVit.className = "label label-success";
      statusBadgeVit.textContent = isEn ? "Ready (OK)" : "Готово (OK)";
    }
    if (statusBadge640m) {
      statusBadge640m.className = "label label-success";
      statusBadge640m.textContent = isEn ? "Ready (OK)" : "Готово (OK)";
    }
    if (progBarVit) progBarVit.style.width = "100%";
    if (pctVit) pctVit.textContent = "100%";
    if (progBar640m) progBar640m.style.width = "100%";
    if (pct640m) pct640m.textContent = "100%";

    if (btnStartModelsDownload) {
      btnStartModelsDownload.disabled = false;
      btnStartModelsDownload.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 4px;"><path d="M20 6 9 17l-5-5"/></svg> ${isEn ? "Verified (OK)" : "Проверено (OK)"}`;
    }

    if (modelsDownloadAlert) {
      modelsDownloadAlert.className = "alert alert-success";
      modelsDownloadAlert.style.display = "block";
      if (modelsAlertMsg) modelsAlertMsg.textContent = isEn ? "All neural network models successfully downloaded and verified!" : "Все нейросетевые модели успешно загружены и проверены!";
    }

    renderEventItem({
      type: "success",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn ? "[AI Models] Download and verification of ViT + 640m models completed successfully." : "[ИИ Модели] Загрузка и верификация моделей ViT + 640m успешно завершена."
    });
  } else {
    if (btnStartModelsDownload) {
      btnStartModelsDownload.disabled = false;
      btnStartModelsDownload.textContent = isEn ? "Retry download" : "Повторить загрузку";
    }
    if (modelsDownloadAlert) {
      modelsDownloadAlert.className = "alert alert-danger";
      modelsDownloadAlert.style.display = "block";
      if (modelsAlertMsg) modelsAlertMsg.textContent = data.message || (isEn ? "Failed to download models. Check internet connection." : "Ошибка загрузки моделей. Проверьте интернет-соединение.");
    }

    renderEventItem({
      type: "danger",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn ? `[AI Models] Error: ${data.message || "Failed to download models"}` : `[ИИ Модели] Ошибка: ${data.message || "Не удалось загрузить модели"}`
    });
  }
}

if (btnOpenModelsModal) btnOpenModelsModal.addEventListener("click", openModelsModal);
if (btnCloseModels) btnCloseModels.addEventListener("click", closeModelsModal);
if (btnCloseModelsX) btnCloseModelsX.addEventListener("click", closeModelsModal);
if (modalModelsBackdrop) modalModelsBackdrop.addEventListener("click", closeModelsModal);

if (btnStartModelsDownload) {
  btnStartModelsDownload.addEventListener("click", () => {
    if (lastModelsStatus && lastModelsStatus.all_ready) {
      updateModelsStatusUI(lastModelsStatus);
      sendWs("check_models_status");
      return;
    }
    if (socket && socket.readyState === WebSocket.OPEN) {
      const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
      socket.send(JSON.stringify({ type: "start_models_download" }));
      isDownloadingModels = true;
      btnStartModelsDownload.disabled = true;
      btnStartModelsDownload.textContent = isEn ? "Connecting..." : "Подключение...";
      if (modelsDownloadAlert) {
        modelsDownloadAlert.className = "alert alert-info";
        modelsDownloadAlert.style.display = "block";
        if (modelsAlertMsg) modelsAlertMsg.textContent = isEn ? "Connecting to HuggingFace model repository..." : "Соединение с репозиторием моделей HuggingFace...";
      }
    }
  });
}

// Browser Extension Helper Modal Controls (Bootstrap 3 Dark Edition)
const modalBrowserExtension = document.getElementById("modal-browser-extension");
const modalBrowserExtensionBackdrop = document.getElementById("modal-browser-extension-backdrop");
const btnOpenExtensionModal = document.getElementById("btn-open-extension-modal");
const btnCloseExt = document.getElementById("btn-close-ext");
const btnCloseExtX = document.getElementById("btn-close-ext-x");
const btnLaunchBrowserExt = document.getElementById("btn-launch-browser-ext");
const btnOpenExtDir = document.getElementById("btn-open-ext-dir");

function openExtensionModal() {
  openModal(modalBrowserExtension, modalBrowserExtensionBackdrop);
}

function closeExtensionModal() {
  closeModal(modalBrowserExtension, modalBrowserExtensionBackdrop);
}

if (btnOpenExtensionModal) btnOpenExtensionModal.addEventListener("click", openExtensionModal);
if (btnCloseExt) btnCloseExt.addEventListener("click", closeExtensionModal);
if (btnCloseExtX) btnCloseExtX.addEventListener("click", closeExtensionModal);
if (modalBrowserExtensionBackdrop) modalBrowserExtensionBackdrop.addEventListener("click", closeExtensionModal);

if (btnLaunchBrowserExt) {
  btnLaunchBrowserExt.addEventListener("click", () => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "launch_browser_with_extension" }));
    }
    renderEventItem({
      type: "system",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: (window.I18N && window.I18N.getLanguage() === "en") ? "[Browser] Launched browser with active blewred Lookahead Interceptor." : "[Браузер] Запущен браузер с активным расширением blewred Lookahead Interceptor."
    });
  });
}

if (btnOpenExtDir) {
  btnOpenExtDir.addEventListener("click", () => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "open_browser_extension_dir" }));
    }
  });
}

// ============================================================================
// Streamer Setup Guide & Recommended Steps Controller
// ============================================================================
// streamerSetupGuide hoisted
const setupGuideBody = document.getElementById("setup-guide-body");
const setupGuideProgressBadge = document.getElementById("setup-guide-progress-badge");
const btnCloseSetupGuide = document.getElementById("btn-close-setup-guide");
const btnNavbarGuide = document.getElementById("btn-navbar-guide");
// chkHideSetupGuide hoisted
const btnOpenDocsGuide = document.getElementById("btn-open-docs-guide");
const btnOpenGraphicsSettings = document.getElementById("btn-open-graphics-settings");
const btnOpenModelsSetup = document.getElementById("btn-open-models-setup");
const btnOpenExtFolder = document.getElementById("btn-open-ext-folder");
// btnTriggerObsSetup hoisted

const stepCardGpu = document.getElementById("step-card-gpu");
const badgeStepGpu = document.getElementById("badge-step-gpu");
const stepCardModels = document.getElementById("step-card-models");
const badgeStepModels = document.getElementById("badge-step-models");
const stepCardExt = document.getElementById("step-card-ext");
const badgeStepExt = document.getElementById("badge-step-ext");
// stepCardObs hoisted
// badgeStepObs hoisted

function initSetupGuideVisibility(hideSetting) {
  let userHide = false;
  try {
    const saved = localStorage.getItem("blewred_hide_setup_guide");
    if (saved !== null) {
      userHide = saved === "true";
    } else if (hideSetting !== undefined) {
      userHide = !!hideSetting;
    }
  } catch (e) { }

  if (chkHideSetupGuide) {
    chkHideSetupGuide.checked = userHide;
  }
  if (streamerSetupGuide) {
    streamerSetupGuide.style.display = userHide ? "none" : "block";
  }

  // Pre-populate verified status from localStorage
  try {
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    if (localStorage.getItem("blewred_setup_ext_verified") === "true") {
      if (badgeStepExt && stepCardExt) {
        badgeStepExt.className = "label label-success step-status-badge";
        badgeStepExt.textContent = isEn ? "Ready (OK)" : "Готово (OK)";
        stepCardExt.classList.add("step-completed");
      }
    }
    if (localStorage.getItem("blewred_setup_obs_verified") === "true") {
      if (badgeStepObs && stepCardObs) {
        badgeStepObs.className = "label label-success step-status-badge";
        badgeStepObs.textContent = isEn ? "Ready (OK)" : "Готово (OK)";
        stepCardObs.classList.add("step-completed");
      }
    }
  } catch (e) { }

  // Recalculate completed steps count
  updateSetupGuideStatus({});
}

// lastSetupGuideData hoisted to top

function updateSetupGuideStatus(data) {
  if (!data) data = {};
  lastSetupGuideData = data;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  // Step 1: GPU / DirectML status
  const gpuInfo = data.gpu_info || (data.telemetry && data.telemetry.gpu_info) || (gpuStatVal ? gpuStatVal.innerText : "") || "";
  if (badgeStepGpu && stepCardGpu && gpuInfo) {
    const isDirectML = gpuInfo.includes("DirectML") || gpuInfo.includes("NVIDIA") || gpuInfo.includes("GeForce") || gpuInfo.includes("GPU");
    if (isDirectML) {
      badgeStepGpu.className = "label label-success step-status-badge";
      badgeStepGpu.textContent = "DirectML OK";
      stepCardGpu.classList.add("step-completed");
    } else if (!gpuInfo.includes("OFF")) {
      badgeStepGpu.className = "label label-info step-status-badge";
      badgeStepGpu.textContent = gpuInfo.length > 16 ? gpuInfo.substring(0, 14) + "..." : gpuInfo;
      stepCardGpu.classList.add("step-completed");
    }
  }

  // Step 2: Models status (only evaluate when explicitly provided)
  const modelsStatus = data.models_status || (data.telemetry && data.telemetry.models_status) || (data.status && (data.type === "models_status_result" || data.type === "models_status") ? data.status : null);
  if (badgeStepModels && stepCardModels && modelsStatus) {
    if (modelsStatus.all_ready) {
      badgeStepModels.className = "label label-success step-status-badge";
      badgeStepModels.textContent = isEn ? "Ready (OK)" : "Готово (OK)";
      stepCardModels.classList.add("step-completed");
    } else if (modelsStatus.vit_exists || modelsStatus.m640_exists) {
      badgeStepModels.className = "label label-warning step-status-badge";
      badgeStepModels.textContent = isEn ? "Incomplete" : "Неполные";
      stepCardModels.classList.remove("step-completed");
    } else {
      badgeStepModels.className = "label label-danger step-status-badge";
      badgeStepModels.textContent = isEn ? "Missing" : "Отсутствуют";
      stepCardModels.classList.remove("step-completed");
    }
  }

  // Step 3: Browser extension (if connected once, permanently marked completed)
  const extConn = data.extension_connected !== undefined ? !!data.extension_connected : (data.telemetry && !!data.telemetry.extension_connected);
  let extPreviouslyVerified = false;
  try {
    extPreviouslyVerified = localStorage.getItem("blewred_setup_ext_verified") === "true";
  } catch (e) { }

  if (badgeStepExt && stepCardExt) {
    if (extConn || extPreviouslyVerified) {
      try { localStorage.setItem("blewred_setup_ext_verified", "true"); } catch (e) { }
      badgeStepExt.className = "label label-success step-status-badge";
      badgeStepExt.textContent = extConn ? (isEn ? "Connected" : "Подключено") : (isEn ? "Ready (OK)" : "Готово (OK)");
      stepCardExt.classList.add("step-completed");
    } else if (!stepCardExt.classList.contains("step-completed")) {
      badgeStepExt.className = "label label-default step-status-badge";
      badgeStepExt.textContent = isEn ? "Not connected" : "Не подключено";
      stepCardExt.classList.remove("step-completed");
    }
  }

  // Step 4: OBS connection
  const obsConn = data.obs_connected !== undefined ? !!data.obs_connected : (data.telemetry && !!data.telemetry.obs_connected);
  let obsPreviouslyVerified = false;
  try {
    obsPreviouslyVerified = localStorage.getItem("blewred_setup_obs_verified") === "true";
  } catch (e) { }

  if (badgeStepObs && stepCardObs) {
    if (obsConn || obsPreviouslyVerified) {
      if (obsConn) {
        try { localStorage.setItem("blewred_setup_obs_verified", "true"); } catch (e) { }
      }
      badgeStepObs.className = "label label-success step-status-badge";
      badgeStepObs.textContent = obsConn ? (isEn ? "Connected" : "Подключено") : (isEn ? "Ready (OK)" : "Готово (OK)");
      stepCardObs.classList.add("step-completed");
    } else if (data.obs_connected !== undefined || (data.telemetry && data.telemetry.obs_connected !== undefined)) {
      badgeStepObs.className = "label label-default step-status-badge";
      badgeStepObs.textContent = isEn ? "Disconnected" : "Отключено";
      stepCardObs.classList.remove("step-completed");
    }
  }

  // Calculate actual completed steps count from the 4 cards
  let completed = 0;
  if (stepCardGpu && stepCardGpu.classList.contains("step-completed")) completed++;
  if (stepCardModels && stepCardModels.classList.contains("step-completed")) completed++;
  if (stepCardExt && stepCardExt.classList.contains("step-completed")) completed++;
  if (stepCardObs && stepCardObs.classList.contains("step-completed")) completed++;

  if (setupGuideProgressBadge) {
    if (completed === 4) {
      setupGuideProgressBadge.className = "label label-success setup-guide-progress-badge";
      setupGuideProgressBadge.textContent = isEn ? "Ready for stream (4 of 4)" : "Готово к эфиру (4 из 4)";
    } else {
      setupGuideProgressBadge.className = "label label-info setup-guide-progress-badge";
      setupGuideProgressBadge.textContent = isEn ? `Progress: ${completed} of 4` : `Готовность: ${completed} из 4`;
    }
  }
}

if (btnCloseSetupGuide) {
  btnCloseSetupGuide.addEventListener("click", () => {
    if (streamerSetupGuide) streamerSetupGuide.style.display = "none";
  });
}

if (btnNavbarGuide) {
  btnNavbarGuide.addEventListener("click", () => {
    if (streamerSetupGuide) {
      const isHidden = streamerSetupGuide.style.display === "none" || streamerSetupGuide.offsetParent === null;
      streamerSetupGuide.style.display = isHidden ? "block" : "none";
      if (isHidden) {
        if (setupGuideBody) setupGuideBody.style.display = "block";
        streamerSetupGuide.scrollIntoView({ behavior: "smooth", block: "start" });
      }
    }
  });
}

if (chkHideSetupGuide) {
  chkHideSetupGuide.addEventListener("change", (e) => {
    const hide = e.target.checked;
    try {
      localStorage.setItem("blewred_hide_setup_guide", hide ? "true" : "false");
    } catch (err) { }
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("set_hide_setup_guide", { hide: hide }).catch(() => { });
    }
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "set_hide_setup_guide", hide: hide }));
    }
  });
}

if (btnOpenGraphicsSettings) {
  btnOpenGraphicsSettings.addEventListener("click", () => {
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("open_windows_graphics_settings").catch(() => { });
    }
    renderEventItem({
      type: "system",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: (window.I18N && window.I18N.getLanguage() === "en") ? "[Setup] Windows 10/11 Graphics settings opened to select Nvidia GPU." : "[Настройка] Открыты параметры графики Windows 10/11 для выбора GPU Nvidia."
    });
  });
}

if (btnOpenExtFolder) {
  btnOpenExtFolder.addEventListener("click", () => {
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("open_extension_folder").catch(() => { });
    }
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "open_browser_extension_dir" }));
    }
    renderEventItem({
      type: "system",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: (window.I18N && window.I18N.getLanguage() === "en") ? "[Setup] Opened extensions/chrome folder for browser loading." : "[Настройка] Открыта папка extensions/chrome для загрузки в браузер."
    });
  });
}

if (btnOpenModelsSetup) {
  btnOpenModelsSetup.addEventListener("click", () => {
    openModelsModal();
  });
}

if (btnOpenDocsGuide) {
  btnOpenDocsGuide.addEventListener("click", () => {
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("open_docs_guide").catch(() => { });
    }
  });
}

// ============================================================================
// Scheduled Cues & Planned Pre-Warning Manager (WinRT Windows Media OCR)
// ============================================================================
const modalCuesManager = document.getElementById("modal-cues-manager");
const modalCuesBackdrop = document.getElementById("modal-cues-backdrop");
const btnOpenCuesModal = document.getElementById("btn-open-cues-modal");
const btnOpenCuesAction = document.getElementById("btn-open-cues-action");
const btnCloseCues = document.getElementById("btn-close-cues");
const btnCloseCuesX = document.getElementById("btn-close-cues-x");
const modalPlayerSyncStatus = document.getElementById("modal-player-sync-status");
// cuesCountBadge hoisted
// cuePrewarnSecondsInput hoisted
// toggleCueAutocensor hoisted
// labelCueAutocensorStatus hoisted
const btnCueModeAppend = document.getElementById("btn-cue-mode-append");
const btnCueModeReplace = document.getElementById("btn-cue-mode-replace");
const cueScreenshotDropzone = document.getElementById("cue-screenshot-dropzone");
const cueFileInput = document.getElementById("cue-file-input");
const cuePreviewContainer = document.getElementById("cue-preview-container");
const cuePreviewThumb = document.getElementById("cue-preview-thumb");
const cuePreviewTitle = document.getElementById("cue-preview-title");
const cuePreviewMeta = document.getElementById("cue-preview-meta");
const btnCueClearPreview = document.getElementById("btn-cue-clear-preview");
const btnCueRecognizeOcr = document.getElementById("btn-cue-recognize-ocr");
const cueManualTextarea = document.getElementById("cue-manual-textarea");
const btnCueParseManualText = document.getElementById("btn-cue-parse-manual-text");
const modalCuesTableCount = document.getElementById("modal-cues-table-count");
const btnClearAllCues = document.getElementById("btn-clear-all-cues");
const cuesTableBody = document.getElementById("cues-table-body");
const cuesModalFooterStatus = document.getElementById("cues-modal-footer-status");
const badgeOcrStatus = document.getElementById("badge-ocr-status");

let currentScheduledCues = [];
let isAppendCuesMode = true;
let currentPastedImageBase64 = "";

function openCuesModal() {
  openModal(modalCuesManager, modalCuesBackdrop, () => {
    sendWs("get_cues_state");
    sendWs("check_ocr_status");
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("get_cues_config").then(cfg => {
        if (cfg) updateCuesConfigUI(cfg);
      }).catch(() => { });
      window.__TAURI__.core.invoke("get_scheduled_cues").then(cues => {
        if (Array.isArray(cues)) renderCuesTable(cues);
      }).catch(() => { });
    }
  });
}

function closeCuesModal() {
  closeModal(modalCuesManager, modalCuesBackdrop);
}

let lastPlayerSyncData = null;

function updatePlayerSyncUI(sync) {
  if (!sync) return;
  lastPlayerSyncData = sync;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  let name = sync.name;
  if (!name || name === "Waiting for player..." || name === "Waiting for player" || name === "Ожидание плеера..." || name === "Ожидание плеера") {
    name = isEn ? "Waiting for player..." : "Ожидание плеера...";
  } else if (name === "Unknown" || name === "Неизвестно") {
    name = isEn ? "Unknown" : "Неизвестно";
  }

  const curTimeSec = sync.time || 0;
  const isPlay = !!sync.is_playing;

  const total = Math.round(curTimeSec);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const timeFormatted = (h > 0 ? `${String(h).padStart(2, "0")}:` : "") +
    `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;

  const statusText = `${name} (${timeFormatted} / ${isPlay ? (isEn ? "Playing" : "Играет") : (isEn ? "Paused" : "Пауза")})`;

  if (modalPlayerSyncStatus) {
    modalPlayerSyncStatus.textContent = statusText;
    modalPlayerSyncStatus.style.borderColor = isPlay ? "#10b981" : "#3b4252";
    modalPlayerSyncStatus.style.color = isPlay ? "#34d399" : "#38bdf8";
  }
}

function updateCuesConfigUI(cfg) {
  if (!cfg) return;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (cfg.pre_warning_seconds !== undefined && cuePrewarnSecondsInput) {
    cuePrewarnSecondsInput.value = cfg.pre_warning_seconds;
  }
  if (cfg.auto_censor !== undefined && toggleCueAutocensor) {
    toggleCueAutocensor.checked = !!cfg.auto_censor;
    if (labelCueAutocensorStatus) {
      labelCueAutocensorStatus.textContent = cfg.auto_censor
        ? (isEn ? "Enabled (OBS + sound)" : "Включена (OBS + звук)")
        : (isEn ? "Disabled (HUD only)" : "Отключена (Только HUD)");
      labelCueAutocensorStatus.style.color = cfg.auto_censor ? "#38bdf8" : "#94a3b8";
    }
  }
  if (cfg.cue_notifications !== undefined && toggleCueNotifications) {
    toggleCueNotifications.checked = !!cfg.cue_notifications;
    if (labelCueNotificationsStatus) {
      labelCueNotificationsStatus.textContent = cfg.cue_notifications
        ? (isEn ? "Enabled (HUD)" : "Включены (HUD)")
        : (isEn ? "Disabled" : "Отключены");
      labelCueNotificationsStatus.style.color = cfg.cue_notifications ? "#38bdf8" : "#94a3b8";
    }
  }
  if (cfg.append_mode !== undefined) {
    isAppendCuesMode = !!cfg.append_mode;
    if (btnCueModeAppend && btnCueModeReplace) {
      if (isAppendCuesMode) {
        btnCueModeAppend.classList.add("active");
        btnCueModeReplace.classList.remove("active");
      } else {
        btnCueModeAppend.classList.remove("active");
        btnCueModeReplace.classList.add("active");
      }
    }
  }
}

function updateOcrStatusBadge(status) {
  if (!badgeOcrStatus || !status) return;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (status.all_ready) {
    badgeOcrStatus.className = "label label-success";
    badgeOcrStatus.textContent = isEn ? "WinRT OCR (Ready)" : "WinRT OCR (Готово)";
  } else {
    badgeOcrStatus.className = "label label-warning";
    badgeOcrStatus.textContent = isEn ? "WinRT OCR (Initializing)" : "WinRT OCR (Инициализация)";
  }
}
const updatePpOcrStatusBadge = updateOcrStatusBadge;

function escapeHtml(str) {
  if (!str) return "";
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function formatDuration(sec) {
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const s = Math.round(sec);
  if (s < 60) return isEn ? `${s}s` : `${s}с`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return rem > 0
    ? (isEn ? `${m}m ${rem}s` : `${m}м ${rem}с`)
    : (isEn ? `${m}m` : `${m}м`);
}

function renderCuesTable(cues) {
  currentScheduledCues = cues || [];
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (modalCuesTableCount) {
    modalCuesTableCount.textContent = currentScheduledCues.length;
  }
  if (cuesCountBadge) {
    cuesCountBadge.textContent = currentScheduledCues.length;
  }

  if (!cuesTableBody) return;

  if (currentScheduledCues.length === 0) {
    cuesTableBody.innerHTML = `
      <tr>
        <td colspan="5" style="text-align: center; color: #64748b; padding: 24px 12px;">
          ${isEn ? "No scheduled cues. Paste screenshot with timestamps via Ctrl+V." : "Нет запланированных таймингов. Вставьте скриншот с таймкодами через Ctrl+V."}
        </td>
      </tr>
    `;
    return;
  }

  cuesTableBody.innerHTML = "";

  currentScheduledCues.forEach(cue => {
    const tr = document.createElement("tr");

    let statusLabel = "";
    if (cue.status === "Approaching") {
      statusLabel = `<span class="label label-warning">${isEn ? "Warning" : "Оповещение"}</span>`;
    } else if (cue.status === "ActiveCensor") {
      statusLabel = `<span class="label label-danger">${isEn ? "BLOCK" : "БЛОКИРОВКА"}</span>`;
    } else if (cue.status === "Completed") {
      statusLabel = `<span class="label label-success">${isEn ? "Completed" : "Завершен"}</span>`;
    } else if (cue.status === "Dismissed") {
      statusLabel = `<span class="label label-info">${isEn ? "Dismissed (F8)" : "Пропущен (F8)"}</span>`;
    } else {
      statusLabel = `<span class="label label-default">${isEn ? "Planned" : "План"}</span>`;
    }

    const fallbackReason = isEn ? "Scheduled block" : "Запланированная блокировка";
    const deleteBtnText = isEn ? "Delete" : "Удалить";

    tr.innerHTML = `
      <td style="padding: 8px 12px; vertical-align: middle;">${statusLabel}</td>
      <td style="padding: 8px 12px; vertical-align: middle;">
        <code style="color: #38bdf8; background: #0c0f15; padding: 2px 6px; border-radius: 3px; font-size: 11.5px;">${escapeHtml(cue.formatted_range)}</code>
      </td>
      <td style="padding: 8px 12px; vertical-align: middle; color: #cbd5e1;">${formatDuration(cue.duration_sec)}</td>
      <td style="padding: 8px 12px; vertical-align: middle; color: #ffffff; font-weight: 500;">${escapeHtml(cue.reason || fallbackReason)}</td>
      <td style="padding: 8px 12px; vertical-align: middle; text-align: right;">
        <button type="button" class="btn btn-danger btn-xs btn-del-cue" data-id="${escapeHtml(cue.id)}">
          ${deleteBtnText}
        </button>
      </td>
    `;

    cuesTableBody.appendChild(tr);
  });

  // Attach delete buttons listeners
  cuesTableBody.querySelectorAll(".btn-del-cue").forEach(btn => {
    btn.addEventListener("click", () => {
      const id = btn.dataset.id;
      if (!id) return;
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: "delete_scheduled_cue", id }));
      }
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("delete_scheduled_cue", { id }).catch(() => { });
      }
    });
  });
}

function handleImageBlob(blob) {
  if (!blob || !blob.type.startsWith("image/")) return;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const reader = new FileReader();
  reader.onload = (e) => {
    currentPastedImageBase64 = e.target.result;
    if (cuePreviewThumb) cuePreviewThumb.src = currentPastedImageBase64;
    if (cuePreviewTitle) cuePreviewTitle.textContent = isEn ? "Screenshot loaded" : "Скриншот загружен";
    if (cuePreviewMeta) cuePreviewMeta.textContent = `${blob.name || (isEn ? "Image from clipboard" : "Изображение из буфера")} (${(blob.size / 1024).toFixed(1)} ${isEn ? "KB" : "КБ"})`;
    if (cuePreviewContainer) cuePreviewContainer.style.display = "flex";

    // Auto-open modal if user pasted from clipboard
    openCuesModal();
  };
  reader.readAsDataURL(blob);
}

function recognizePastedImage() {
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (!currentPastedImageBase64) {
    alert(isEn ? "First paste or upload a screenshot with timings (Ctrl+V)" : "Сначала вставьте или загрузите скриншот с таймингами (Ctrl+V)");
    return;
  }

  if (btnCueRecognizeOcr) {
    btnCueRecognizeOcr.disabled = true;
    btnCueRecognizeOcr.innerHTML = `
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="spin" style="animation: spin 1s linear infinite;"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
      ${isEn ? "Recognizing via Windows OCR..." : "Распознавание Windows OCR..."}
    `;
  }

  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "recognize_screenshot_cues",
      image: currentPastedImageBase64
    }));
  } else if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("recognize_cues_from_image", { imageBase64: currentPastedImageBase64 })
      .then(res => {
        handleRecognizedCuesResult(res);
      })
      .catch(err => {
        alert((isEn ? "OCR recognition error: " : "Ошибка распознавания OCR: ") + err);
      })
      .finally(() => {
        if (btnCueRecognizeOcr) {
          btnCueRecognizeOcr.disabled = false;
          btnCueRecognizeOcr.innerHTML = `
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
            ${isEn ? "Recognize via Windows OCR" : "Распознать через Windows OCR"}
          `;
        }
      });
  }
}

function handleRecognizedCuesResult(data) {
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (btnCueRecognizeOcr) {
    btnCueRecognizeOcr.disabled = false;
    btnCueRecognizeOcr.innerHTML = `
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
      ${isEn ? "Recognize via Windows OCR" : "Распознать через Windows OCR"}
    `;
  }

  if (data.raw_text && cueManualTextarea) {
    cueManualTextarea.value = data.raw_text;
  }

  const cues = data.cues || [];
  if (cues.length > 0) {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "add_scheduled_cues",
        cues: cues,
        append: isAppendCuesMode
      }));
    }
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("add_scheduled_cues", { cues, append: isAppendCuesMode })
        .then(updated => {
          if (Array.isArray(updated)) renderCuesTable(updated);
        }).catch(() => { });
    }
    renderEventItem({
      type: "success",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn ? `[WinRT OCR] Recognized and added ${cues.length} cues.` : `[WinRT OCR] Распознано и добавлено ${cues.length} таймингов.`
    });
  } else {
    renderEventItem({
      type: "warning",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn
        ? "[WinRT OCR] No cues found on image. Check screenshot clarity or enter manually."
        : "[WinRT OCR] Тайминги на изображении не найдены. Проверьте четкость скриншота или введите вручную."
    });
  }
}

function cleanCuesWhitespace(text) {
  if (!text) return "";
  return text
    .split("\n")
    .map(line => {
      let s = line.trim();
      if (!s) return "";
      // Replace non-breaking spaces, tabs, zero-width spaces
      s = s.replace(/[\u00A0\t\u200B\uFEFF]/g, " ");
      // Remove spaces around colons adjacent to digits
      s = s.replace(/(\d)\s*:\s*(\d)/g, "$1:$2");
      s = s.replace(/(\d)\s*:/g, "$1:");
      s = s.replace(/:\s*(\d)/g, ":$1");
      // Remove spaces around dots/commas between digits
      s = s.replace(/(\d)\s*[\.,]\s*(\d)/g, "$1.$2");
      // Remove spaces inside brackets around numbers
      s = s.replace(/\[\s*/g, "[").replace(/\s*\]/g, "]");
      s = s.replace(/\(\s*/g, "(").replace(/\s*\)/g, ")");
      // Remove accidental spaces between digits near colons
      s = s.replace(/(\d)\s+(\d)(?=:)/g, "$1$2");
      s = s.replace(/(?<=:\d)\s+(\d)/g, "$1");
      // Normalize range dash
      s = s.replace(/\s*[-—–~]\s*/g, " - ");
      // Collapse multiple spaces
      s = s.replace(/[ ]{2,}/g, " ");
      return s.trim();
    })
    .filter(Boolean)
    .join("\n");
}

function parseManualText() {
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  let text = cueManualTextarea ? cueManualTextarea.value.trim() : "";
  if (!text) {
    alert(isEn ? "Enter text with timings (e.g. 12:45-14:20 scene)" : "Введите текст с таймингами (например, 12:45-14:20 сцена)");
    return;
  }

  text = cleanCuesWhitespace(text);
  if (cueManualTextarea) {
    cueManualTextarea.value = text;
  }

  const lines = text.split("\n");
  const parsedCues = [];

  lines.forEach((line, idx) => {
    line = line.trim();
    if (!line) return;

    // Pattern matching time ranges
    const match = line.match(/(?:(?:(\d{1,2})[:.])?(\d{1,2})[:.](\d{1,2}))\s*[-—–~]\s*(?:(?:(\d{1,2})[:.])?(\d{1,2})[:.](\d{1,2}))(.*)/);
    if (match) {
      const h1 = match[1] ? parseInt(match[1], 10) : 0;
      const m1 = parseInt(match[2], 10);
      const s1 = parseInt(match[3], 10);
      const startSec = h1 * 3600 + m1 * 60 + s1;

      const h2 = match[4] ? parseInt(match[4], 10) : 0;
      const m2 = parseInt(match[5], 10);
      const s2 = parseInt(match[6], 10);
      let endSec = h2 * 3600 + m2 * 60 + s2;

      if (endSec <= startSec) {
        endSec = m2 * 3600 + s2 * 60;
      }

      if (endSec > startSec) {
        const fallbackReason = isEn ? "Scheduled block" : "Запланированная блокировка";
        const reason = (match[7] || "").replace(/^[\s,;:-]+/, "").trim() || fallbackReason;
        const fmt = `${String(Math.floor(startSec / 60)).padStart(2, "0")}:${String(startSec % 60).padStart(2, "0")} – ${String(Math.floor(endSec / 60)).padStart(2, "0")}:${String(endSec % 60).padStart(2, "0")}`;
        parsedCues.push({
          id: `manual-${Date.now()}-${idx}`,
          start_sec: startSec,
          end_sec: endSec,
          duration_sec: endSec - startSec,
          formatted_range: fmt,
          reason,
          raw_text: line,
          status: "Pending"
        });
      }
    }
  });

  if (parsedCues.length > 0) {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        type: "add_scheduled_cues",
        cues: parsedCues,
        append: isAppendCuesMode
      }));
    }
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("add_scheduled_cues", { cues: parsedCues, append: isAppendCuesMode })
        .then(updated => {
          if (Array.isArray(updated)) renderCuesTable(updated);
        }).catch(() => { });
    }
    renderEventItem({
      type: "success",
      timestamp: new Date().toTimeString().split(" ")[0],
      message: isEn ? `[Parser] Recognized ${parsedCues.length} cues from text.` : `[Парсер] Распознано ${parsedCues.length} таймингов из текста.`
    });
  } else {
    alert(isEn
      ? "Could not recognize timecodes. Specify interval in MM:SS-MM:SS format (e.g. 12:30-14:15)"
      : "Не удалось распознать таймкоды. Укажите интервал в формате MM:SS-MM:SS (например 12:30-14:15)");
  }
}

function saveCuesConfig(partial) {
  const cfg = {
    pre_warning_seconds: parseFloat(cuePrewarnSecondsInput?.value) || 20,
    auto_censor: !!toggleCueAutocensor?.checked,
    cue_notifications: !!toggleCueNotifications?.checked,
    append_mode: isAppendCuesMode,
    ...partial
  };
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({
      type: "set_cues_config",
      ...cfg
    }));
  }
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("set_cues_config", {
      preWarningSeconds: cfg.pre_warning_seconds,
      autoCensor: cfg.auto_censor,
      cueNotifications: cfg.cue_notifications,
      appendMode: cfg.append_mode
    }).catch(() => { });
  }
}

// Modal Trigger Event Listeners
if (btnOpenCuesModal) btnOpenCuesModal.addEventListener("click", openCuesModal);
if (btnOpenCuesAction) btnOpenCuesAction.addEventListener("click", openCuesModal);
if (btnCloseCues) btnCloseCues.addEventListener("click", closeCuesModal);
if (btnCloseCuesX) btnCloseCuesX.addEventListener("click", closeCuesModal);
if (modalCuesBackdrop) modalCuesBackdrop.addEventListener("click", closeCuesModal);

// Drag and Drop & File Input
if (cueScreenshotDropzone) {
  cueScreenshotDropzone.addEventListener("click", () => {
    if (cueFileInput) cueFileInput.click();
  });

  cueScreenshotDropzone.addEventListener("dragover", (e) => {
    e.preventDefault();
    cueScreenshotDropzone.style.borderColor = "#38bdf8";
    cueScreenshotDropzone.style.background = "#141a24";
  });

  cueScreenshotDropzone.addEventListener("dragleave", (e) => {
    e.preventDefault();
    cueScreenshotDropzone.style.borderColor = "#333f52";
    cueScreenshotDropzone.style.background = "#0e1117";
  });

  cueScreenshotDropzone.addEventListener("drop", (e) => {
    e.preventDefault();
    cueScreenshotDropzone.style.borderColor = "#333f52";
    cueScreenshotDropzone.style.background = "#0e1117";
    if (e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      handleImageBlob(e.dataTransfer.files[0]);
    }
  });
}

if (cueFileInput) {
  cueFileInput.addEventListener("change", (e) => {
    if (e.target.files && e.target.files.length > 0) {
      handleImageBlob(e.target.files[0]);
    }
  });
}

// Global Clipboard Paste (Ctrl+V)
window.addEventListener("paste", (e) => {
  const target = e.target;
  const isTextInput = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA");

  if (e.clipboardData) {
    const items = e.clipboardData.items;
    if (items) {
      for (let i = 0; i < items.length; i++) {
        if (items[i].type.startsWith("image/")) {
          const blob = items[i].getAsFile();
          if (blob) {
            e.preventDefault();
            handleImageBlob(blob);
            return;
          }
        }
      }
    }

    if (!isTextInput && modalCuesManager && modalCuesManager.classList.contains("in")) {
      const text = e.clipboardData.getData("text");
      if (text && cueManualTextarea) {
        cueManualTextarea.value = cleanCuesWhitespace(text);
      }
    }
  }
});

// Control Listeners
if (cuePrewarnSecondsInput) {
  cuePrewarnSecondsInput.addEventListener("change", () => {
    const val = parseFloat(cuePrewarnSecondsInput.value) || 20;
    saveCuesConfig({ pre_warning_seconds: val });
  });
}

if (toggleCueAutocensor) {
  toggleCueAutocensor.addEventListener("change", () => {
    const val = !!toggleCueAutocensor.checked;
    if (labelCueAutocensorStatus) {
      const _isEnCueStVal = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
      labelCueAutocensorStatus.textContent = val ? (_isEnCueStVal ? "Enabled (OBS + Audio)" : "Включена (OBS + звук)") : (_isEnCueStVal ? "Disabled (HUD Only)" : "Отключена (Только HUD)");
      labelCueAutocensorStatus.style.color = val ? "#38bdf8" : "#94a3b8";
    }
    saveCuesConfig({ auto_censor: val });
  });
}

if (toggleCueNotifications) {
  toggleCueNotifications.addEventListener("change", () => {
    const val = !!toggleCueNotifications.checked;
    if (labelCueNotificationsStatus) {
      const _isEnNotifVal = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
      labelCueNotificationsStatus.textContent = val ? (_isEnNotifVal ? "Enabled (HUD)" : "Включены (HUD)") : (_isEnNotifVal ? "Disabled" : "Отключены");
      labelCueNotificationsStatus.style.color = val ? "#38bdf8" : "#94a3b8";
    }
    saveCuesConfig({ cue_notifications: val });
  });
}

if (btnCueModeAppend && btnCueModeReplace) {
  btnCueModeAppend.addEventListener("click", () => {
    isAppendCuesMode = true;
    btnCueModeAppend.classList.add("active");
    btnCueModeReplace.classList.remove("active");
    saveCuesConfig({ append_mode: true });
  });
  btnCueModeReplace.addEventListener("click", () => {
    isAppendCuesMode = false;
    btnCueModeAppend.classList.remove("active");
    btnCueModeReplace.classList.add("active");
    saveCuesConfig({ append_mode: false });
  });
}

if (btnCueClearPreview) {
  btnCueClearPreview.addEventListener("click", () => {
    currentPastedImageBase64 = "";
    if (cuePreviewThumb) cuePreviewThumb.src = "";
    if (cuePreviewContainer) cuePreviewContainer.style.display = "none";
    if (cueFileInput) cueFileInput.value = "";
  });
}

if (btnCueRecognizeOcr) {
  btnCueRecognizeOcr.addEventListener("click", recognizePastedImage);
}

if (btnCueParseManualText) {
  btnCueParseManualText.addEventListener("click", parseManualText);
}

if (btnClearAllCues) {
  btnClearAllCues.addEventListener("click", () => {
    const _isEnClr = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    if (!confirm(_isEnClr ? "Clear all scheduled timings?" : "Очистить все запланированные тайминги?")) return;
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "clear_scheduled_cues" }));
    }
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("clear_scheduled_cues").catch(() => { });
    }
    renderCuesTable([]);
  });
}

// Global Escape Key Modal Closer
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (modalStopwords && modalStopwords.classList.contains("in")) closeStopwordsModal();
    if (modalHotkeys && modalHotkeys.classList.contains("in")) closeHotkeysModal();
    if (modalAbout && modalAbout.classList.contains("in")) closeAboutModal();
    if (modalModels && modalModels.classList.contains("in")) closeModelsModal();
    if (modalBrowserExtension && modalBrowserExtension.classList.contains("in")) closeExtensionModal();
    if (modalCuesManager && modalCuesManager.classList.contains("in")) closeCuesModal();
    if (modalObsSetupPrompt && modalObsSetupPrompt.classList.contains("in")) closeObsSetupPrompt();
    if (typeof modalModelTuning !== "undefined" && modalModelTuning && modalModelTuning.classList.contains("in")) closeModelTuningModal();
  }
});

// =============================================================================
// MODEL TUNING & PROFILES CONTROLLER (Bootstrap 3 Dark Edition)
// =============================================================================
let availableModelProfiles = [
  {
    name: "Gaming",
    profile: "gaming",
    exact_rules: "preset:gaming,vit_filter:on,min_conf:0.38,hold:12",
    vit_game_filter: true,
    nudenet_min_confidence: 0.38,
    tracker_hold_frames: 12,
    is_preset: true
  },
  {
    name: "RealLife",
    profile: "reallife",
    exact_rules: "preset:reallife,vit_filter:off,min_conf:0.25,hold:25",
    vit_game_filter: false,
    nudenet_min_confidence: 0.25,
    tracker_hold_frames: 25,
    is_preset: true
  },
  {
    name: "Strict",
    profile: "strict",
    exact_rules: "preset:strict,vit_filter:off,min_conf:0.15,hold:30",
    vit_game_filter: false,
    nudenet_min_confidence: 0.15,
    tracker_hold_frames: 30,
    is_preset: true
  }
];
let activeModelProfileName = "Gaming";
let activeModelTuning = Object.assign({}, availableModelProfiles[0]);

const modalModelTuning = document.getElementById("modal-model-tuning");
const modalModelTuningBackdrop = document.getElementById("modal-model-tuning-backdrop");
const btnOpenTuningFromCascade = document.getElementById("btn-open-tuning-from-cascade");
const btnCloseModelTuningX = document.getElementById("btn-close-model-tuning-x");
const btnCloseModelTuning = document.getElementById("btn-close-model-tuning");

const modelProfileSelect = document.getElementById("model-profile-select");
const btnDeleteModelProfile = document.getElementById("btn-delete-model-profile");
const btnOpenProfilesFolder = document.getElementById("btn-open-profiles-folder");
const modelProfileDesc = document.getElementById("model-profile-desc");

const toggleTuningVitFilter = document.getElementById("toggle-tuning-vit-filter");
const sliderTuningNudenetCutoff = document.getElementById("slider-tuning-nudenet-cutoff");
const valTuningNudenetCutoff = document.getElementById("val-tuning-nudenet-cutoff");
const sliderTuningTrackerHold = document.getElementById("slider-tuning-tracker-hold");
const valTuningTrackerHold = document.getElementById("val-tuning-tracker-hold");
const inputTuningExactRule = document.getElementById("input-tuning-exact-rule");
const tuningRuleStatus = document.getElementById("tuning-rule-status");
const btnApplyTuningRule = document.getElementById("btn-apply-tuning-rule");

const inputSaveProfileName = document.getElementById("input-save-profile-name");
const btnSaveModelProfile = document.getElementById("btn-save-model-profile");

function isProtectedModelPreset(name) {
  if (!name) return false;
  const n = name.trim().toLowerCase();
  return n === "gaming" || n === "reallife" || n === "strict" || n === "irl";
}

function openModelTuningModal() {
  if (modalModelTuning && modalModelTuningBackdrop) {
    modalModelTuning.style.display = "block";
    modalModelTuningBackdrop.style.display = "block";
    setTimeout(() => {
      modalModelTuning.classList.add("in");
      modalModelTuningBackdrop.classList.add("in");
    }, 10);
  }
}

function closeModelTuningModal() {
  if (modalModelTuning && modalModelTuningBackdrop) {
    modalModelTuning.classList.remove("in");
    modalModelTuningBackdrop.classList.remove("in");
    setTimeout(() => {
      modalModelTuning.style.display = "none";
      modalModelTuningBackdrop.style.display = "none";
    }, 150);
  }
}

if (btnOpenTuningFromCascade) {
  btnOpenTuningFromCascade.addEventListener("click", openModelTuningModal);
}
if (btnCloseModelTuningX) {
  btnCloseModelTuningX.addEventListener("click", closeModelTuningModal);
}
if (btnCloseModelTuning) {
  btnCloseModelTuning.addEventListener("click", closeModelTuningModal);
}
if (modalModelTuning) {
  modalModelTuning.addEventListener("click", (e) => {
    if (e.target === modalModelTuning) closeModelTuningModal();
  });
}

function updateModelProfilesList(profiles, activeName) {
  if (Array.isArray(profiles) && profiles.length > 0) {
    availableModelProfiles = profiles;
  }
  if (activeName) {
    activeModelProfileName = activeName;
  }
  renderModelProfilesDropdown();
}

function renderModelProfilesDropdown() {
  if (!modelProfileSelect) return;
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  modelProfileSelect.innerHTML = "";

  availableModelProfiles.forEach((prof) => {
    const opt = document.createElement("option");
    opt.value = prof.name;
    const lower = prof.name.toLowerCase();

    let displayLabel = prof.name;
    if (lower === "gaming") {
      displayLabel = isEn ? "Gaming / 3D & Anime (Preset)" : "Игры / 3D & Аниме (Пресет)";
    } else if (lower === "reallife" || lower === "irl") {
      displayLabel = isEn ? "Real Life / IRL (Preset)" : "Реальный контент / IRL (Пресет)";
    } else if (lower === "strict") {
      displayLabel = isEn ? "Strict / Paranoia (Preset)" : "Макс. защита (Пресет)";
    } else {
      displayLabel = `${prof.name} (${isEn ? "Custom" : "Пользовательский"})`;
    }

    opt.textContent = displayLabel;
    if (prof.name.toLowerCase() === activeModelProfileName.toLowerCase()) {
      opt.selected = true;
    }
    modelProfileSelect.appendChild(opt);
  });

  updateProfileActionsState();
}

function updateProfileActionsState() {
  const isProtected = isProtectedModelPreset(activeModelProfileName);
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");

  if (btnDeleteModelProfile) {
    if (isProtected) {
      btnDeleteModelProfile.disabled = true;
      btnDeleteModelProfile.style.opacity = "0.45";
      btnDeleteModelProfile.style.cursor = "not-allowed";
    } else {
      btnDeleteModelProfile.disabled = false;
      btnDeleteModelProfile.style.opacity = "1.0";
      btnDeleteModelProfile.style.cursor = "pointer";
    }
  }

  if (modelProfileDesc) {
    const lower = activeModelProfileName.toLowerCase();
    if (lower === "gaming") {
      modelProfileDesc.textContent = isEn
        ? "Gaming mode: active ViT cel-shading filter, minimum 0.38 NudeNet cutoff, and 12-frame retention to eliminate false positives on 3D textures, avatar skins, and polygon shadows."
        : "Игровой режим: фильтр 3D/аниме в ViT, порог 0.38 в NudeNet и 12 кадров удержания для исключения ложных срабатываний на текстуры скинов и тени полигонов.";
    } else if (lower === "reallife" || lower === "irl") {
      modelProfileDesc.textContent = isEn
        ? "Real-life mode: balanced IRL human anatomy detection with 0.25 threshold and 25-frame retention for live webcams."
        : "Реальный контент: сбалансированная детекция анатомии человека (порог 0.25, удержание 25 кадров) для вебкамер и IRL-стримов.";
    } else if (lower === "strict") {
      modelProfileDesc.textContent = isEn
        ? "Strict paranoia mode: maximum sensitivity (0.15 cutoff, 30-frame hold) with zero tolerance for exposed skin."
        : "Строгий режим: максимальная чувствительность детектора (порог 0.15, удержание 30 кадров) при малейшем подозрении.";
    } else {
      modelProfileDesc.textContent = isEn
        ? `Custom saved profile "${activeModelProfileName}". Your fine-tuned thresholds and filters are active.`
        : `Пользовательский профиль «${activeModelProfileName}». Активны ваши персональные пороги и настройки.`;
    }
  }
}

function applyModelTuningToUI(tuning, profileName) {
  if (!tuning) return;
  activeModelTuning = Object.assign({}, activeModelTuning, tuning);
  if (profileName) {
    activeModelProfileName = profileName;
  } else if (tuning.name) {
    activeModelProfileName = tuning.name;
  }

  if (toggleTuningVitFilter) {
    toggleTuningVitFilter.checked = !!activeModelTuning.vit_game_filter;
  }
  if (sliderTuningNudenetCutoff && valTuningNudenetCutoff) {
    const conf = typeof activeModelTuning.nudenet_min_confidence === "number" ? activeModelTuning.nudenet_min_confidence : 0.38;
    sliderTuningNudenetCutoff.value = conf.toFixed(2);
    valTuningNudenetCutoff.textContent = conf.toFixed(2);
  }
  if (sliderTuningTrackerHold && valTuningTrackerHold) {
    const hold = activeModelTuning.tracker_hold_frames || 12;
    sliderTuningTrackerHold.value = hold;
    valTuningTrackerHold.textContent = hold;
  }
  if (inputTuningExactRule) {
    inputTuningExactRule.value = activeModelTuning.exact_rules || buildExactRuleString();
  }
  if (tuningRuleStatus) {
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    tuningRuleStatus.textContent = isEn ? "Synchronized" : "Синхронизировано";
    tuningRuleStatus.className = "label label-info";
  }

  renderModelProfilesDropdown();
}

function buildExactRuleString() {
  const pName = activeModelProfileName ? activeModelProfileName.toLowerCase() : "gaming";
  const vf = toggleTuningVitFilter && toggleTuningVitFilter.checked ? "on" : "off";
  const mc = sliderTuningNudenetCutoff ? parseFloat(sliderTuningNudenetCutoff.value).toFixed(2) : "0.38";
  const hd = sliderTuningTrackerHold ? parseInt(sliderTuningTrackerHold.value, 10) : 12;
  return `preset:${pName},vit_filter:${vf},min_conf:${mc},hold:${hd}`;
}

function notifyModelTuningChanged() {
  const vit = toggleTuningVitFilter ? toggleTuningVitFilter.checked : true;
  const nudenetConf = sliderTuningNudenetCutoff ? parseFloat(sliderTuningNudenetCutoff.value) : 0.38;
  const holdFrames = sliderTuningTrackerHold ? parseInt(sliderTuningTrackerHold.value, 10) : 12;
  const exactRule = buildExactRuleString();

  if (inputTuningExactRule) {
    inputTuningExactRule.value = exactRule;
  }
  if (tuningRuleStatus) {
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    tuningRuleStatus.textContent = isEn ? "Custom Modified" : "Пользовательские";
    tuningRuleStatus.className = "label label-warning";
  }

  activeModelTuning = {
    name: activeModelProfileName,
    profile: isProtectedModelPreset(activeModelProfileName) ? activeModelProfileName.toLowerCase() : "custom",
    exact_rules: exactRule,
    vit_game_filter: vit,
    nudenet_min_confidence: nudenetConf,
    tracker_hold_frames: holdFrames,
    is_preset: isProtectedModelPreset(activeModelProfileName)
  };

  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("set_model_tuning", { tuning: activeModelTuning }).catch(() => {});
  }
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: "set_model_tuning", tuning: activeModelTuning }));
  }
}

if (modelProfileSelect) {
  modelProfileSelect.addEventListener("change", (e) => {
    const selectedName = e.target.value;
    activeModelProfileName = selectedName;
    const found = availableModelProfiles.find(p => p.name.toLowerCase() === selectedName.toLowerCase());
    if (found) {
      applyModelTuningToUI(found, selectedName);
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("set_model_tuning", { tuning: found }).catch(() => {});
      }
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: "set_model_tuning", tuning: found }));
      }
    }
    updateProfileActionsState();
  });
}

if (toggleTuningVitFilter) {
  toggleTuningVitFilter.addEventListener("change", notifyModelTuningChanged);
}

if (sliderTuningNudenetCutoff) {
  sliderTuningNudenetCutoff.addEventListener("input", (e) => {
    if (valTuningNudenetCutoff) valTuningNudenetCutoff.textContent = parseFloat(e.target.value).toFixed(2);
    notifyModelTuningChanged();
  });
}

if (sliderTuningTrackerHold) {
  sliderTuningTrackerHold.addEventListener("input", (e) => {
    if (valTuningTrackerHold) valTuningTrackerHold.textContent = e.target.value;
    notifyModelTuningChanged();
  });
}

if (btnApplyTuningRule) {
  btnApplyTuningRule.addEventListener("click", () => {
    const ruleStr = inputTuningExactRule ? inputTuningExactRule.value.trim() : "";
    if (!ruleStr) return;

    // Parse rule string: preset:gaming,vit_filter:on,min_conf:0.38,hold:12
    const parts = ruleStr.split(",");
    parts.forEach(part => {
      const kv = part.split(":");
      if (kv.length === 2) {
        const k = kv[0].trim().toLowerCase();
        const v = kv[1].trim().toLowerCase();
        if (k === "vit_filter" && toggleTuningVitFilter) {
          toggleTuningVitFilter.checked = (v === "on" || v === "true" || v === "1");
        } else if (k === "min_conf" && sliderTuningNudenetCutoff) {
          const num = parseFloat(v);
          if (!isNaN(num)) {
            sliderTuningNudenetCutoff.value = num.toFixed(2);
            if (valTuningNudenetCutoff) valTuningNudenetCutoff.textContent = num.toFixed(2);
          }
        } else if (k === "hold" && sliderTuningTrackerHold) {
          const num = parseInt(v, 10);
          if (!isNaN(num)) {
            sliderTuningTrackerHold.value = num;
            if (valTuningTrackerHold) valTuningTrackerHold.textContent = num;
          }
        }
      }
    });

    notifyModelTuningChanged();
    const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
    if (typeof showToast === "function") {
      showToast(isEn ? "Model rules applied" : "Правила моделей обновлены");
    }
  });
}

// Save New Profile Action
async function handleSaveNewModelProfile() {
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  const name = inputSaveProfileName ? inputSaveProfileName.value.trim() : "";
  if (!name) {
    if (typeof showToast === "function") {
      showToast(isEn ? "Enter a profile name" : "Введите название профиля");
    } else {
      alert(isEn ? "Enter a profile name" : "Введите название профиля");
    }
    return;
  }

  if (isProtectedModelPreset(name)) {
    const msg = isEn
      ? "Built-in presets Gaming, RealLife, and Strict cannot be overwritten"
      : "Встроенные пресеты Gaming, RealLife и Strict нельзя перезаписать";
    if (typeof showToast === "function") {
      showToast(msg);
    } else {
      alert(msg);
    }
    return;
  }

  const vit = toggleTuningVitFilter ? toggleTuningVitFilter.checked : true;
  const nudenetConf = sliderTuningNudenetCutoff ? parseFloat(sliderTuningNudenetCutoff.value) : 0.38;
  const holdFrames = sliderTuningTrackerHold ? parseInt(sliderTuningTrackerHold.value, 10) : 12;
  const ruleStr = `preset:${name.toLowerCase()},vit_filter:${vit ? "on" : "off"},min_conf:${nudenetConf.toFixed(2)},hold:${holdFrames}`;

  const newProfile = {
    name: name,
    profile: "custom",
    exact_rules: ruleStr,
    vit_game_filter: vit,
    nudenet_min_confidence: nudenetConf,
    tracker_hold_frames: holdFrames,
    is_preset: false
  };

  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      const updatedList = await window.__TAURI__.core.invoke("save_model_profile", { profile: newProfile });
      if (Array.isArray(updatedList)) {
        availableModelProfiles = updatedList;
      }
    }
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "save_model_profile", profile: newProfile }));
    }

    activeModelProfileName = name;
    activeModelTuning = newProfile;
    if (inputSaveProfileName) inputSaveProfileName.value = "";

    renderModelProfilesDropdown();
    updateProfileActionsState();

    const savedMsg = isEn ? `Profile "${name}" saved successfully` : `Профиль «${name}» успешно сохранен`;
    if (typeof showToast === "function") {
      showToast(savedMsg);
    }
  } catch (err) {
    console.error("[Tuning] Failed to save profile:", err);
    alert(isEn ? `Failed to save profile: ${err}` : `Ошибка сохранения профиля: ${err}`);
  }
}

if (btnSaveModelProfile) {
  btnSaveModelProfile.addEventListener("click", handleSaveNewModelProfile);
}

if (inputSaveProfileName) {
  inputSaveProfileName.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSaveNewModelProfile();
    }
  });
}

// Delete Profile Action
async function handleDeleteModelProfile() {
  const isEn = (window.I18N && window.I18N.getLanguage() === "en") || (typeof currentLanguage !== "undefined" && currentLanguage === "en");
  if (isProtectedModelPreset(activeModelProfileName)) {
    const msg = isEn
      ? "Built-in presets Gaming, RealLife, and Strict cannot be deleted"
      : "Встроенные пресеты Gaming, RealLife и Strict нельзя удалить";
    if (typeof showToast === "function") showToast(msg);
    return;
  }

  const confirmMsg = isEn
    ? `Delete saved profile "${activeModelProfileName}"?`
    : `Удалить сохраненный профиль «${activeModelProfileName}»?`;
  if (!confirm(confirmMsg)) return;

  try {
    const deletingName = activeModelProfileName;
    if (window.__TAURI__ && window.__TAURI__.core) {
      const updatedList = await window.__TAURI__.core.invoke("delete_model_profile", { name: deletingName });
      if (Array.isArray(updatedList)) {
        availableModelProfiles = updatedList;
      }
    }
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "delete_model_profile", name: deletingName }));
    }

    activeModelProfileName = "Gaming";
    const gaming = availableModelProfiles.find(p => p.name.toLowerCase() === "gaming");
    if (gaming) {
      applyModelTuningToUI(gaming, "Gaming");
    } else {
      renderModelProfilesDropdown();
    }

    if (typeof showToast === "function") {
      showToast(isEn ? "Profile deleted" : "Профиль удален");
    }
  } catch (err) {
    console.error("[Tuning] Failed to delete profile:", err);
  }
}

if (btnDeleteModelProfile) {
  btnDeleteModelProfile.addEventListener("click", handleDeleteModelProfile);
}

// Open Profiles Folder in Explorer
function handleOpenProfilesFolder() {
  if (window.__TAURI__ && window.__TAURI__.core) {
    window.__TAURI__.core.invoke("open_profiles_folder").catch((err) => {
      console.error("[Tuning] Failed to open profiles folder:", err);
    });
  }
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: "open_profiles_folder" }));
  }
}

if (btnOpenProfilesFolder) {
  btnOpenProfilesFolder.addEventListener("click", handleOpenProfilesFolder);
}

// ==============================================================================
// BILINGUAL LOCALIZATION SYSTEM (RU / EN)
// ==============================================================================
currentLanguage = (window.I18N && window.I18N.getLanguage()) || localStorage.getItem("blewred_lang") || "en";

function setLanguage(lang) {
  currentLanguage = (lang === "en") ? "en" : "ru";
  try {
    localStorage.setItem("blewred_lang", currentLanguage);
  } catch (e) { }
  document.documentElement.lang = currentLanguage;

  if (window.I18N) {
    window.I18N.setLanguage(currentLanguage);
    window.I18N.applyToDOM(currentLanguage);
  }
  updateOcrStatusUI();

  // Broadcast language change to WebSocket clients (e.g. Censor Shield in OBS) and persist via Tauri IPC
  if (typeof socket !== "undefined" && socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: "set_language", language: currentLanguage }));
  }
  if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === "function") {
    window.__TAURI__.core.invoke("set_language", { language: currentLanguage }).catch(() => { });
  }

  // Update Toolbar Language Switcher button states
  const btnRu = document.getElementById("btn-lang-ru");
  const btnEn = document.getElementById("btn-lang-en");
  if (btnRu && btnEn) {
    if (currentLanguage === "en") {
      btnRu.classList.remove("active");
      btnEn.classList.add("active");
    } else {
      btnEn.classList.remove("active");
      btnRu.classList.add("active");
    }
  }

  const isEn = (currentLanguage === "en");

  // 1. Header status ribbon pills
  const pObs = document.querySelector("#obs-pill .pill-title"); if (pObs) pObs.textContent = isEn ? "OBS Studio" : "OBS Studio";
  const pPlugin = document.querySelector("#obs-plugin-pill .pill-title"); if (pPlugin) pPlugin.textContent = isEn ? "OBS Plugin" : "Плагин OBS";
  const pScreen = document.querySelector("#screen-pill .pill-title"); if (pScreen) pScreen.textContent = isEn ? "Capture" : "Видеозахват";
  const pShield = document.querySelector("#shield-pill .pill-title"); if (pShield) pShield.textContent = isEn ? "OBS Shield" : "Защита OBS";
  const pExt = document.querySelector("#extension-pill .pill-title"); if (pExt) pExt.textContent = isEn ? "Extension" : "Расширение";
  const pVis = document.querySelector("#vision-pill .pill-title"); if (pVis) pVis.textContent = isEn ? "Vision" : "Видеоанализ";

  // 2. Streamer Setup Guide panel
  const guideTitle = document.querySelector(".setup-guide-title");
  if (guideTitle) guideTitle.textContent = isEn ? "Streamer Quick Start: Recommended Setup Steps" : "Быстрый старт стримера: Рекомендуемые шаги настройки";

  const step1Title = document.querySelector("#step-card-gpu .step-title");
  if (step1Title) step1Title.textContent = isEn ? "GPU Hardware Acceleration (DirectML)" : "Аппаратное ускорение GPU (DirectML)";
  const step1Sub = document.querySelector("#step-card-gpu .step-subtitle");
  if (step1Sub) step1Sub.textContent = isEn ? "DirectML hardware acceleration (8–14 ms)" : "DirectML аппаратное ускорение (8–14 мс)";
  const bGraph = document.getElementById("btn-open-graphics-settings");
  if (bGraph) {
    bGraph.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 3px;"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg> ${isEn ? "Windows Graphics Settings" : "Настройки графики Windows"}`;
  }

  const step2Title = document.querySelector("#step-card-models .step-title");
  if (step2Title) step2Title.textContent = isEn ? "AI Neural Network Models" : "Модели нейросетей";
  const step2Sub = document.querySelector("#step-card-models .step-subtitle");
  if (step2Sub) step2Sub.textContent = isEn ? "ViT screener + NudeNet 640m localizer" : "ViT скринер + NudeNet 640m локализатор";
  const bMod = document.getElementById("btn-open-models-setup");
  if (bMod) {
    bMod.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 3px;"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg> ${isEn ? "Download Models" : "Загрузка моделей"}`;
  }

  const step3Title = document.querySelector("#step-card-ext .step-title");
  if (step3Title) step3Title.textContent = isEn ? "Browser Extension" : "Браузерное расширение";
  const step3Sub = document.querySelector("#step-card-ext .step-subtitle");
  if (step3Sub) step3Sub.textContent = isEn ? "Player timeline sync and timings" : "Синхронизация таймлайна плеера и таймингов";
  const bExt = document.getElementById("btn-open-ext-folder");
  if (bExt) {
    bExt.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 3px;"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg> ${isEn ? "Open Extension Folder" : "Открыть папку расширения"}`;
  }

  const step4Title = document.querySelector("#step-card-obs .step-title");
  if (step4Title) step4Title.textContent = isEn ? "OBS Studio Integration" : "Связка с OBS Studio";
  const step4Sub = document.querySelector("#step-card-obs .step-subtitle");
  if (step4Sub) step4Sub.textContent = isEn ? "Hardware shader filter & Shield overlay" : "Аппаратный шейдерный фильтр & Заставка Shield";
  const bObsSetup = document.getElementById("btn-trigger-obs-setup");
  if (bObsSetup) {
    bObsSetup.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 3px;"><polygon points="5 3 19 12 5 21 5 3"/></svg> ${isEn ? "Auto-Setup OBS" : "Автонастройка OBS"}`;
  }

  // 3. Toolbar buttons
  const bCues = document.getElementById("btn-open-cues-modal");
  if (bCues) {
    const badge = document.getElementById("cues-count-badge");
    const count = badge ? badge.textContent : "0";
    bCues.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px;"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg> ${isEn ? "Player Timings" : "Тайминги и планировщик"} <span class="badge" id="cues-count-badge" style="background: #0f172a; border: 1px solid #334155; color: #f8fafc; margin-left: 3px;">${count}</span>`;
  }
  const bStop = document.getElementById("btn-open-stopwords-modal");
  if (bStop) {
    bStop.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px; margin-right: 4px;"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg> ${isEn ? "OCR Stopwords" : "Стоп-слова OCR"}`;
  }
  const bNsfwSim = document.getElementById("btn-run-nsfw-sim");
  if (bNsfwSim) {
    bNsfwSim.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg> <span>${isEn ? "Test NSFW" : "Тест NSFW"}</span>`;
  }
  const ocrLabel = document.querySelector(".ocr-toggle-label");
  if (ocrLabel) ocrLabel.textContent = isEn ? "OCR Detection:" : "OCR детекция:";
  const bHotkeys = document.getElementById("btn-open-hotkeys-modal");
  if (bHotkeys) {
    bHotkeys.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -1px;"><rect x="2" y="4" width="20" height="16" rx="2" ry="2"/><line x1="6" y1="8" x2="6.01" y2="8"/><line x1="10" y1="8" x2="10.01" y2="8"/><line x1="14" y1="8" x2="14.01" y2="8"/><line x1="18" y1="8" x2="18.01" y2="8"/><line x1="8" y1="12" x2="8.01" y2="12"/><line x1="12" y1="12" x2="12.01" y2="12"/><line x1="16" y1="12" x2="16.01" y2="12"/><line x1="7" y1="16" x2="17" y2="16"/></svg> ${isEn ? "Hotkeys" : "Горячие клавиши"}`;
  }

  // 4. Stream & Censor Control Panel
  const mPlayer = document.querySelector("#mode-btn-player .mode-name"); if (mPlayer) mPlayer.textContent = isEn ? "Player Analysis" : "Анализ плеера";
  const mPlayerSub = document.querySelector("#mode-btn-player .mode-sub"); if (mPlayerSub) mPlayerSub.textContent = isEn ? "Browser Lookahead (CPU < 1%)" : "Lookahead в браузере (CPU < 1%)";
  const mScreen = document.querySelector("#mode-btn-screen .mode-name"); if (mScreen) mScreen.textContent = isEn ? "Screen Analysis" : "Анализ экрана";
  const mScreenSub = document.querySelector("#mode-btn-screen .mode-sub"); if (mScreenSub) mScreenSub.textContent = isEn ? "Screen Capture + OCR" : "Захват экрана + OCR";
  const mHybrid = document.querySelector("#mode-btn-hybrid .mode-name"); if (mHybrid) mHybrid.textContent = isEn ? "Hybrid" : "Гибридный";
  const mHybridSub = document.querySelector("#mode-btn-hybrid .mode-sub"); if (mHybridSub) mHybridSub.textContent = isEn ? "Screen + Player (Max Protection)" : "Экран + Плеер (Макс. защита)";
  const mOff = document.querySelector("#mode-btn-off .mode-name"); if (mOff) mOff.textContent = isEn ? "Protection Off" : "Защита выкл";
  const mOffSub = document.querySelector("#mode-btn-off .mode-sub"); if (mOffSub) mOffSub.textContent = isEn ? "Standby mode" : "Пауза всех режимов (Standby)";

  // Refresh operation mode UI badges
  updateOperationModeUI(currentOperationMode);

  // 5. Lookahead Warning HUD Panel
  updateLookaheadHudButtons();
  const unblurredTag = document.querySelector(".lookahead-unblurred-tag");
  if (unblurredTag) unblurredTag.textContent = isEn ? "UNCENSORED PREVIEW (STREAMER ONLY)" : "ПРЕДПРОСМОТР БЕЗ БЛЮРА (ТОЛЬКО ДЛЯ СТРИМЕРА)";
  const reasonSub = document.querySelector(".lookahead-label-sub");
  if (reasonSub) reasonSub.textContent = isEn ? "DETECTED CONTENT:" : "ОБНАРУЖЕННЫЙ КОНТЕНТ:";
  const timerMeta = document.querySelector(".lookahead-timer-meta span");
  if (timerMeta) timerMeta.textContent = isEn ? "TIME BEFORE AUDIENCE SEES:" : "ВРЕМЯ ДО ПОКАЗА ЗРИТЕЛЯМ:";

  // 6. Selective Censor Chips
  const chipGen = document.querySelector("#chip-genitalia .chip-label"); if (chipGen) chipGen.textContent = isEn ? "Genitalia" : "Гениталии";
  const chipBreasts = document.querySelector("#chip-breasts .chip-label"); if (chipBreasts) chipBreasts.textContent = isEn ? "Female Breasts" : "Женская грудь";
  const chipButt = document.querySelector("#chip-buttocks .chip-label"); if (chipButt) chipButt.textContent = isEn ? "Buttocks / Anus" : "Ягодицы / анус";
  const chipUnder = document.querySelector("#chip-underwear .chip-label"); if (chipUnder) chipUnder.textContent = isEn ? "Underwear / Bikini" : "Белье / бикини";
  const chipTorso = document.querySelector("#chip-body-exposed .chip-label"); if (chipTorso) chipTorso.textContent = isEn ? "Male Torso" : "Мужской торс";

  // 7. Audit panel stats and table
  const statLabels = document.querySelectorAll(".audit-stat-card .audit-stat-label");
  if (statLabels.length >= 6) {
    statLabels[0].textContent = isEn ? "Total" : "Всего";
    statLabels[1].textContent = isEn ? "NSFW" : "NSFW";
    statLabels[2].textContent = isEn ? "Stopwords" : "Стоп-слова";
    statLabels[3].textContent = isEn ? "Cues" : "Тайминги";
    statLabels[4].textContent = isEn ? "Latency" : "Задержка";
    statLabels[5].textContent = isEn ? "Protection" : "Защита";
  }

  const tabAll = document.getElementById("tab-filter-all"); if (tabAll) tabAll.innerHTML = `${isEn ? "All" : "Все"} (<span id="count-all">${countAll ? countAll.textContent : "0"}</span>)`;
  const tabNsfw = document.getElementById("tab-filter-nsfw"); if (tabNsfw) tabNsfw.innerHTML = `${isEn ? "Nudity" : "Нагота"} (<span id="count-nsfw">${countNsfw ? countNsfw.textContent : "0"}</span>)`;
  const tabOcr = document.getElementById("tab-filter-ocr"); if (tabOcr) tabOcr.innerHTML = `${isEn ? "Stopwords" : "Стоп-слова"} (<span id="count-ocr">${countOcr ? countOcr.textContent : "0"}</span>)`;
  const tabCue = document.getElementById("tab-filter-cue"); if (tabCue) tabCue.innerHTML = `${isEn ? "Timings" : "Тайминги"} (<span id="count-cue">${countCue ? countCue.textContent : "0"}</span>)`;
  const tabAct = document.getElementById("tab-filter-active"); if (tabAct) tabAct.innerHTML = `${isEn ? "Active" : "Активные"} (<span id="count-active">${countActive ? countActive.textContent : "0"}</span>)`;

  const ths = document.querySelectorAll(".incidents-table thead th");
  if (ths.length >= 6) {
    ths[0].textContent = isEn ? "Time" : "Время";
    ths[1].textContent = isEn ? "Monitor" : "Монитор";
    ths[2].textContent = isEn ? "Type" : "Тип";
    ths[3].textContent = isEn ? "Violation" : "Нарушение";
    ths[4].textContent = isEn ? "OBS Action" : "Действие в OBS";
    ths[5].textContent = isEn ? "Status" : "Статус";
  }

  const emptyRow = document.querySelector("#incidents-empty-row td");
  if (emptyRow && totalIncidents === 0) {
    emptyRow.textContent = isEn ? "No censorship incidents recorded. Stream is clean." : "Инцидентов цензуры не зафиксировано. Поток безопасен.";
  }

  // 8. Modals:
  // Modal 1: Stopwords
  const stopAlert = document.getElementById("stopwords-info-alert");
  if (stopAlert) stopAlert.textContent = isEn
    ? "One rule per line. Exact words, wildcards word*, regex /pattern/, and Cyrillic/Latin homoglyphs are supported. Safe masked examples: [twitch_hate_speech_en] (n????r, retard, c?nt, b??ch, person.pdf), [twitch_hate_speech_ru] (3.14д0р, носильщик, роскомнадзор)."
    : "Построчный ввод. Поддерживаются точные совпадения, маски слово*, регулярные выражения /шаблон/ и омоглифы кириллицы/латиницы. Примеры категорий: [twitch_hate_speech_en] (n????r, retard, c?nt, b??ch, person.pdf), [twitch_hate_speech_ru] (3.14д0р, носильщик, роскомнадзор).";
  const btnCloseStop = document.getElementById("btn-close-stopwords"); if (btnCloseStop) btnCloseStop.textContent = isEn ? "Close" : "Закрыть";
  const btnSaveStop = document.getElementById("btn-save-rules"); if (btnSaveStop) btnSaveStop.textContent = isEn ? "Apply Changes" : "Применить изменения";

  // Modal 2: Hotkeys
  const btnCloseHotkeys = document.getElementById("btn-close-hotkeys"); if (btnCloseHotkeys) btnCloseHotkeys.textContent = isEn ? "Close" : "Закрыть";

  // Modal 3: About
  const btnCloseAbout = document.getElementById("btn-close-about"); if (btnCloseAbout) btnCloseAbout.textContent = isEn ? "Close" : "Закрыть";
  const btnAboutDonate = document.getElementById("btn-about-donate");
  if (btnAboutDonate) {
    btnAboutDonate.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"/></svg> ${isEn ? "Support the Developer" : "Поддержать разработчика"}`;
  }

  // Modal 4: Cues Manager
  const btnCloseCues = document.getElementById("btn-close-cues"); if (btnCloseCues) btnCloseCues.textContent = isEn ? "Close" : "Закрыть";
  const btnClearCues = document.getElementById("btn-clear-all-cues");
  if (btnClearCues) {
    btnClearCues.innerHTML = `<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg> ${isEn ? "Clear all" : "Очистить все"}`;
  }
  const cuesThs = document.querySelectorAll("#modal-cues-manager table thead th");
  if (cuesThs.length >= 5) {
    cuesThs[0].textContent = isEn ? "Status" : "Статус";
    cuesThs[1].textContent = isEn ? "Interval" : "Интервал";
    cuesThs[2].textContent = isEn ? "Duration" : "Длительность";
    cuesThs[3].textContent = isEn ? "Description / Reason" : "Описание / Причина";
    cuesThs[4].textContent = isEn ? "Action" : "Действие";
  }

  // Modal 5: Models Download
  const btnCloseModels = document.getElementById("btn-close-models"); if (btnCloseModels) btnCloseModels.textContent = isEn ? "Close" : "Закрыть";
  const btnStartDownload = document.getElementById("btn-start-models-download");
  if (btnStartDownload) {
    btnStartDownload.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -2px; margin-right: 4px;"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg> ${isEn ? "Download Models" : "Загрузить модели"}`;
  }

  // Modal 6: Browser Extension
  const btnCloseExt = document.getElementById("btn-close-ext"); if (btnCloseExt) btnCloseExt.textContent = isEn ? "Close" : "Закрыть";
  const btnLaunchBrowser = document.getElementById("btn-launch-browser-ext");
  if (btnLaunchBrowser) {
    btnLaunchBrowser.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg> ${isEn ? "Launch Browser with Extension" : "Запустить браузер с расширением"}`;
  }
  const btnOpenExtDir = document.getElementById("btn-open-ext-dir");
  if (btnOpenExtDir) {
    btnOpenExtDir.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg> ${isEn ? "Open Extension Folder" : "Открыть папку расширения"}`;
  }

  // Modal 7: OBS Setup Prompt
  const btnCancelObsPrompt = document.getElementById("btn-cancel-obs-setup-prompt"); if (btnCancelObsPrompt) btnCancelObsPrompt.textContent = isEn ? "Cancel" : "Отмена";
  const btnConfirmObsContinue = document.getElementById("btn-confirm-obs-setup-continue");
  if (btnConfirmObsContinue) {
    btnConfirmObsContinue.innerHTML = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg> <span>${isEn ? "Continue Auto-Setup" : "Продолжить автонастройку"}</span>`;
  }
  const btnLaunchObs = document.getElementById("btn-modal-launch-obs");
  if (btnLaunchObs) {
    btnLaunchObs.innerHTML = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg> <span>${isEn ? "Launch OBS Studio" : "Запустить OBS Studio"}</span>`;
  }

  // Modal 8: Disable Support Confirm
  const btnCancelDisable = document.getElementById("btn-cancel-disable-support"); if (btnCancelDisable) btnCancelDisable.textContent = isEn ? "No" : "Нет";
  const btnConfirmDisable = document.getElementById("btn-confirm-disable-support"); if (btnConfirmDisable) btnConfirmDisable.textContent = isEn ? "Yes" : "Да";

  // 9. Synchronize other stateful components
  if (typeof renderModelProfilesDropdown === "function") {
    renderModelProfilesDropdown();
  }
  if (typeof updateSetupGuideStatus === "function") {
    updateSetupGuideStatus(lastSetupGuideData || {});
  }
  if (lastModelsStatus) {
    updateModelsStatusUI(lastModelsStatus);
  }
  if (typeof formatSensitivityUI === "function" && nsfwThresholdSlider) {
    formatSensitivityUI(nsfwThresholdSlider.value);
  }
  if (typeof updateVisionUI === "function") {
    updateVisionUI(isBoosted);
  }
  if (typeof updateOcrStateUI === "function") {
    updateOcrStateUI(isOcrEnabled);
  }
  if (labelCueAutocensorStatus && toggleCueAutocensor) {
    labelCueAutocensorStatus.textContent = toggleCueAutocensor.checked
      ? (isEn ? "Enabled (OBS + Audio)" : "Включена (OBS + звук)")
      : (isEn ? "Disabled (HUD Only)" : "Отключена (Только HUD)");
  }
  if (labelCueNotificationsStatus && toggleCueNotifications) {
    labelCueNotificationsStatus.textContent = toggleCueNotifications.checked
      ? (isEn ? "Enabled (HUD)" : "Включены (HUD)")
      : (isEn ? "Disabled" : "Отключены");
    labelCueNotificationsStatus.style.color = toggleCueNotifications.checked ? "#38bdf8" : "#94a3b8";
  }
  if (activeRulesCounter) {
    const rCount = parseInt(activeRulesCounter.innerText, 10) || 0;
    activeRulesCounter.innerText = isEn ? `${rCount} rules` : `${rCount} правил`;
  }
  if (obsSetupPromptStatusBadge) {
    if (!isObsConnected) {
      obsSetupPromptStatusBadge.textContent = isEn ? "OBS Not Running" : "OBS не открыта";
    }
  }

  // Re-localize any active table rows
  if (incidentsTbody) {
    const rows = incidentsTbody.querySelectorAll("tr");
    rows.forEach(tr => {
      const type = tr.dataset.type;
      const isActive = tr.dataset.active === "true";
      const pill = tr.querySelector(".pill-tag");
      if (pill) {
        if (type === "cue") {
          pill.textContent = isEn ? "TIMING (PLAYER)" : "ТАЙМИНГ (ПЛЕЕР)";
        } else if (type === "nsfw") {
          pill.textContent = isEn ? "NUDITY (NSFW)" : "НАГОТА (NSFW)";
        } else if (type === "ocr") {
          pill.textContent = isEn ? "STOPWORD (OCR)" : "СТОП-СЛОВО (OCR)";
        }
      }
      const statusTag = tr.querySelector(".incident-status-tag");
      if (statusTag) {
        if (isActive) {
          statusTag.textContent = isEn ? "ACTIVE" : "АКТИВЕН";
        } else if (statusTag.textContent.includes("ТАЙМИНГ") || statusTag.textContent.includes("TIMING")) {
          statusTag.textContent = isEn ? "RESOLVED: TIMING EXPIRED" : "СНЯТ: ТАЙМИНГ ИСТЕК";
        } else if (statusTag.textContent.includes("ПОТОК") || statusTag.textContent.includes("STREAM")) {
          statusTag.textContent = isEn ? "RESOLVED: STREAM SAFE" : "СНЯТ: ПОТОК БЕЗОПАСЕН";
        } else {
          statusTag.textContent = isEn ? "RESOLVED: SCREEN CLEAN" : "СНЯТ: ЭКРАН ЧИСТ";
        }
      }
    });
  }

  // Update modal player sync status
  if (lastPlayerSyncData) {
    updatePlayerSyncUI(lastPlayerSyncData);
  } else {
    updatePlayerSyncUI({ name: null, time: 0, is_playing: false });
  }
}

window.setLanguage = setLanguage;

// Bind language switcher buttons in toolbar
const btnLangRu = document.getElementById("btn-lang-ru");
const btnLangEn = document.getElementById("btn-lang-en");

if (btnLangRu) btnLangRu.addEventListener("click", () => setLanguage("ru"));
if (btnLangEn) btnLangEn.addEventListener("click", () => setLanguage("en"));

if (window.I18N) {
  window.I18N.onLanguageChange((lang) => {
    if (currentLanguage !== lang) setLanguage(lang);
  });
}

// Restore saved settings immediately
restoreLocalPreferences();
loadInitialTauriData();
setLanguage(currentLanguage);

// Start connection on load
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    restoreLocalPreferences();
    loadInitialTauriData();
    setLanguage(currentLanguage);
    connectWebSocket();
  });
} else {
  connectWebSocket();
}
