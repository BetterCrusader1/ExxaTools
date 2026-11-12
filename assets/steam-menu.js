window.__vdGlyphs = __GLYPHS__;

// Steam runs several CEF windows: the shell that owns the menu bar, and separate
// windows for the store, the library and Community. The panel is built in
// whichever window the user is looking at, so it renders inside Steam instead of
// floating above the desktop.
window.__vdPanel = (opts) => {
  const API = "__API__";
  const PANEL = "__PANEL__";
  const glyph = (name) => (window.__vdGlyphs || {})[name] || "";

  const build = () => {
    if (!document.body) return null;
    const style = document.createElement("style");
    style.id = PANEL + "-style";
    style.textContent = `
      #${PANEL} {
        position: fixed; z-index: 2147483647; width: 300px; padding: 6px;
        background: #151A21; border: 1px solid #2E3A49; border-radius: 14px;
        box-shadow: 0 24px 60px rgba(0,0,0,.62);
        font-family: Montserrat, "Motiva Sans", Arial, sans-serif;
        color: #E9EDF2; display: none;
      }
      #${PANEL}.open { display: block; }
      #${PANEL} .vd-head { padding: 9px 11px 10px; }
      #${PANEL} .vd-title { display: flex; align-items: center; gap: 8px;
        font-size: 13px; font-weight: 700; letter-spacing: .01em; }
      #${PANEL} .vd-title span.name { flex: 1; }
      #${PANEL} .vd-ver { font: 400 10.5px ui-monospace, monospace; color: #8B96A5; }
      #${PANEL} .vd-x { width: 22px; height: 22px; flex: none; display: grid; place-items: center;
        border-radius: 6px; border: 1px solid transparent; background: transparent;
        color: #8B96A5; font: 600 13px/1 Montserrat, Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-x:hover { background: #1F2733; color: #EFF3F8; }
      #${PANEL} .vd-state { display: flex; align-items: center; gap: 7px; margin-top: 4px;
        font-size: 11.5px; color: #A9B4C3; }
      #${PANEL} .vd-dot { width: 7px; height: 7px; border-radius: 50%; background: #6B7686; flex: none; }
      #${PANEL} .vd-state.ok .vd-dot { background: #58B77E; box-shadow: 0 0 0 3px rgba(88,183,126,.14); }
      #${PANEL} .vd-state.warn .vd-dot { background: #D9A15A; box-shadow: 0 0 0 3px rgba(217,161,90,.14); }
      #${PANEL} .vd-state.bad .vd-dot { background: #D9655C; box-shadow: 0 0 0 3px rgba(217,101,92,.14); }
      #${PANEL} .vd-sep { height: 1px; margin: 4px 6px; background: #222A35; }
      #${PANEL} .vd-label { padding: 6px 11px 4px; font-size: 10px; font-weight: 700;
        letter-spacing: .08em; text-transform: uppercase; color: #8B96A5; }
      #${PANEL} .vd-plans { display: flex; flex-direction: column; padding: 0 4px 2px; }
      #${PANEL} .vd-plan { display: flex; align-items: center; gap: 8px; padding: 6px 8px;
        border-radius: 9px; font-size: 12px; color: #A9B4C3; }
      #${PANEL} .vd-plan b { font-weight: 600; color: #E9EDF2; overflow: hidden;
        text-overflow: ellipsis; white-space: nowrap; }
      #${PANEL} .vd-flag { margin-left: auto; font: 600 10px Montserrat, Arial, sans-serif;
        padding: 2px 7px; border-radius: 999px; border: 1px solid #2E3A49; color: #8B96A5; }
      #${PANEL} .vd-flag.ok { color: #7FCFA2; border-color: #2F5B45; }
      #${PANEL} .vd-flag.warn { color: #D9A15A; border-color: #5B4A2F; }
      #${PANEL} .vd-alert { display: flex; align-items: center; gap: 8px; padding: 8px 10px; margin: 2px 4px; border: 1px solid rgba(237,180,95,.32); border-radius: 9px; background: rgba(237,180,95,.08); font-size: 11.5px; color: #F0C887; }
      #${PANEL} .vd-alert-text { flex: 1; line-height: 1.3; }
      #${PANEL} .vd-mini { height: 24px; padding: 0 9px; border: 1px solid rgba(237,180,95,.4); border-radius: 7px; background: transparent; color: #F0C887; font: 600 11px Montserrat, Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-mini:hover { background: rgba(237,180,95,.16); }
      #${PANEL} .vd-empty { padding: 8px 11px 10px; font-size: 11.5px; color: #8B96A5; }
      #${PANEL} .vd-actions { display: flex; flex-direction: column; gap: 2px; padding: 2px 4px 4px; }
      #${PANEL} .vd-item { display: flex; align-items: center; width: 100%; min-height: 34px;
        padding: 7px 10px; gap: 10px; background: transparent; border: 1px solid transparent;
        border-radius: 9px; color: #A9B4C3; font: 500 12.5px Montserrat, "Motiva Sans", Arial, sans-serif;
        text-align: left; cursor: pointer; }
      #${PANEL} .vd-item:hover { background: #1A212B; color: #E9EDF2; }
      #${PANEL} .vd-item svg { width: 15px; height: 15px; flex: none; opacity: .9; }
      #${PANEL} .vd-msg { min-height: 15px; padding: 4px 11px 8px; font-size: 11.5px; color: #A9B4C3; }
    `;
    document.head.appendChild(style);

    const panel = document.createElement("div");
    panel.id = PANEL;
    panel.innerHTML = `
      <div class="vd-head">
        <div class="vd-title"><span class="name">ExxaTools</span><span class="vd-ver" id="vd-ver">—</span><button class="vd-x" data-act="close" title="Close">×</button></div>
        <div class="vd-state" id="vd-state"><span class="vd-dot"></span><span id="vd-state-text">checking engine</span></div>
      </div>
      <div class="vd-sep"></div>
      <div class="vd-alert" id="vd-alert" hidden>
        <span class="vd-alert-text" id="vd-alert-text"></span>
        <button class="vd-mini" data-act="keys">Fix</button>
      </div>
      <div class="vd-sep"></div>
      <div class="vd-label">Plans</div>
      <div class="vd-plans" id="vd-plans"><div class="vd-empty">no plans yet</div></div>
      <div class="vd-sep"></div>
      <div class="vd-actions">
        <button class="vd-item" data-act="open">ICON_GLYPH_APP Open ExxaTools</button>
        <button class="vd-item" data-act="restart">ICON_GLYPH_RESTART Restart Steam</button>
        <button class="vd-item" data-act="provider">ICON_GLYPH_PROVIDER Install provider</button>
      </div>
      <div class="vd-sep"></div>
      <div class="vd-actions">
        <button class="vd-item" data-act="about">ICON_GLYPH_ABOUT About</button>
      </div>
      <div class="vd-msg" id="vd-msg"></div>
    `
      .replace("ICON_GLYPH_APP", glyph("app"))
      .replace("ICON_GLYPH_RESTART", glyph("restart"))
      .replace("ICON_GLYPH_PROVIDER", glyph("provider"))
      .replace("ICON_GLYPH_ABOUT", glyph("about"));
    document.body.appendChild(panel);

    const msg = (text) => {
      const node = panel.querySelector("#vd-msg");
      if (node) node.textContent = text;
    };

    // Steam's store and community pages send a CSP that blocks requests to
    // loopback, so the panel never talks to the API itself: the server pushes a
    // state snapshot in, and clicks are read back through the same channel.
    window.__vdRender = (state) => {
      state = state || {};
      const engine = state.engine;
      const stateNode = panel.querySelector("#vd-state");
      const text = panel.querySelector("#vd-state-text");
      const version = panel.querySelector("#vd-ver");
      if (!engine) {
        text.textContent = "ExxaTools is not answering";
        stateNode.className = "vd-state bad";
      } else {
        version.textContent = engine.version || "—";
        let label = "engine not installed";
        let kind = "bad";
        if (engine.installed) {
          if (!engine.steam_running) { label = "Steam is not running"; kind = "warn"; }
          else if (engine.steam_engine_loaded) { label = "engine loaded in Steam"; kind = "ok"; }
          else { label = "restart Steam to load the engine"; kind = "warn"; }
        }
        text.textContent = label;
        stateNode.className = "vd-state " + kind;
      }
      const box = panel.querySelector("#vd-plans");
      const plans = state.apps || [];
      if (!plans.length) {
        box.innerHTML = '<div class="vd-empty">no plans yet</div>';
      } else {
        box.replaceChildren(...plans.slice(0, 4).map((plan) => {
          const missing = (plan.missing_keys || []).length;
          const flag = plan.installed
            ? (missing ? { text: "keys", kind: "warn" } : { text: "ready", kind: "ok" })
            : { text: "plan", kind: "" };
          const row = document.createElement("div");
          row.className = "vd-plan";
          const name = document.createElement("b");
          name.textContent = plan.name || String(plan.appid);
          const chip = document.createElement("span");
          chip.className = "vd-flag " + flag.kind;
          chip.textContent = flag.text;
          row.append(name, chip);
          return row;
        }));
      }
      const alerts = state.diagnose || [];
      const alertBox = panel.querySelector("#vd-alert");
      if (alertBox) {
        if (alerts.length) {
          alertBox.hidden = false;
          panel.querySelector("#vd-alert-text").textContent = alerts[0].title;
          const fix = alertBox.querySelector(".vd-mini");
          if (fix) fix.dataset.act = alerts[0].action || "keys";
        } else {
          alertBox.hidden = true;
        }
      }
      if (state.msg !== undefined) msg(state.msg || "");
    };

    panel.addEventListener("click", (event) => {
      const button = event.target.closest("[data-act]");
      if (!button) return;
      window.__vdAction = button.dataset.act;
      msg("working…");
    });
    return panel;
  };

  const panel = document.getElementById(PANEL) || build();
  if (!panel) return false;

  if (opts && opts.open === false) {
    panel.classList.remove("open");
    return true;
  }

  panel.style.left = Math.max(8, Number((opts && opts.x) || 8)) + "px";
  panel.style.top = Math.max(8, Number((opts && opts.y) || 44)) + "px";
  panel.classList.add("open");
  if (opts && opts.state && window.__vdRender) window.__vdRender(opts.state);
  return true;
};

window.__vdUpdate = (state) => {
  if (window.__vdRender) window.__vdRender(state);
  return true;
};

// The icon belongs to the window that owns the Help menu.
window.__vdInject = () => {
  if (window.__vdNoIcon) return "panel-only";
  const API = "__API__";
  const ROOT = "__ROOT__";
  const ICON = "__ICON__";

  if (document.getElementById(ROOT)) return "present";

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

  const toggle = () => {
    const rect = item.getBoundingClientRect();
    fetch(API + "/api/steamui/panel", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        toggle: true,
        x: Math.round(rect.left),
        y: Math.round(rect.bottom),
        screen_x: Math.round(window.screenX || 0),
        screen_y: Math.round(window.screenY || 0),
      }),
    }).catch(() => {});
  };

  item.addEventListener("click", (event) => {
    event.stopPropagation();
    toggle();
  });
  document.addEventListener("keydown", (event) => {
    if (event.ctrlKey && event.shiftKey && (event.key === "E" || event.key === "e")) {
      event.preventDefault();
      toggle();
    }
    if (event.key === "Escape") {
      fetch(API + "/api/steamui/panel", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ open: false }) }).catch(() => {});
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

window.__vdInject();