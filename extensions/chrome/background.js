/**
 * blewred Extension Background Service Worker
 * Intercepts media requests via webRequest, injects CORS headers, and relays
 * discovered stream URLs to content scripts for predictive lookahead streaming.
 */

// 1. Setup declarative CORS headers rule for untainted canvas access
chrome.runtime.onInstalled.addListener(() => {
  console.log("[blewred] Video Player Sync & Lookahead extension installed.");
  setupCorsBypassRule();
});

chrome.runtime.onStartup.addListener(() => {
  setupCorsBypassRule();
});

function setupCorsBypassRule() {
  if (!chrome.declarativeNetRequest) return;
  chrome.declarativeNetRequest.updateDynamicRules({
    removeRuleIds: [1001],
    addRules: [{
      id: 1001,
      priority: 1,
      action: {
        type: "modifyHeaders",
        responseHeaders: [
          { header: "Access-Control-Allow-Origin", operation: "set", value: "*" },
          { header: "Access-Control-Allow-Methods", operation: "set", value: "GET, HEAD, OPTIONS" },
          { header: "Access-Control-Allow-Headers", operation: "set", value: "*" }
        ]
      },
      condition: {
        resourceTypes: ["media", "xmlhttprequest", "other"],
        urlFilter: "*"
      }
    }]
  }).catch(() => {});
}

// 2. Intercept video streams across all tabs via webRequest
if (chrome.webRequest && chrome.webRequest.onBeforeRequest) {
  chrome.webRequest.onBeforeRequest.addListener(
    (details) => {
      const url = details.url;
      if (!url || typeof url !== "string") return;
      if (url.startsWith("blob:") || url.startsWith("data:")) return;

      const lower = url.toLowerCase();
      const isMedia = lower.includes(".m3u8") ||
                      lower.includes("/hls/") ||
                      lower.includes(".mpd") ||
                      (lower.includes(".mp4") && !lower.includes("blob:")) ||
                      (lower.includes(".m4s") && lower.includes("video"));

      if (isMedia && details.tabId > 0) {
        chrome.tabs.sendMessage(details.tabId, {
          type: "stream_url_discovered",
          url: url,
          frameId: details.frameId
        }).catch(() => {});
      }
    },
    { urls: ["<all_urls>"] }
  );
}
