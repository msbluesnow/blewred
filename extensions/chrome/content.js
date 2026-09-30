/**
 * blewred Ahead-of-Time Subtitle & Predictive Video Lookahead Interceptor
 * Intercepts Closed Captions & probes future video frames 10-15s ahead of playback.
 */

(() => {
  const WS_URL = "ws://127.0.0.1:51789";
  let socket = null;
  let isConnected = false;
  const processedCueKeys = new Set();
  const LOOKAHEAD_LEAD_TIME = 10.0; // Seconds ahead to probe in player

  // 1. Intercept stream URLs (HLS .m3u8, MP4, manifests)
  let lastCapturedStreamUrl = "";
  try {
    const origFetch = window.fetch;
    window.fetch = function(...args) {
      if (args && args[0] && typeof args[0] === "string") {
        const url = args[0];
        if (url.includes(".m3u8") || url.includes(".mp4") || url.includes("/hls/") || url.includes("manifest")) {
          lastCapturedStreamUrl = url;
        }
      }
      return origFetch.apply(this, args);
    };

    const origXhrOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function(method, url, ...rest) {
      if (typeof url === "string") {
        if (url.includes(".m3u8") || url.includes(".mp4") || url.includes("/hls/") || url.includes("manifest")) {
          lastCapturedStreamUrl = url;
        }
      }
      return origXhrOpen.call(this, method, url, ...rest);
    };
  } catch (e) {}

  let currentMode = 0; // 0=Hybrid, 1=PlayerOnly, 2=ScreenOnly
  let isPreviewWindowOpen = false;
  const candidateStreamUrls = new Set();
  let onStreamCandidateDiscovered = null;

  function registerCandidateStreamUrl(url, source) {
    if (!url || typeof url !== "string") return;
    if (url.startsWith("blob:") || url.startsWith("data:")) return;
    if (candidateStreamUrls.has(url)) return;

    candidateStreamUrls.add(url);
    console.log("[blewred] Discovered stream URL from " + source + ":", url.substring(0, 100));
    if (typeof onStreamCandidateDiscovered === "function") {
      onStreamCandidateDiscovered(url);
    }
  }

  // Listen from background.js (webRequest)
  if (typeof chrome !== "undefined" && chrome.runtime && chrome.runtime.onMessage) {
    chrome.runtime.onMessage.addListener((msg) => {
      if (msg && msg.type === "stream_url_discovered" && msg.url) {
        registerCandidateStreamUrl(msg.url, "webRequest");
      }
    });
  }

  // Listen from page_interceptor.js (MAIN world)
  window.addEventListener("message", (event) => {
    if (event.data && event.data.type === "__blewred_stream_url__" && event.data.url) {
      registerCandidateStreamUrl(event.data.url, event.data.source || "main_world");
    }
  });

  function connectBridge() {
    if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
      return;
    }

    try {
      socket = new WebSocket(WS_URL);

      socket.onopen = () => {
        isConnected = true;
        socket.send(JSON.stringify({
          type: "register",
          client: "browser",
          url: window.location.href
        }));
        socket.send(JSON.stringify({
          type: "get_operation_mode"
        }));
        console.log("[blewred] Connected to local desktop core");

        if (!window._blewredPulseInterval) {
          window._blewredPulseInterval = setInterval(() => {
            if (isConnected && socket && socket.readyState === WebSocket.OPEN) {
              try {
                socket.send(JSON.stringify({
                  type: "heartbeat",
                  client: "browser",
                  url: window.location.href
                }));
              } catch (e) {}
            }
          }, 2500);
        }
      };

      socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          if (data.type === "operation_mode_changed" && data.operation_mode !== undefined) {
            currentMode = data.operation_mode;
            console.log("[blewred] Pipeline mode changed to:", currentMode);
          } else if (data.type === "init_state") {
            if (data.operation_mode !== undefined) {
              currentMode = data.operation_mode;
            }
            if (data.lookahead_preview_visible !== undefined) {
              isPreviewWindowOpen = data.lookahead_preview_visible;
            }
          } else if (data.type === "lookahead_preview_state" && data.is_open !== undefined) {
            isPreviewWindowOpen = data.is_open;
          }
        } catch (e) {}
      };

      socket.onclose = () => {
        isConnected = false;
        setTimeout(connectBridge, 3000);
      };

      socket.onerror = () => {
        isConnected = false;
      };
    } catch (e) {
      setTimeout(connectBridge, 3000);
    }
  }

  connectBridge();

  function detectPlatform() {
    const host = window.location.hostname;
    if (host.includes("kinobox")) return "kinobox";
    if (host.includes("alloha") || host.includes("stravers.live")) return "alloha";
    if (host.includes("collaps")) return "collaps";
    if (host.includes("rutube.ru")) return "rutube";
    if (host.includes("vk.com") || host.includes("vkvideo.ru")) return "vk";
    if (host.includes("kodik")) return "kodik";
    if (host.includes("videocdn")) return "videocdn";
    if (host.includes("hdrezka") || host.includes("rezka")) return "hdrezka";
    if (host.includes("youtube.com")) return "youtube";
    if (host.includes("twitch.tv")) return "twitch";
    return "web_player";
  }

  function detectPlayerName() {
    const select = document.querySelector(".player_select");
    if (select && select.selectedOptions && select.selectedOptions.length > 0) {
      return select.selectedOptions[0].textContent.trim();
    }
    
    const host = window.location.hostname;
    if (host.includes("stravers.live") || host.includes("alloha")) return "Alloha Player";
    if (host.includes("collaps")) return "Collaps Player";
    if (host.includes("turbovid") || host.includes("turbo")) return "Turbo Player";
    if (host.includes("veoveo")) return "Veoveo Player";
    if (host.includes("videoseed")) return "Videoseed Player";
    if (host.includes("kinobox")) return "Kinobox Player";
    if (host.includes("kodik")) return "Kodik Player";
    if (host.includes("videocdn")) return "VideoCDN Player";
    if (host.includes("hdrezka") || host.includes("rezka")) return "HDRezka Player";
    if (host.includes("rutube.ru")) return "Rutube";
    if (host.includes("vk.com") || host.includes("vkvideo.ru")) return "VK Video";
    if (host.includes("youtube.com")) return "YouTube";
    if (host.includes("twitch.tv")) return "Twitch";
    
    return "Web Player (" + host + ")";
  }

  function sendCues(video, cues) {
    if (!isConnected || !socket || currentMode === 2 || cues.length === 0) return;
    const currentTime = video ? video.currentTime : 0.0;

    const newCues = [];
    for (const cue of cues) {
      const key = `${cue.text}_${cue.start.toFixed(2)}`;
      if (!processedCueKeys.has(key)) {
        processedCueKeys.add(key);
        newCues.push(cue);
      }
    }

    if (newCues.length > 0) {
      if (processedCueKeys.size > 2000) {
        processedCueKeys.clear();
      }

      socket.send(JSON.stringify({
        type: "captions",
        platform: detectPlatform(),
        current_time: currentTime,
        cues: newCues
      }));
    }
  }

  // --- 2. Hook HTML5 TextTrack API ---
  function hookVideoTracks(video) {
    if (!video || video.__blewred_hooked) return;
    video.__blewred_hooked = true;

    function inspectTracks() {
      if (!video.textTracks) return;
      for (let i = 0; i < video.textTracks.length; i++) {
        const track = video.textTracks[i];
        if (track.__blewred_hooked) continue;
        track.__blewred_hooked = true;

        const handleCues = () => {
          if (!track.cues) return;
          const cuesList = [];
          const now = video.currentTime;
          for (let j = 0; j < track.cues.length; j++) {
            const cue = track.cues[j];
            if (cue.startTime >= now - 0.5 && cue.startTime <= now + 15.0) {
              cuesList.push({
                text: cue.text || "",
                start: cue.startTime,
                end: cue.endTime
              });
            }
          }
          if (cuesList.length > 0) {
            sendCues(video, cuesList);
          }
        };

        track.addEventListener("cuechange", handleCues);
        if (track.cues && track.cues.length > 0) {
          handleCues();
        }
      }
    }

    if (video.textTracks) {
      video.textTracks.addEventListener("addtrack", inspectTracks);
      inspectTracks();
    }
  }

  // --- 3. Fallback: DOM MutationObserver for Player Subtitle Containers ---
  function hookDomCaptions() {
    const observer = new MutationObserver((mutations) => {
      const video = document.querySelector("video");
      const currentTime = video ? video.currentTime : 0;

      for (const m of mutations) {
        for (const node of m.addedNodes) {
          if (node.nodeType === Node.ELEMENT_NODE) {
            if (node.classList && (node.classList.contains("ytp-caption-segment") || node.closest(".ytp-caption-window-container"))) {
              const text = node.textContent.trim();
              if (text) {
                sendCues(video, [{
                  text: text,
                  start: currentTime,
                  end: currentTime + 2.5
                }]);
              }
            } else if (node.classList && (node.classList.contains("timed-text-line") || node.classList.contains("player-timedtext-text-container"))) {
              const text = node.textContent.trim();
              if (text) {
                sendCues(video, [{
                  text: text,
                  start: currentTime,
                  end: currentTime + 2.5
                }]);
              }
            }
          }
        }
      }
    });

    observer.observe(document.documentElement, {
      childList: true,
      subtree: true
    });
  }

  // --- 4. Predictive Video Lookahead Sampling (Offscreen Clone & Stream Probe) ---
  function hookVideoLookahead(video) {
    if (!video || video.__blewred_lookahead_hooked) return;
    video.__blewred_lookahead_hooked = true;

    const canvas = document.createElement("canvas");
    canvas.width = 480;
    canvas.height = 270;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });

    let isSampling = false;
    let shadowVideo = null;
    let shadowHls = null;
    let shadowReady = false;
    let activeStreamUrl = "";
    let sampleTimer = null;
    let lastStableLookaheadTime = 0;

    function inspectVideoSources() {
      if (video.currentSrc && !video.currentSrc.startsWith("blob:")) {
        registerCandidateStreamUrl(video.currentSrc, "video_currentSrc");
      }
      if (video.src && !video.src.startsWith("blob:")) {
        registerCandidateStreamUrl(video.src, "video_src");
      }
      const sources = video.querySelectorAll("source");
      for (let i = 0; i < sources.length; i++) {
        const s = sources[i];
        if (s.src && !s.src.startsWith("blob:")) {
          registerCandidateStreamUrl(s.src, "source_tag");
        }
      }
      ["data-src", "data-hls", "data-stream", "data-file", "data-url"].forEach((attr) => {
        const val = video.getAttribute(attr);
        if (val && !val.startsWith("blob:") && !val.startsWith("data:")) {
          registerCandidateStreamUrl(val, "attr_" + attr);
        }
      });
    }

    function initShadowWithUrl(url) {
      if (!url || url === activeStreamUrl) return;
      activeStreamUrl = url;

      try {
        if (!shadowVideo) {
          shadowVideo = document.createElement("video");
          shadowVideo.id = "__blewred_shadow_video__";
          shadowVideo.setAttribute("data-blewred-shadow", "true");
          shadowVideo.setAttribute("aria-hidden", "true");
          shadowVideo.tabIndex = -1;
          shadowVideo.__blewred_is_shadow = true;
          shadowVideo.__blewred_events_hooked = true;
          shadowVideo.__blewred_hooked = true;
          shadowVideo.__blewred_lookahead_hooked = true;
          shadowVideo.muted = true;
          shadowVideo.volume = 0;
          shadowVideo.playsInline = true;
          shadowVideo.preload = "auto";
          shadowVideo.crossOrigin = "anonymous";
          shadowVideo.style.cssText = "position:fixed !important;left:-9999px !important;top:-9999px !important;width:1px !important;height:1px !important;opacity:0 !important;pointer-events:none !important;visibility:hidden !important;z-index:-999999 !important;";
          (document.body || document.documentElement).appendChild(shadowVideo);
        }

        shadowReady = false;
        const lower = url.toLowerCase();
        const isHls = lower.includes(".m3u8") || lower.includes("/hls/");

        if (isHls && typeof window.Hls !== "undefined" && window.Hls.isSupported()) {
          if (shadowHls) {
            try { shadowHls.destroy(); } catch (e) {}
          }
          shadowHls = new window.Hls({
            maxBufferLength: 20,
            maxMaxBufferLength: 40,
            autoLevelCapping: 0, // Pick lowest resolution (e.g. 360p/480p) for ultra-lightweight bandwidth
            enableWorker: true
          });
          shadowHls.loadSource(url);
          shadowHls.attachMedia(shadowVideo);

          shadowHls.on(window.Hls.Events.MANIFEST_PARSED, () => {
            shadowReady = true;
            console.log("[blewred] Lookahead HLS stream attached & ready (+10s ahead)");
          });
          shadowHls.on(window.Hls.Events.ERROR, (evt, errData) => {
            if (errData && errData.fatal) {
              shadowReady = false;
            }
          });
        } else {
          shadowVideo.src = url;
          shadowVideo.oncanplay = () => {
            shadowReady = true;
            console.log("[blewred] Lookahead direct video stream attached & ready (+10s ahead)");
          };
          shadowVideo.onerror = () => {
            shadowReady = false;
          };
        }
      } catch (e) {
        shadowReady = false;
      }
    }

    onStreamCandidateDiscovered = (url) => {
      if (!shadowReady) {
        initShadowWithUrl(url);
      }
    };

    function ensureShadowVideo() {
      // 1. Check for MSE shadow player from page_interceptor (for protected players like obrut.show)
      const domMseShadow = document.getElementById("__blewred_shadow_video__");
      if (domMseShadow && domMseShadow.readyState >= 1) {
        shadowVideo = domMseShadow;
        shadowVideo.__blewred_is_shadow = true;
        shadowVideo.__blewred_events_hooked = true;
        shadowVideo.__blewred_hooked = true;
        shadowVideo.__blewred_lookahead_hooked = true;
        shadowReady = true;
        return;
      }

      if (shadowVideo && shadowReady) return;
      inspectVideoSources();

      if (!activeStreamUrl && candidateStreamUrls.size > 0) {
        // Pick best candidate: prefer 360p/480p MP4 or master .m3u8
        const list = Array.from(candidateStreamUrls);
        const best = list.find((u) => u.includes("360") || u.includes("480")) ||
                     list.find((u) => u.includes(".m3u8") || u.includes("/hls/")) ||
                     list[0];
        if (best) {
          initShadowWithUrl(best);
        }
      }
    }

    async function sampleLookaheadFrame() {
      if (!isConnected || !socket || isSampling || currentMode === 2) return;
      if (video.paused || video.ended || video.readyState < 2) {
        if (shadowVideo && !shadowVideo.paused) {
          try { shadowVideo.pause(); } catch (e) {}
        }
        return;
      }
      if (socket.bufferedAmount > 65536) return; // Prevent WebSocket buffer accumulation lag

      isSampling = true;
      try {
        ensureShadowVideo();

        // Also check if MSE shadow became ready
        if (!shadowReady) {
          const domMseShadow = document.getElementById("__blewred_shadow_video__");
          if (domMseShadow && domMseShadow.readyState >= 1) {
            shadowVideo = domMseShadow;
            shadowVideo.__blewred_is_shadow = true;
            shadowVideo.__blewred_events_hooked = true;
            shadowVideo.__blewred_hooked = true;
            shadowVideo.__blewred_lookahead_hooked = true;
            shadowReady = true;
          }
        }

        const curTime = video.currentTime || 0.0;
        let targetSource = video;
        let lookTime = curTime;
        let isAhead = false;

        // If shadow clone is active, keep steady 10.0s lead without runaway or flickering
        if (shadowVideo && shadowReady) {
          // Double check pause state: if user paused during sample scheduling, abort
          if (video.paused) {
            if (!shadowVideo.paused) {
              try { shadowVideo.pause(); } catch (e) {}
            }
            return;
          }

          let maxBuffered = 0;
          try {
            for (let i = 0; i < shadowVideo.buffered.length; i++) {
              if (shadowVideo.buffered.end(i) > maxBuffered) {
                maxBuffered = shadowVideo.buffered.end(i);
              }
            }
          } catch (e) {}

          const desiredLead = LOOKAHEAD_LEAD_TIME; // 10.0s
          const desiredTime = curTime + desiredLead;
          const targetTime = maxBuffered > curTime + 1.0 ? Math.min(desiredTime, maxBuffered - 0.25) : desiredTime;
          const currentLead = shadowVideo.currentTime - curTime;

          // Lead bound pacing (keeps lead tightly bounded between 8.5s and 11.5s)
          if (!video.paused) {
            if (currentLead > desiredLead + 1.5) {
              // Too far ahead (> 11.5s): pause briefly to let main video close the gap
              if (!shadowVideo.paused) {
                shadowVideo.pause();
              }
            } else if (currentLead < desiredLead - 1.5) {
              // Lagging behind (< 8.5s): play or seek to close gap
              if (shadowVideo.paused && !shadowVideo.seeking) {
                shadowVideo.play().catch(() => {});
              }
            } else {
              // Optimal horizon: play smoothly in parallel at 1.0x speed
              if (shadowVideo.paused && !shadowVideo.seeking) {
                shadowVideo.play().catch(() => {});
              }
            }
          }

          // Only initiate seek on severe drift (> 3.0s or behind main playhead) and NOT while already seeking
          const drift = Math.abs(shadowVideo.currentTime - targetTime);
          if ((drift > 3.0 || shadowVideo.currentTime <= curTime + 1.0) && !shadowVideo.seeking) {
            shadowVideo.currentTime = targetTime;
          }

          // Anti-flicker frame selection:
          // Never fall back to on-screen video (0s) during seek or momentary buffer stall!
          if (shadowVideo.seeking) {
            if (lastStableLookaheadTime >= curTime + 1.0) {
              targetSource = shadowVideo;
              lookTime = lastStableLookaheadTime;
              isAhead = true;
            }
          } else if (shadowVideo.readyState >= 2 && shadowVideo.currentTime >= curTime + 1.0) {
            targetSource = shadowVideo;
            lookTime = shadowVideo.currentTime;
            lastStableLookaheadTime = lookTime;
            isAhead = true;
          } else if (lastStableLookaheadTime >= curTime + 1.0) {
            // Buffer/decode hiccup: preserve previous lookahead frame instead of jumping back to 0s
            targetSource = shadowVideo;
            lookTime = lastStableLookaheadTime;
            isAhead = true;
          }
        }

        // Fast hardware-accelerated frame capture using createImageBitmap
        if (typeof createImageBitmap === "function") {
          try {
            const bmp = await createImageBitmap(targetSource, {
              resizeWidth: canvas.width,
              resizeHeight: canvas.height,
              resizeQuality: "low"
            });
            ctx.drawImage(bmp, 0, 0);
            bmp.close();
          } catch (e) {
            ctx.drawImage(targetSource, 0, 0, canvas.width, canvas.height);
          }
        } else {
          ctx.drawImage(targetSource, 0, 0, canvas.width, canvas.height);
        }

        const dataUrl = canvas.toDataURL("image/jpeg", 0.60);

        socket.send(JSON.stringify({
          type: "lookahead_frame",
          player_id: "p-" + Math.abs(window.location.href.split("").reduce((a, b) => { a = ((a << 5) - a) + b.charCodeAt(0); return a & a }, 0)),
          player_type: detectPlayerName(),
          current_time: curTime,
          lookahead_time: lookTime,
          is_ahead: isAhead,
          width: canvas.width,
          height: canvas.height,
          image: dataUrl
        }));
      } catch (err) {
        // Handled safely
      } finally {
        isSampling = false;
      }
    }

    video.addEventListener("timeupdate", () => {
      ensureShadowVideo();
    });

    video.addEventListener("play", () => {
      ensureShadowVideo();
      if (shadowVideo && shadowVideo.paused && !shadowVideo.seeking && shadowVideo.readyState >= 1) {
        shadowVideo.play().catch(() => {});
      }
      sampleLookaheadFrame();
    });

    video.addEventListener("pause", () => {
      if (shadowVideo && !shadowVideo.paused) {
        try { shadowVideo.pause(); } catch (e) {}
      }
    });

    video.addEventListener("seeking", () => {
      if (shadowVideo) {
        const curT = video.currentTime || 0.0;
        shadowVideo.currentTime = curT + LOOKAHEAD_LEAD_TIME;
        if (video.paused) {
          try { shadowVideo.pause(); } catch (e) {}
        }
      }
    });

    video.addEventListener("seeked", () => {
      ensureShadowVideo();
      if (!video.paused) {
        sampleLookaheadFrame();
      }
    });

    // Adaptive non-blocking sampling loop: 4 FPS (250ms) if preview open, 1.5 FPS (650ms) if closed
    function scheduleSampler() {
      clearTimeout(sampleTimer);
      const delay = isPreviewWindowOpen ? 250 : 650;
      sampleTimer = setTimeout(async () => {
        if (video && !video.paused && !video.ended && video.readyState >= 2) {
          await sampleLookaheadFrame();
        }
        scheduleSampler();
      }, delay);
    }
    scheduleSampler();
  }

  // --- 5. Recursive Shadow DOM Video Finder & Dedicated Player Time Sync ---
  function isShadowVideo(v) {
    if (!v) return true;
    if (v.__blewred_is_shadow) return true;
    if (v.id === "__blewred_shadow_video__") return true;
    if (v.hasAttribute && v.hasAttribute("data-blewred-shadow")) return true;
    if (v.style && (v.style.position === "fixed" || v.style.position === "absolute") && (v.style.width === "1px" || v.style.opacity === "0")) return true;
    try {
      const rect = v.getBoundingClientRect ? v.getBoundingClientRect() : null;
      if (rect && rect.width <= 10 && rect.height <= 10) return true;
    } catch (e) {}
    return false;
  }

  function findVideos(root = document) {
    const raw = [];
    if (!root) return raw;

    try {
      if (root.querySelectorAll) {
        raw.push(...Array.from(root.querySelectorAll("video")));
        const allNodes = root.querySelectorAll("*");
        for (let i = 0; i < allNodes.length; i++) {
          const node = allNodes[i];
          if (node.shadowRoot) {
            raw.push(...findVideos(node.shadowRoot));
          }
        }
      }
    } catch (e) {}

    return raw.filter((v) => !isShadowVideo(v));
  }

  let primaryUserVideo = null;

  function getMasterVideo() {
    if (primaryUserVideo && primaryUserVideo.isConnected && !isShadowVideo(primaryUserVideo)) {
      return primaryUserVideo;
    }
    const all = findVideos(document);
    if (all.length === 0) {
      primaryUserVideo = null;
      return null;
    }
    if (all.length === 1) {
      primaryUserVideo = all[0];
      return primaryUserVideo;
    }
    // Multiple visible videos: pick the visible on-screen video with the largest rendered area
    let best = all[0];
    let maxArea = -1;
    for (const v of all) {
      try {
        const rect = v.getBoundingClientRect ? v.getBoundingClientRect() : null;
        const w = rect ? rect.width : (v.videoWidth || 0);
        const h = rect ? rect.height : (v.videoHeight || 0);
        const area = w * h;
        if (area > maxArea) {
          maxArea = area;
          best = v;
        }
      } catch (e) {}
    }
    primaryUserVideo = best;
    return primaryUserVideo;
  }

  let lastReportedTime = -1;
  let lastReportedPlayState = false;
  let lastPulseTime = 0;

  function syncActivePlayerTime() {
    if (!isConnected || !socket || socket.readyState !== WebSocket.OPEN) return;

    const activeVideo = getMasterVideo();
    const now = Date.now();

    if (!activeVideo) {
      // No video in this frame: send periodic connection heartbeat (every 2.5s)
      if (now - lastPulseTime >= 2500) {
        lastPulseTime = now;
        try {
          socket.send(JSON.stringify({
            type: "heartbeat",
            client: "browser",
            url: window.location.href
          }));
        } catch (e) {}
      }
      return;
    }

    const curTime = activeVideo.currentTime || 0.0;
    const duration = activeVideo.duration || 0.0;
    const isPlaying = !activeVideo.paused && !activeVideo.ended;

    // Strict master-sync: if real video is paused, shadow video MUST pause immediately
    if (!isPlaying) {
      const domMseShadow = document.getElementById("__blewred_shadow_video__");
      if (domMseShadow && !domMseShadow.paused) {
        try { domMseShadow.pause(); } catch (e) {}
      }
    }

    const timeDelta = Math.abs(curTime - lastReportedTime);
    if (now - lastPulseTime < 1500 && timeDelta < 0.10 && isPlaying === lastReportedPlayState) {
      return;
    }
    lastReportedTime = curTime;
    lastReportedPlayState = isPlaying;
    lastPulseTime = now;

    const playerId = "p-" + Math.abs(window.location.href.split("").reduce((a, b) => { a = ((a << 5) - a) + b.charCodeAt(0); return a & a }, 0));

    try {
      socket.send(JSON.stringify({
        type: "player_sync",
        client: "browser",
        player_id: playerId,
        player_name: detectPlayerName(),
        current_time: curTime,
        duration: duration,
        is_playing: isPlaying,
        detected: true,
        url: window.location.href
      }));
    } catch (e) {}
  }

  // Allow extension popup to query player status directly from the active tab
  try {
    if (typeof chrome !== "undefined" && chrome.runtime && chrome.runtime.onMessage) {
      chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
        if (request && request.type === "get_player_status") {
          const activeVideo = getMasterVideo();
          if (!activeVideo) {
            sendResponse({
              detected: false,
              name: "Not Detected",
              time: 0,
              is_playing: false
            });
            return true;
          }

          sendResponse({
            detected: true,
            name: detectPlayerName(),
            time: activeVideo.currentTime || 0,
            is_playing: !activeVideo.paused && !activeVideo.ended
          });
          return true;
        }
      });
    }
  } catch (e) {}

  function hookVideoEvents(v) {
    if (!v || isShadowVideo(v) || v.__blewred_events_hooked) return;
    v.__blewred_events_hooked = true;
    hookVideoTracks(v);
    hookVideoLookahead(v);
    v.addEventListener("seeked", () => syncActivePlayerTime());
    v.addEventListener("seeking", () => syncActivePlayerTime());
    v.addEventListener("play", () => syncActivePlayerTime());
    v.addEventListener("pause", () => syncActivePlayerTime());
    v.addEventListener("timeupdate", () => syncActivePlayerTime());
  }

  // Scan for existing videos and future added videos
  function initVideoScanner() {
    const all = findVideos(document);
    all.forEach(hookVideoEvents);

    const videoObserver = new MutationObserver(() => {
      findVideos(document).forEach(hookVideoEvents);
    });

    videoObserver.observe(document.documentElement, {
      childList: true,
      subtree: true
    });

    // Start dedicated high-precision player time sync ticker (runs in all modes)
    setInterval(syncActivePlayerTime, 400);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => {
      initVideoScanner();
      hookDomCaptions();
    });
  } else {
    initVideoScanner();
    hookDomCaptions();
  }
})();

