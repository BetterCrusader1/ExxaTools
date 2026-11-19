window.__vdGlyphs = __GLYPHS__;

// The icon belongs to the window that owns the Help menu.
window.__vdInject = () => {
  if (window.__vdNoIcon) return "panel-only";
  const ROOT = "__ROOT__";
  const ICON = "__ICON__";

  const existing = document.getElementById(ROOT);
  if (existing && existing.dataset.build === (window.__vdBuild || "")) return "present";
  if (existing) existing.remove();

  let help = null;
  for (const el of document.querySelectorAll("div,button,span,a,li")) {
    if (el.childElementCount) continue;
    if (el.textContent.trim() !== "Help") continue;
    const rect = el.getBoundingClientRect();
    if (rect.width > 0 && rect.top < 60) { help = el; break; }
  }
  if (!help) return "no-menubar";
  const wrapper = help.parentElement;
  if (!wrapper) return "no-anchor";

  const item = wrapper.cloneNode(true);
  item.id = ROOT;
  item.dataset.build = window.__vdBuild || "";
  item.innerHTML = "";
  item.style.cursor = "pointer";
  item.style.display = "flex";
  item.style.alignItems = "center";
  item.style.justifyContent = "center";
  item.title = "ExxaTools (Ctrl+Shift+E)";
  const icon = document.createElement("span");
  icon.id = ICON;
  icon.style.display = "flex";
  icon.style.width = "16px";
  icon.style.height = "16px";
  icon.innerHTML = ICON_ICON;
  item.appendChild(icon);
  wrapper.insertAdjacentElement("afterend", item);

  // The icon next to Help opens the full ExxaTools app.
  const open_app = () => {
    window.__vdAction = "dashboard";
  };

  item.addEventListener("click", (event) => {
    event.stopPropagation();
    open_app();
  });
  document.addEventListener("keydown", (event) => {
    if (event.ctrlKey && event.shiftKey && (event.key === "E" || event.key === "e")) {
      event.preventDefault();
      open_app();
    }
  });

  return "injected";
};

if (!window.__vdWatcher) {
  let pending = null;
  window.__vdWatcher = new MutationObserver(() => {
    if (pending) return;
    pending = setTimeout(() => {
      pending = null;
      if (!window.__vdNoIcon && !document.getElementById("__ROOT__") && document.body) {
        try { window.__vdInject(); } catch (error) { /* Steam is mid-render */ }
      }
    }, 250);
  });
  window.__vdWatcher.observe(document.body, { childList: true, subtree: true });
}


// Inline actions beside the store page title. No floating card: the row uses
// Steam's Motiva Sans and ExxaTools' violet accent, and shows only the actions
// that matter for this app (Add, then Restart Steam, then Sync/Fix).
window.__vdAppPage = () => {
  const match = (window.location.pathname || "").match(/^\/app\/(\d+)/);
  if (!match) return false;
  if (!/(^|\.)store\.steampowered\.com$/.test(window.location.hostname || "")) return false;
  const appid = Number(match[1]);
  const title = document.getElementById("appHubAppName") || document.querySelector(".apphub_AppName");
  if (!title) return false;

  // Drop the previous floating card if an older build left one behind.
  const legacy = document.getElementById("vd-app-card");
  if (legacy) legacy.remove();
  const legacyStyle = document.getElementById("vd-app-style");
  if (legacyStyle) legacyStyle.remove();
  if (window.__vdAppPoll) { clearInterval(window.__vdAppPoll); window.__vdAppPoll = null; }

  const escapeHtml = (value) => String(value == null ? "" : value)
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;").replace(/'/g, "&#39;");
  const plusIcon = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M8 3v10M3 8h10"/></svg>';
  const glyph = (name) => (window.__vdGlyphs || {})[name] || "";
  const button = (want, label, cls, icon, disabled) =>
    '<button type="button" class="exxa-store-btn ' + cls + '" data-want="' + want + '"'
    + (disabled ? " disabled" : "") + ">" + (icon || "") + "<span>" + escapeHtml(label) + "</span></button>";
  const sep = '<span class="exxa-store-sep"></span>';

  const render = (wrap, state) => {
    const busy = window.__vdPageBusy;
    let html = "";
    if (busy) {
      const labels = { add: "Adding…", restart: "Restarting…", sync: "Syncing…", fix: "Fixing…", settings: "Opening…" };
      html = button(busy, labels[busy] || "Working…", "primary", "", true);
    } else if (state && Number(state.appid) === appid) {
      if (state.can_restart) {
        html = button("restart", "Restart Steam", "primary", glyph("restart"));
      } else {
        if (state.can_add) {
          html = button("add", "Add", "primary", plusIcon);
        } else if (state.can_sync) {
          html = button("sync", "Sync", "secondary", glyph("sync"));
          if (state.can_fix) html += sep + button("fix", "Fix update", "secondary", glyph("fix"));
        }
        html += (html ? sep : "") + button("restart", "Restart Steam", "secondary", glyph("restart"));
      }
      if (state.can_setup) html += (html ? sep : "") + button("settings", "Setup source", "secondary", glyph("provider"));
      if (state.error && state.status) html += '<span class="exxa-store-error">' + escapeHtml(state.status) + "</span>";
    } else {
      html = button("add", "Add", "primary", plusIcon, true);
    }
    wrap.innerHTML = html;
  };

  let style = document.getElementById("vd-store-style");
  if (!style) {
    style = document.createElement("style");
    style.id = "vd-store-style";
    style.textContent = `
      .exxa-store-actions { display: inline-flex; align-items: center; gap: 8px;
        margin-left: 14px; vertical-align: middle; white-space: nowrap;
        font-family: "Motiva Sans", sans-serif; font-size: 13px; line-height: 1; }
      .exxa-store-sep { width: 1px; height: 18px; background: rgba(255,255,255,.18); flex: none; }
      .exxa-store-btn { display: inline-flex; align-items: center; gap: 6px; height: 28px;
        padding: 0 12px; border-radius: 3px; border: 1px solid rgba(167,139,250,.55);
        background: linear-gradient(180deg, rgba(139,92,246,.22), rgba(109,40,217,.12));
        color: #ede9fe; font: 600 13px/1 "Motiva Sans", sans-serif; letter-spacing: .01em;
        cursor: pointer; transition: background .15s, border-color .15s, transform .1s; }
      .exxa-store-btn svg { width: 14px; height: 14px; flex: none; }
      .exxa-store-btn:hover { border-color: #a78bfa;
        background: linear-gradient(180deg, rgba(139,92,246,.34), rgba(109,40,217,.2)); }
      .exxa-store-btn:active { transform: translateY(1px); }
      .exxa-store-btn.primary { background: linear-gradient(180deg, #8b5cf6, #6d28d9);
        border-color: #a78bfa; color: #fff;
        box-shadow: 0 1px 0 rgba(0,0,0,.35), inset 0 1px 0 rgba(255,255,255,.18); }
      .exxa-store-btn.primary:hover { background: linear-gradient(180deg, #9f75ff, #7c3aed); }
      .exxa-store-btn.secondary { background: rgba(255,255,255,.06);
        border-color: rgba(255,255,255,.2); color: #c6d4df; }
      .exxa-store-btn.secondary:hover { border-color: rgba(167,139,250,.6); color: #ede9fe;
        background: rgba(139,92,246,.12); }
      .exxa-store-btn:disabled { opacity: .55; cursor: default; transform: none; }
      .exxa-store-error { margin-left: 10px; color: #fca5a5;
        font: 400 12px/1.3 "Motiva Sans", sans-serif; white-space: normal;
        max-width: 340px; display: inline-block; vertical-align: middle; }
    `;
    document.head.appendChild(style);
  }

  let wrap = document.getElementById("vd-store-actions");
  if (wrap && (wrap.dataset.appid !== String(appid) || !title.contains(wrap))) {
    wrap.remove();
    wrap = null;
  }
  if (!wrap) {
    wrap = document.createElement("span");
    wrap.id = "vd-store-actions";
    wrap.className = "exxa-store-actions";
    wrap.dataset.appid = String(appid);
    wrap.setAttribute("role", "group");
    wrap.setAttribute("aria-label", "ExxaTools actions");
    title.appendChild(wrap);
    wrap.addEventListener("click", (event) => {
      const btn = event.target.closest("button[data-want]");
      if (!btn || btn.disabled) return;
      window.__vdPageBusy = btn.dataset.want;
      window.__vdPageRequest = { appid: appid, want: btn.dataset.want };
      render(wrap, (window.__vdPageCache || {})[appid]);
    });
    const cache = window.__vdPageCache || (window.__vdPageCache = {});
    if (!cache[appid]) window.__vdPageRequest = { appid: appid, want: "probe" };
  }
  wrap.dataset.build = window.__vdBuild || "";
  const state = (window.__vdPageCache || {})[appid];
  const key = (window.__vdBuild || "") + "|" + JSON.stringify(state || null) + "|" + (window.__vdPageBusy || "");
  if (wrap.dataset.key !== key) {
    wrap.dataset.key = key;
    render(wrap, state);
  }
  return true;
};

// Rust pushes state by assigning window.__vdPageState; cache it and re-render.
Object.defineProperty(window, "__vdPageState", {
  configurable: true,
  get() { return window.__vdPageStateValue || null; },
  set(state) {
    window.__vdPageStateValue = state;
    window.__vdPageBusy = null;
    if (state && state.appid) {
      const cache = window.__vdPageCache || (window.__vdPageCache = {});
      cache[Number(state.appid)] = state;
    }
    try { window.__vdAppPage(); } catch (error) { /* Steam is mid-render */ }
  },
});

if (!window.__vdAppWatcher) {
  window.__vdAppWatcher = setInterval(() => { try { window.__vdAppPage(); } catch (error) { } }, 1200);
}
try { window.__vdAppPage(); } catch (error) { /* store page not ready yet */ }

window.__vdInject();
