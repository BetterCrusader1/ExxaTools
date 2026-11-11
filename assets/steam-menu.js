window.__vdGlyphs = __GLYPHS__;
window.__vdToast = (text) => {
  const node = document.createElement("div");
  Object.assign(node.style, {
    position: "fixed", right: "18px", bottom: "18px", zIndex: 2147483001,
    padding: "10px 14px", borderRadius: "10px",
    background: "#151A21", border: "1px solid #2E3A49", color: "#E9EDF2",
    font: "500 12.5px Montserrat, 'Motiva Sans', Arial, sans-serif",
    boxShadow: "0 18px 40px rgba(0,0,0,.55)", opacity: "0",
    transition: "opacity 160ms ease, transform 160ms ease",
    transform: "translateY(6px)",
  });
  node.textContent = text;
  document.body.appendChild(node);
  requestAnimationFrame(() => {
    node.style.opacity = "1";
    node.style.transform = "translateY(0)";
  });
  setTimeout(() => {
    node.style.opacity = "0";
    setTimeout(() => node.remove(), 200);
  }, 2600);
};

window.__vdInject = () => {
  const API = "__API__";
  const ROOT = "__ROOT__";
  const PANEL = "__PANEL__";
  const ICON = "__ICON__";

  if (document.getElementById(ROOT)) return "present";

  // The Help item is a leaf div inside a one-child wrapper inside the menubar.
  let help = null;
  for (const el of document.querySelectorAll("div,button,span,a,li")) {
    if (el.childElementCount) continue;
    if (el.textContent.trim() !== "Help") continue;
    const rect = el.getBoundingClientRect();
    if (rect.width > 0 && rect.top < 60) { help = el; break; }
  }
  if (!help) return "no-menubar";
  const wrapper = help.parentElement;
  const bar = wrapper && wrapper.parentElement;
  if (!wrapper || !bar) return "no-anchor";

  // Reuse Steam's own menu item markup so hover and spacing stay native.
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

  const style = document.createElement("style");
  style.id = ROOT + "-style";
  style.textContent = `
    #${PANEL} {
      position: fixed; z-index: 2147483000; width: 272px; padding: 6px;
      background: #151A21; border: 1px solid #2E3A49; border-radius: 14px;
      box-shadow: 0 24px 60px rgba(0,0,0,.62);
      font-family: Montserrat, "Motiva Sans", Arial, sans-serif;
      color: #E9EDF2; display: none;
    }
    #${PANEL}.open { display: block; }
    #${PANEL} .vd-head { padding: 8px 10px 9px; }
    #${PANEL} .vd-title { display: flex; justify-content: space-between; align-items: baseline;
      font-size: 13px; font-weight: 700; letter-spacing: .01em; }
    #${PANEL} .vd-ver { font-family: ui-monospace, monospace; font-size: 10.5px; font-weight: 400;
      color: #6B7686; }
    #${PANEL} .vd-state { display: flex; align-items: center; gap: 6px; margin-top: 3px;
      font-size: 11.5px; color: #9DA8B7; }
    #${PANEL} .vd-dot { width: 6px; height: 6px; border-radius: 50%; background: #6B7686; }
    #${PANEL} .vd-state.ok .vd-dot { background: #58B77E; }
    #${PANEL} .vd-state.warn .vd-dot { background: #D9A15A; }
    #${PANEL} .vd-state.bad .vd-dot { background: #D9655C; }
    #${PANEL} .vd-sep { height: 1px; margin: 6px 4px; background: #222A35; }
    #${PANEL} .vd-item { display: flex; align-items: center; width: 100%; min-height: 32px;
      padding: 6px 10px; gap: 9px; background: transparent; border: 1px solid transparent;
      border-radius: 9px; color: #9DA8B7; font: 500 12.5px Montserrat, "Motiva Sans", Arial, sans-serif;
      text-align: left; cursor: pointer; }
    #${PANEL} .vd-item:hover { background: #1A212B; color: #E9EDF2; }
    #${PANEL} .vd-item svg { width: 15px; height: 15px; flex: none; opacity: .9; }
    #${PANEL} .vd-plans { padding: 2px 4px 4px; }
    #${PANEL} .vd-plan { display: flex; align-items: center; gap: 8px; padding: 5px 6px;
      border-radius: 8px; font-size: 12px; color: #9DA8B7; }
    #${PANEL} .vd-plan b { font-weight: 600; color: #E9EDF2; overflow: hidden;
      text-overflow: ellipsis; white-space: nowrap; }
    #${PANEL} .vd-flag { margin-left: auto; font: 600 10px Montserrat, Arial, sans-serif;
      padding: 2px 6px; border-radius: 999px; border: 1px solid #2E3A49; color: #6B7686; }
    #${PANEL} .vd-flag.ok { color: #7FCFA2; border-color: #2F5B45; }
    #${PANEL} .vd-flag.warn { color: #D9A15A; border-color: #5B4A2F; }
    #${PANEL} .vd-empty { padding: 6px 10px; font-size: 11.5px; color: #6B7686; }
    #${PANEL} .vd-msg { padding: 6px 10px 2px; font-size: 11.5px; color: #9DA8B7; min-height: 16px; }
  `;
  document.head.appendChild(style);

  const panel = document.createElement("div");
  panel.id = PANEL;
  panel.innerHTML = `
    <div class="vd-head">
      <div class="vd-title">ExxaTools <span class="vd-ver" id="vd-ver">—</span></div>
      <div class="vd-state" id="vd-state"><span class="vd-dot"></span><span id="vd-state-text">checking engine</span></div>
    </div>
    <div class="vd-sep"></div>
    <div class="vd-plans" id="vd-plans"><div class="vd-empty">no plans yet</div></div>
    <div class="vd-sep"></div>
    <button class="vd-item" data-act="open">ICON_GLYPH_APP Open ExxaTools</button>
    <button class="vd-item" data-act="restart">ICON_GLYPH_RESTART Restart Steam</button>
    <button class="vd-item" data-act="provider">ICON_GLYPH_PROVIDER Install provider</button>
    <div class="vd-sep"></div>
    <button class="vd-item" data-act="about">ICON_GLYPH_ABOUT About</button>
    <div class="vd-msg" id="vd-msg"></div>
  `.replace("ICON_GLYPH_APP", ICON_GLYPH("app"))
   .replace("ICON_GLYPH_RESTART", ICON_GLYPH("restart"))
   .replace("ICON_GLYPH_PROVIDER", ICON_GLYPH("provider"))
   .replace("ICON_GLYPH_ABOUT", ICON_GLYPH("about"));
  document.body.appendChild(panel);

  function ICON_GLYPH(name) { return (window.__vdGlyphs || {})[name] || ""; }

  const msg = (text) => {
    const node = panel.querySelector("#vd-msg");
    if (node) node.textContent = text;
  };

  const setState = (engine) => {
    const state = panel.querySelector("#vd-state");
    const text = panel.querySelector("#vd-state-text");
    const version = panel.querySelector("#vd-ver");
    if (!engine) {
      text.textContent = "ExxaTools is not answering";
      state.className = "vd-state bad";
      return;
    }
    version.textContent = engine.version || "—";
    let label = "engine not installed";
    let kind = "bad";
    if (engine.installed) {
      if (!engine.steam_running) { label = "Steam is not running"; kind = "warn"; }
      else if (engine.steam_engine_loaded) { label = "engine loaded in Steam"; kind = "ok"; }
      else { label = "restart Steam to load the engine"; kind = "warn"; }
    }
    text.textContent = label;
    state.className = "vd-state " + kind;
  };

  const planRow = (plan) => {
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
  };

  const renderPlans = (status) => {
    const box = panel.querySelector("#vd-plans");
    const plans = (status && status.apps) || [];
    if (!plans.length) {
      box.innerHTML = '<div class="vd-empty">no plans yet</div>';
      return;
    }
    box.replaceChildren(...plans.slice(0, 4).map(planRow));
  };

  const refresh = async () => {
    try {
      const response = await fetch(API + "/api/status", { cache: "no-store" });
      const status = await response.json();
      setState(status.engine);
      renderPlans(status);
    } catch (error) {
      setState(null);
    }
  };

  const place = () => {
    const rect = item.getBoundingClientRect();
    const width = 272;
    let left = Math.min(Math.max(8, rect.left - width + rect.width), window.innerWidth - width - 8);
    panel.style.left = left + "px";
    panel.style.top = Math.round(rect.bottom + 10) + "px";
  };

  const close = () => panel.classList.remove("open");
  const open = () => {
    place();
    panel.classList.add("open");
    refresh();
  };
  const toggle = () => (panel.classList.contains("open") ? close() : open());

  item.addEventListener("click", (event) => {
    event.stopPropagation();
    toggle();
  });
  document.addEventListener("click", (event) => {
    if (!panel.contains(event.target) && !item.contains(event.target)) close();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") close();
    if (event.ctrlKey && event.shiftKey && (event.key === "E" || event.key === "e")) {
      event.preventDefault();
      toggle();
    }
  });
  window.addEventListener("resize", () => { if (panel.classList.contains("open")) place(); });

  panel.addEventListener("click", async (event) => {
    const button = event.target.closest("[data-act]");
    if (!button) return;
    const action = button.dataset.act;
    msg("");
    try {
      if (action === "open") {
        const response = await fetch(API + "/api/app/launch", { method: "POST", headers: {"Content-Type": "application/json"}, body: "{}" });
        const data = await response.json();
        msg(data.started ? "opening the dashboard" : "dashboard already running");
      } else if (action === "restart") {
        msg("stopping Steam…");
        await fetch(API + "/api/steam/restart", { method: "POST", headers: {"Content-Type": "application/json"}, body: "{}" });
        msg("Steam is restarting");
      } else if (action === "provider") {
        const response = await fetch(API + "/api/provider", { method: "POST", headers: {"Content-Type": "application/json"}, body: "{}" });
        const data = await response.json();
        const text = data.error ? data.error : "provider script written";
        msg(text);
        window.__vdToast(text);
      } else if (action === "about") {
        msg("ExxaTools — depots, manifests and keys for Vapor Forge");
      }
    } catch (error) {
      msg("ExxaTools is not answering");
    }
  });

  refresh();
  return "injected";
};

if (!window.__vdWatcher) {
  let pending = null;
  window.__vdWatcher = new MutationObserver(() => {
    if (pending) return;
    pending = setTimeout(() => {
      pending = null;
      if (!document.getElementById("__ROOT__") && document.body) {
        try { window.__vdInject(); } catch (error) { /* Steam is mid-render */ }
      }
    }, 250);
  });
  window.__vdWatcher.observe(document.body, { childList: true, subtree: true });
}

window.__vdInject();