/**
 * blewred Page Interceptor (Runs in page MAIN world)
 * 1. Clones decrypted MSE chunks directly from SourceBuffer.prototype.appendBuffer
 *    into an offscreen shadow video player (works for protected players like obrut.show, Alloha, etc.)
 * 2. Intercepts real-time video stream URLs (.m3u8, .mp4, manifests) from Hls.js, VK Video, and network calls.
 * 3. Shields website player scripts from ever detecting or interacting with the shadow video element.
 * 4. Synchronizes pause/seek states instantly via capturing event listeners.
 */

(() => {
  if (window.__blewred_page_interceptor_loaded) return;
  window.__blewred_page_interceptor_loaded = true;

  function broadcastStreamUrl(url, source) {
    if (!url || typeof url !== "string") return;
    if (url.startsWith("blob:") || url.startsWith("data:")) return;

    // Filter media candidates
    const lower = url.toLowerCase();
    const isMedia = lower.includes(".m3u8") ||
                    lower.includes("/hls/") ||
                    lower.includes(".mp4") ||
                    lower.includes(".mpd") ||
                    lower.includes(".m4s") ||
                    lower.includes("video") ||
                    lower.includes("manifest") ||
                    lower.includes("playlist");

    if (!isMedia) return;

    try {
      window.postMessage({
        type: "__blewred_stream_url__",
        url: url,
        source: source || "unknown"
      }, "*");
    } catch (e) {}
  }

  // --- 1. MSE SourceBuffer Cloner (For Protected / Tokenized / Obfuscated Players) ---
  let shadowMediaSource = null;
  let shadowVideo = null;
  let shadowSourceBuffer = null;
  const shadowQueue = [];
  let videoMimeType = "";

  function ensureShadowMsePlayer() {
    if (shadowVideo) return;
    try {
      shadowVideo = document.createElement("video");
      shadowVideo.id = "__blewred_shadow_video__";
      shadowVideo.setAttribute("data-blewred-shadow", "true");
      shadowVideo.setAttribute("aria-hidden", "true");
      shadowVideo.tabIndex = -1;
      shadowVideo.__blewred_is_shadow = true;
      shadowVideo.muted = true;
      shadowVideo.volume = 0;
      shadowVideo.playsInline = true;
      shadowVideo.preload = "auto";
      shadowVideo.crossOrigin = "anonymous";
      shadowVideo.style.cssText = "position:fixed !important;left:-9999px !important;top:-9999px !important;width:1px !important;height:1px !important;opacity:0 !important;pointer-events:none !important;visibility:hidden !important;z-index:-999999 !important;";
      (document.body || document.documentElement).appendChild(shadowVideo);

      shadowMediaSource = new MediaSource();
      shadowVideo.src = URL.createObjectURL(shadowMediaSource);

      shadowMediaSource.addEventListener("sourceopen", () => {
        if (videoMimeType && !shadowSourceBuffer && shadowMediaSource.readyState === "open") {
          try {
            shadowSourceBuffer = shadowMediaSource.addSourceBuffer(videoMimeType);
            setupShadowBufferEvents();
          } catch (e) {}
        }
      });
    } catch (e) {}
  }

  function setupShadowBufferEvents() {
    if (!shadowSourceBuffer) return;
    shadowSourceBuffer.addEventListener("updateend", () => {
      if (shadowQueue.length > 0 && !shadowSourceBuffer.updating) {
        try {
          const nextChunk = shadowQueue.shift();
          shadowSourceBuffer.appendBuffer(nextChunk);
        } catch (e) {}
      }
    });
    shadowSourceBuffer.addEventListener("error", () => {});
  }

  function feedShadowChunk(chunk) {
    ensureShadowMsePlayer();
    if (!shadowSourceBuffer) {
      if (shadowMediaSource && shadowMediaSource.readyState === "open" && videoMimeType) {
        try {
          shadowSourceBuffer = shadowMediaSource.addSourceBuffer(videoMimeType);
          setupShadowBufferEvents();
        } catch (e) {}
      }
    }

    if (shadowSourceBuffer) {
      if (shadowSourceBuffer.updating || shadowQueue.length > 0) {
        if (shadowQueue.length < 50) {
          shadowQueue.push(chunk);
        }
      } else {
        try {
          shadowSourceBuffer.appendBuffer(chunk);
        } catch (e) {
          if (shadowQueue.length < 50) {
            shadowQueue.push(chunk);
          }
        }
      }
    } else {
      if (shadowQueue.length < 50) {
        shadowQueue.push(chunk);
      }
    }
  }

  // Hook MediaSource.prototype.addSourceBuffer
  let initSegment = null;
  try {
    const origAddSourceBuffer = MediaSource.prototype.addSourceBuffer;
    MediaSource.prototype.addSourceBuffer = function(type) {
      const sb = origAddSourceBuffer.apply(this, arguments);

      // Do NOT hook our own shadow SourceBuffer
      if (this === shadowMediaSource) {
        sb.__blewred_is_shadow = true;
        // If we already have an initSegment, feed it immediately
        if (initSegment && !sb.updating) {
          try {
            sb.appendBuffer(initSegment.slice(0));
          } catch (e) {}
        }
        return sb;
      }

      const isVideoTrack = type && typeof type === "string" && type.toLowerCase().includes("video");

      if (isVideoTrack) {
        videoMimeType = type;
        ensureShadowMsePlayer();
      }

      // Hook appendBuffer on this specific SourceBuffer
      const origAppend = sb.appendBuffer;
      sb.appendBuffer = function(data) {
        if (this.__blewred_is_shadow) {
          return origAppend.apply(this, arguments);
        }

        if (isVideoTrack && data) {
          try {
            let copy;
            if (data instanceof ArrayBuffer) {
              copy = data.slice(0);
            } else if (ArrayBuffer.isView(data)) {
              copy = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength);
            }
            if (copy) {
              if (!initSegment) {
                initSegment = copy.slice(0);
              }
              feedShadowChunk(copy);
            }
          } catch (e) {}
        }
        return origAppend.apply(this, arguments);
      };

      return sb;
    };
  } catch (e) {}

  // --- 2. Hook window.fetch ---
  try {
    const origFetch = window.fetch;
    window.fetch = function(...args) {
      if (args && args[0]) {
        const url = typeof args[0] === "string" ? args[0] : (args[0].url || "");
        broadcastStreamUrl(url, "fetch");
      }
      return origFetch.apply(this, args);
    };
  } catch (e) {}

  // --- 3. Hook XMLHttpRequest ---
  try {
    const origXhrOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function(method, url, ...rest) {
      if (typeof url === "string") {
        broadcastStreamUrl(url, "xhr");
      }
      return origXhrOpen.call(this, method, url, ...rest);
    };
  } catch (e) {}

  // --- 4. Hook Hls.js if present ---
  try {
    function hookHlsClass(hlsClass) {
      if (!hlsClass || !hlsClass.prototype || hlsClass.prototype.__blewred_hooked) return;
      hlsClass.prototype.__blewred_hooked = true;
      const origLoadSource = hlsClass.prototype.loadSource;
      hlsClass.prototype.loadSource = function(url) {
        broadcastStreamUrl(url, "hls_load_source");
        return origLoadSource.apply(this, arguments);
      };
    }

    if (window.Hls) {
      hookHlsClass(window.Hls);
    } else {
      let _hls = window.Hls;
      Object.defineProperty(window, "Hls", {
        configurable: true,
        enumerable: true,
        get: () => _hls,
        set: (val) => {
          _hls = val;
          hookHlsClass(val);
        }
      });
    }
  } catch (e) {}

  // --- 5. Hook HTMLMediaElement (video.src, video.play) ---
  try {
    const origPlay = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function() {
      if (this.currentSrc && !this.currentSrc.startsWith("blob:")) {
        broadcastStreamUrl(this.currentSrc, "media_currentSrc");
      } else if (this.src && !this.src.startsWith("blob:")) {
        broadcastStreamUrl(this.src, "media_src");
      }
      return origPlay.apply(this, arguments);
    };
  } catch (e) {}

  // --- 6. Periodic scanner for VK Video / Playerjs global objects ---
  setInterval(() => {
    try {
      if (window.mvcur && window.mvcur.player) {
        const p = window.mvcur.player;
        if (p.options && p.options.params) {
          const params = p.options.params;
          if (params.hls) broadcastStreamUrl(params.hls, "vk_hls");
          if (params.mp4_720) broadcastStreamUrl(params.mp4_720, "vk_mp4_720");
          if (params.mp4_480) broadcastStreamUrl(params.mp4_480, "vk_mp4_480");
          if (params.mp4_360) broadcastStreamUrl(params.mp4_360, "vk_mp4_360");
        }
      }
      if (window.playerjs_data && typeof window.playerjs_data === "string") {
        broadcastStreamUrl(window.playerjs_data, "playerjs_data");
      }
    } catch (e) {}
  }, 2000);

  // --- 7. Instant Capture Event Listeners: Synchronize Pause/Seek/Play ---
  // Capturing phase ensures these run before any website stopPropagation()
  document.addEventListener("pause", (e) => {
    if (e.target && e.target.tagName === "VIDEO" && !e.target.__blewred_is_shadow && e.target.id !== "__blewred_shadow_video__") {
      if (shadowVideo && !shadowVideo.paused) {
        try { shadowVideo.pause(); } catch (err) {}
      }
    }
  }, true);

  document.addEventListener("seeking", (e) => {
    if (e.target && e.target.tagName === "VIDEO" && !e.target.__blewred_is_shadow && e.target.id !== "__blewred_shadow_video__") {
      if (shadowVideo && !shadowVideo.seeking) {
        try {
          shadowVideo.currentTime = (e.target.currentTime || 0) + 10.0;
          if (e.target.paused) {
            shadowVideo.pause();
          }
        } catch (err) {}
      }
    }
  }, true);

  document.addEventListener("play", (e) => {
    if (e.target && e.target.tagName === "VIDEO" && !e.target.__blewred_is_shadow && e.target.id !== "__blewred_shadow_video__") {
      if (shadowVideo && shadowVideo.paused && !shadowVideo.seeking && shadowVideo.readyState >= 1) {
        try { shadowVideo.play().catch(() => {}); } catch (err) {}
      }
    }
  }, true);

  // --- 8. Shield Webpage Player Scripts from seeing the Shadow Video Element ---
  function isShadowElement(el) {
    if (!el) return false;
    return !!(el.__blewred_is_shadow || el.id === "__blewred_shadow_video__" || (el.hasAttribute && el.hasAttribute("data-blewred-shadow")));
  }

  try {
    const origDocQS = Document.prototype.querySelector;
    Document.prototype.querySelector = function(sel) {
      if (typeof sel === "string" && sel.toLowerCase().includes("video")) {
        const res = origDocQS.apply(this, arguments);
        if (res && isShadowElement(res)) {
          const all = Document.prototype.querySelectorAll.call(this, "video");
          for (let i = 0; i < all.length; i++) {
            if (!isShadowElement(all[i])) {
              return all[i];
            }
          }
          return null;
        }
        return res;
      }
      return origDocQS.apply(this, arguments);
    };

    const origDocQSA = Document.prototype.querySelectorAll;
    Document.prototype.querySelectorAll = function(sel) {
      const res = origDocQSA.apply(this, arguments);
      if (typeof sel === "string" && sel.toLowerCase().includes("video")) {
        return Array.from(res).filter(el => !isShadowElement(el));
      }
      return res;
    };

    const origDocGetByTag = Document.prototype.getElementsByTagName;
    Document.prototype.getElementsByTagName = function(tag) {
      const res = origDocGetByTag.apply(this, arguments);
      if (typeof tag === "string" && tag.toLowerCase() === "video") {
        return Array.from(res).filter(el => !isShadowElement(el));
      }
      return res;
    };

    const origElemQS = Element.prototype.querySelector;
    Element.prototype.querySelector = function(sel) {
      if (typeof sel === "string" && sel.toLowerCase().includes("video")) {
        const res = origElemQS.apply(this, arguments);
        if (res && isShadowElement(res)) {
          const all = Element.prototype.querySelectorAll.call(this, "video");
          for (let i = 0; i < all.length; i++) {
            if (!isShadowElement(all[i])) {
              return all[i];
            }
          }
          return null;
        }
        return res;
      }
      return origElemQS.apply(this, arguments);
    };

    const origElemQSA = Element.prototype.querySelectorAll;
    Element.prototype.querySelectorAll = function(sel) {
      const res = origElemQSA.apply(this, arguments);
      if (typeof sel === "string" && sel.toLowerCase().includes("video")) {
        return Array.from(res).filter(el => !isShadowElement(el));
      }
      return res;
    };

    const origElemGetByTag = Element.prototype.getElementsByTagName;
    Element.prototype.getElementsByTagName = function(tag) {
      const res = origElemGetByTag.apply(this, arguments);
      if (typeof tag === "string" && tag.toLowerCase() === "video") {
        return Array.from(res).filter(el => !isShadowElement(el));
      }
      return res;
    };
  } catch (e) {}

})();
