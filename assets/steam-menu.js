window.__vdGlyphs = __GLYPHS__;

// Steam runs several CEF windows: the shell that owns the menu bar, and separate
// windows for the store, the library and Community. The panel is built in
// whichever window the user is looking at, so it renders inside Steam instead of
// floating above the desktop.
window.__vdPanel = (opts) => {
  const API = "__API__";
  const PANEL = "__PANEL__";
  const glyph = (name) => (window.__vdGlyphs || {})[name] || "";
  const escapeHtml = (value) => String(value == null ? "" : value)
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;").replace(/'/g, "&#39;");

  const openDashboard = (route) => {
    let shell = document.getElementById(PANEL + "-dash");
    if (!shell) {
      shell = document.createElement("div");
      shell.id = PANEL + "-dash";
      shell.innerHTML = '<div class="vd-dash-bar">ExxaTools<span>dashboard</span>'
        + '<button class="vd-dash-x" title="Close">×</button></div>'
        + '<iframe title="ExxaTools dashboard"></iframe>';
      shell.querySelector("iframe").src = API + (typeof route === "string" && route ? route : "/");
      shell.querySelector(".vd-dash-x").addEventListener("click", () => shell.classList.remove("open"));
      document.body.appendChild(shell);
    }
    if (typeof route === "string" && route) {
      const frame = shell.querySelector("iframe");
      if (frame) frame.src = API + route;
    }
    shell.classList.add("open");
    return true;
  };
  const POS_KEY = "exxa.panel.pos";
  const readPos = () => {
    try {
      const raw = JSON.parse(localStorage.getItem(POS_KEY) || "null");
      if (raw && Number.isFinite(raw.x) && Number.isFinite(raw.y)) return raw;
    } catch (error) { /* storage is optional */ }
    return null;
  };
  const savePos = (x, y) => {
    try { localStorage.setItem(POS_KEY, JSON.stringify({ x: Math.round(x), y: Math.round(y) })); } catch (error) { }
  };
  const clampPos = (x, y) => {
    const current = document.getElementById(PANEL);
    const width = 360;
    const height = current ? Math.max(120, current.getBoundingClientRect().height) : 120;
    return {
      x: Math.max(8, Math.min(x, window.innerWidth - width - 8)),
      y: Math.max(8, Math.min(y, window.innerHeight - height - 8)),
    };
  };

  const build = () => {
    if (!document.body) return null;
    const style = document.createElement("style");
    style.id = PANEL + "-style";
    style.textContent = `
      #${PANEL} {
        position: fixed; z-index: 2147483647; width: 360px;
        background: #09090D; border: 1px solid #2A2935; border-radius: 8px;
        box-shadow: 0 16px 42px rgba(0,0,0,.46);
        font: 500 12px/1.4 "Inter", "Geist", "Noto Sans", "Motiva Sans", sans-serif;
        color: #F4F2FA; display: none;
        max-height: calc(100vh - 16px); overflow-y: auto; overscroll-behavior: contain;
      }
      #${PANEL}.open { display: block; }
      #${PANEL} [hidden] { display: none !important; }
      #${PANEL} .vd-head { cursor: grab; user-select: none; padding: 10px 12px;
        border-bottom: 1px solid #232230; }
      #${PANEL}.dragging, #${PANEL}.dragging .vd-head { cursor: grabbing; }
      #${PANEL} .vd-brand { display: flex; align-items: center; gap: 8px; }
      #${PANEL} .vd-name { flex: 1; padding: 0; border: 0; background: transparent;
        color: #F4F2FA; font: 600 14px/1.2 "Inter", "Geist", "Noto Sans", "Motiva Sans", sans-serif;
        text-align: left; cursor: pointer; }
      #${PANEL} .vd-name:hover { color: #C4B5FD; }
      #${PANEL} .vd-x, #${PANEL} .vd-more {
        width: 28px; height: 28px; flex: none; display: grid; place-items: center;
        border: 0; border-radius: 6px; background: transparent;
        color: #A9A3BC; font: 500 17px/1 "Inter", sans-serif; cursor: pointer;
      }
      #${PANEL} .vd-x:hover, #${PANEL} .vd-more:hover { background: #241E38; color: #fff; }
      #${PANEL} .vd-state { display: flex; align-items: center; gap: 7px; margin-top: 8px;
        font-size: 11px; color: #A9A3BC; }
      #${PANEL} .vd-dot { width: 7px; height: 7px; border-radius: 50%; background: #6F6A7D; flex: none; }
      #${PANEL} .vd-state.ok .vd-dot { background: #8fc7a5; }
      #${PANEL} .vd-state.warn .vd-dot { background: #d8b27a; }
      #${PANEL} .vd-state.bad .vd-dot { background: #d96a60; }
      #${PANEL} .vd-body { padding: 10px 12px 12px; }
      #${PANEL} .vd-alert { display: flex; align-items: center; gap: 7px; margin-bottom: 10px;
        padding: 8px 9px; border: 1px solid rgba(216,178,122,.32); border-radius: 6px;
        background: rgba(216,178,122,.08); font-size: 11px; color: #e9c48a; }
      #${PANEL} .vd-alert-text { flex: 1; line-height: 1.35; }
      #${PANEL} .vd-mini { min-height: 26px; padding: 3px 9px; border: 1px solid #4C3A78;
        border-radius: 6px; background: #241E38; color: #F4F2FA;
        font: 600 11px "Inter", "Geist", "Noto Sans", "Motiva Sans", sans-serif; cursor: pointer; }
      #${PANEL} .vd-quick { display: grid; grid-template-columns: 1fr auto auto; gap: 8px; }
      #${PANEL} .vd-action { min-height: 32px; padding: 6px 11px; border: 1px solid #2A2935;
        border-radius: 6px; background: #14141D; color: #F4F2FA;
        font: 600 12px "Inter", "Geist", "Noto Sans", "Motiva Sans", sans-serif; cursor: pointer; }
      #${PANEL} .vd-action:hover { background: #241E38; border-color: #8B5CF6; color: #fff; }
      #${PANEL} .vd-action.primary { background: #8B5CF6; border-color: #8B5CF6;
        color: #09090D; font-weight: 700; }
      #${PANEL} .vd-action.primary:hover { background: #C4B5FD; border-color: #C4B5FD; }
      #${PANEL} .vd-action.restart { border-color: #4C3A78; background: #1B1526; color: #C4B5FD; }
      #${PANEL} .vd-action.restart:hover { background: #2A2140; border-color: #8B5CF6; color: #fff; }
      #${PANEL} .vd-label { margin: 14px 1px 7px; color: #8B8598; font-size: 11px; font-weight: 600; }
      #${PANEL} .vd-games { display: flex; flex-direction: column; gap: 6px; }
      #${PANEL} .vd-games:empty { display: none; }
      #${PANEL} .vd-game { position: relative; display: grid;
        grid-template-columns: 34px minmax(0,1fr) auto 28px; align-items: center; gap: 8px;
        min-height: 58px; padding: 5px 6px; border: 1px solid #2A2935; border-radius: 8px;
        background: #101017; }
      #${PANEL} .vd-game.expanded { border-color: #4C3A78; }
      #${PANEL} .vd-art { width: 34px; height: 48px; object-fit: cover; border-radius: 4px;
        background: #14141D; }
      #${PANEL} .vd-art-fallback { width: 34px; height: 48px; display: grid; place-items: center;
        border-radius: 4px; background: #241E38; color: #8B8598; font-weight: 700; }
      #${PANEL} .vd-game-copy { min-width: 0; }
      #${PANEL} .vd-game-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
        font-size: 12px; font-weight: 600; color: #F4F2FA; }
      #${PANEL} .vd-game-state { margin-top: 3px; font-size: 10.5px; color: #8B8598; }
      #${PANEL} .vd-game.ready .vd-game-state { color: #8fc7a5; }
      #${PANEL} .vd-game.keys_missing .vd-game-state { color: #d96a60; }
      #${PANEL} .vd-play { min-height: 28px; padding: 4px 9px; border: 1px solid #4C3A78;
        border-radius: 6px; background: #241E38; color: #F4F2FA;
        font: 600 11px "Inter", "Geist", "Noto Sans", "Motiva Sans", sans-serif; cursor: pointer; }
      #${PANEL} .vd-play:hover { background: #8B5CF6; border-color: #8B5CF6; color: #09090D; }
      #${PANEL} .vd-more { font-size: 16px; line-height: 1; }
      #${PANEL} .vd-game-more { grid-column: 1 / -1; display: grid; grid-template-columns: 1fr 1fr;
        gap: 6px; padding: 7px 0 1px; border-top: 1px solid #2A2935; }
      #${PANEL} .vd-game-more button { min-height: 28px; padding: 4px 7px;
        border: 1px solid #2A2935; border-radius: 6px; background: #14141D; color: #D8D3E2;
        font: 500 11px "Inter", "Geist", "Noto Sans", "Motiva Sans", sans-serif; cursor: pointer; }
      #${PANEL} .vd-game-more button:hover { background: #241E38; color: #fff; }
      #${PANEL} .vd-empty { padding: 18px 10px; border: 1px dashed #2A2935; border-radius: 8px;
        text-align: center; color: #8B8598; }
      #${PANEL} .vd-empty strong { display: block; margin-bottom: 4px; color: #F4F2FA; font-size: 12px; }
      #${PANEL} .vd-msg { padding: 8px 2px 0; font-size: 11px; line-height: 1.35; color: #A9A3BC; }
      #${PANEL} .vd-msg:empty { display: none; }
      #${PANEL} .vd-plugins { display: flex; flex-direction: column; gap: 7px; margin-top: 10px; }
      #${PANEL} .vd-plugins section { padding: 10px; border: 1px solid #2A2935;
        border-radius: 8px; background: #101017; }
      #${PANEL} .vd-plugins section .vd-label { margin: 0 0 6px; color: #A9A3BC; }
      #${PANEL} .vd-plugins:empty { display: none; }
      #${PANEL} .vd-foot-row { display: flex; align-items: center; gap: 10px; margin-top: 12px;
        padding-top: 9px; border-top: 1px solid #232230; }
      #${PANEL} .vd-link { padding: 4px 0; border: 0; background: none;
        color: #A9A3BC; font: 500 11px "Inter", "Geist", "Noto Sans", "Motiva Sans", sans-serif; cursor: pointer; }
      #${PANEL} .vd-link:hover { color: #C4B5FD; }
      #${PANEL} .vd-ver { margin-left: auto; font-size: 10.5px; color: #6F6A7D;
        font-variant-numeric: tabular-nums; }
      #${PANEL} button:focus-visible { outline: 2px solid #C4B5FD; outline-offset: 2px; }
      #${PANEL} button:disabled { opacity: .55; cursor: default; }
      #${PANEL}-dash {
        position: fixed; inset: 0; z-index: 2147483647;
        display: none; flex-direction: column; background: #09090D;
        font-family: "Inter", "Geist", "Noto Sans", "Motiva Sans", sans-serif;
      }
      #${PANEL}-dash.open { display: flex; }
      #${PANEL}-dash .vd-dash-bar {
        display: flex; align-items: center; gap: 10px; height: 40px; flex: none;
        padding: 0 10px 0 14px; background: #09090D; border-bottom: 1px solid #232230;
        color: #F4F2FA; font-size: 12.5px; font-weight: 600;
      }
      #${PANEL}-dash .vd-dash-bar span { flex: 1; color: #8B8598; font-weight: 400; }
      #${PANEL}-dash .vd-dash-x {
        width: 27px; height: 27px; display: grid; place-items: center;
        border: 1px solid #2A2935; border-radius: 6px; background: transparent;
        color: #A9A3BC; font: 500 15px/1 "Inter", sans-serif; cursor: pointer;
      }
      #${PANEL}-dash .vd-dash-x:hover { background: #241E38; color: #ffffff; }
      #${PANEL}-dash iframe { flex: 1; width: 100%; border: 0; background: #09090D; }
    `;
    // Refresh the stylesheet instead of skipping it: a panel injected by an
    // older build leaves a style node behind, and new rules would never apply.
    const stale = document.getElementById(PANEL + "-style");
    if (stale) stale.remove();
    document.head.appendChild(style);

    const panel = document.createElement("div");
    panel.id = PANEL;
    panel.innerHTML = `
      <div class="vd-head">
        <div class="vd-brand"><button class="vd-name" data-act="dashboard" title="Open dashboard">ExxaTools</button><button class="vd-x" data-act="close" title="Close" aria-label="Close">×</button></div>
        <div class="vd-state" id="vd-state" hidden><span class="vd-dot"></span><span id="vd-state-text"></span></div>
      </div>
      <div class="vd-body">
        <div class="vd-alert" id="vd-alert" hidden>
          <span class="vd-alert-text" id="vd-alert-text"></span>
          <button class="vd-mini" data-act="keys">Fix</button>
          <button class="vd-mini" data-act="dismiss" title="Hide until it happens again" aria-label="Dismiss">×</button>
        </div>
        <div class="vd-quick">
          <button class="vd-action primary" data-act="discover">Add games</button>
          <button class="vd-action" data-act="sync_all">Sync all</button>
          <button class="vd-action restart" data-act="restart">Restart Steam</button>
        </div>
        <div class="vd-label vd-library-label" id="vd-library-label" hidden>Library</div>
        <div class="vd-games" id="vd-games"></div>
        <div class="vd-label vd-label-plugins" hidden>Plugins</div>
        <div class="vd-plugins" id="vd-plugins-host"></div>
        <div class="vd-msg" id="vd-msg"></div>
        <div class="vd-foot-row">
          <button class="vd-link" data-act="settings">Settings</button>
          <button class="vd-link" data-act="system">System check</button>
          <span class="vd-ver" id="vd-ver"></span>
        </div>
      </div>
    `
      .replace("ICON_GLYPH_APP", glyph("app"))
      .replace("ICON_GLYPH_RESTART", glyph("restart"))
      .replace("ICON_GLYPH_PROVIDER", glyph("provider"))
      .replace("ICON_GLYPH_ABOUT", glyph("about"));
    document.body.appendChild(panel);
    panel.dataset.build = window.__vdBuild || "";
    // Plugin sections live in their own root; move it into the panel so a rebuild
    // keeps whatever plugins already rendered instead of dropping it.
    const pluginHost = panel.querySelector("#vd-plugins-host");
    if (pluginHost) {
      let root = document.getElementById("exxa-plugin-root");
      if (!root) {
        root = document.createElement("div");
        root.id = "exxa-plugin-root";
      }
      pluginHost.appendChild(root);
      const pluginsLabel = panel.querySelector(".vd-label-plugins");
      const syncPlugins = () => {
        if (pluginsLabel) pluginsLabel.hidden = root.children.length === 0;
      };
      new MutationObserver(syncPlugins).observe(root, { childList: true });
      syncPlugins();
    }

    let msgTimer = null;
    const msg = (text, options = {}) => {
      const node = panel.querySelector("#vd-msg");
      if (!node) return;
      node.textContent = text || "";
      if (msgTimer) { clearTimeout(msgTimer); msgTimer = null; }
      if (text && !options.persist && text !== "working…") {
        msgTimer = setTimeout(() => {
          const current = panel.querySelector("#vd-msg");
          if (current && current.textContent === text) current.textContent = "";
        }, 5000);
      }
    };
    window.__vdSay = (text) => msg(text);

    // Steam's store and community pages send a CSP that blocks requests to
    // loopback, so the panel never talks to the API itself: the server pushes a
    // state snapshot in, and clicks are read back through the same channel.
    window.__vdRender = (state) => {
      state = state || {};
      const engine = state.engine;
      const stateNode = panel.querySelector("#vd-state");
      const text = panel.querySelector("#vd-state-text");
      const version = panel.querySelector("#vd-ver");
      version.textContent = state.version ? "v" + state.version : (engine && engine.version ? engine.version : "");
      if (!engine) {
        stateNode.hidden = true;
      } else if (!engine.installed) {
        stateNode.hidden = false;
        text.textContent = "Engine is not installed";
        stateNode.className = "vd-state bad";
      } else if (!engine.steam_running) {
        stateNode.hidden = false;
        text.textContent = "Steam is not running";
        stateNode.className = "vd-state warn";
      } else if (!engine.steam_engine_loaded) {
        stateNode.hidden = false;
        text.textContent = "Restart Steam to load the engine";
        stateNode.className = "vd-state warn";
      } else {
        stateNode.hidden = true;
      }
      window.__vdLastState = state;
      const games = (Array.isArray(state.apps) ? state.apps : [])
        .filter((game) => Number(game && game.appid) > 0);
      const gamesHost = panel.querySelector("#vd-games");
      const gamesLabel = panel.querySelector("#vd-library-label");
      if (gamesLabel) {
        gamesLabel.hidden = false;
        gamesLabel.textContent = games.length ? `Library · ${games.length}` : "Library";
      }
      if (gamesHost) {
        const signature = JSON.stringify(games.map((game) => [
          game.appid, game.name, game.state, game.installed, game.art,
        ]));
        if (gamesHost.dataset.signature !== signature) {
          gamesHost.dataset.signature = signature;
          if (!games.length) {
            gamesHost.innerHTML = '<div class="vd-empty"><strong>No games yet</strong>Add a game to see it here.</div>';
          } else {
            gamesHost.innerHTML = games.slice(0, 6).map((game) => {
              const appid = Number(game.appid);
              const name = escapeHtml(game.name || `App ${appid}`);
              const rawState = String(game.state || "planned");
              const planState = ["ready", "keys_missing", "planned", "disabled"].includes(rawState) ? rawState : "planned";
              const stateText = { ready: "Ready", keys_missing: "Keys missing", planned: "Not installed", disabled: "Disabled" }[planState];
              const primary = game.installed
                ? `<button class="vd-play" data-act="play:${appid}">Play</button>`
                : `<button class="vd-play" data-act="install:${appid}">Install</button>`;
              const art = game.art
                ? `<img class="vd-art" src="${escapeHtml(game.art)}" alt="" loading="lazy" onerror="this.outerHTML='<span class=&quot;vd-art-fallback&quot;>•</span>'">`
                : `<span class="vd-art-fallback">${escapeHtml(name.slice(0, 1))}</span>`;
              return `<div class="vd-game ${planState}" data-game="${appid}">`
                + art
                + `<div class="vd-game-copy"><div class="vd-game-name" title="${name}">${name}</div>`
                + `<div class="vd-game-state">${stateText}</div></div>`
                + primary
                + `<button class="vd-more" data-act="more:${appid}" title="More actions" aria-label="More actions for ${name}" aria-expanded="false">⋯</button>`
                + `<div class="vd-game-more" hidden>`
                + `<button data-act="sync:${appid}">Sync manifests</button>`
                + `<button data-act="fix:${appid}" title="Repair update flags and restart Steam">Fix update</button>`
                + `</div></div>`;
            }).join("");
          }
        }
      }
      window.__vdHidden = window.__vdHidden || new Set();
      const key = (incident) => `${incident.id}:${(incident.line || "").slice(0, 60)}`;
      const alerts = (state.diagnose || [])
        .filter((incident) => String(incident.title || "").trim().length > 0)
        .filter((incident) => !window.__vdHidden.has(key(incident)));
      const alertBox = panel.querySelector("#vd-alert");
      if (alertBox) {
        if (alerts.length) {
          alertBox.hidden = false;
          const count = alerts.length > 1 ? `${alerts.length} problems · ` : "";
          panel.querySelector("#vd-alert-text").textContent = count + alerts[0].title;
          const buttons = alertBox.querySelectorAll(".vd-mini");
          if (buttons[0]) buttons[0].dataset.act = alerts[0].action || "keys";
          if (buttons[1]) buttons[1].dataset.act = alerts[0].action === "dismiss" ? "dismiss" : "dismiss";
        } else {
          alertBox.hidden = true;
        }
      }
      if (state.msg !== undefined) msg(state.msg || "");
    };

    const head = panel.querySelector(".vd-head");
    let drag = null;
    head.addEventListener("pointerdown", (event) => {
      if (event.target.closest("button")) return;
      const rect = panel.getBoundingClientRect();
      drag = { dx: event.clientX - rect.left, dy: event.clientY - rect.top };
      try { head.setPointerCapture(event.pointerId); } catch (error) { /* synthetic events */ }
      panel.classList.add("dragging");
    });
    head.addEventListener("pointermove", (event) => {
      if (!drag) return;
      const next = clampPos(event.clientX - drag.dx, event.clientY - drag.dy);
      panel.style.left = next.x + "px";
      panel.style.top = next.y + "px";
    });
    const endDrag = () => {
      if (!drag) return;
      drag = null;
      panel.classList.remove("dragging");
      const rect = panel.getBoundingClientRect();
      savePos(rect.left, rect.top);
    };
    head.addEventListener("pointerup", endDrag);
    head.addEventListener("pointercancel", endDrag);

    panel.addEventListener("click", (event) => {
      const button = event.target.closest("[data-act]");
      if (!button) return;
      if (button.dataset.act.startsWith("more:")) {
        const row = button.closest(".vd-game");
        const actions = row && row.querySelector(".vd-game-more");
        if (!actions) return;
        const show = actions.hidden;
        panel.querySelectorAll(".vd-game-more").forEach((node) => { node.hidden = true; });
        panel.querySelectorAll(".vd-game.expanded").forEach((node) => node.classList.remove("expanded"));
        panel.querySelectorAll(".vd-more[aria-expanded]").forEach((node) => node.setAttribute("aria-expanded", "false"));
        actions.hidden = !show;
        row.classList.toggle("expanded", show);
        button.setAttribute("aria-expanded", String(show));
        return;
      }
      if (button.dataset.act === "close") {
        window.__vdPanelRequest = { close: true };
        if (window.__vdPanel) window.__vdPanel({ open: false });
        return;
      }
      if (button.dataset.act === "dismiss") {
        const alerts = (window.__vdLastState && window.__vdLastState.diagnose) || [];
        const incident = alerts[0];
        if (incident) {
          window.__vdHidden = window.__vdHidden || new Set();
          window.__vdHidden.add(`${incident.id}:${(incident.line || "").slice(0, 60)}`);
        }
        window.__vdAction = "";
        return;
      }
      if (button.dataset.act === "open") {
        // A store page blocks loopback iframes, so ask Rust to open the dashboard
        // overlay in the Steam shell window instead.
        window.__vdAction = "dashboard";
        msg("opening…");
        return;
      }
      window.__vdAction = button.dataset.act;
      msg("working…");
    });
    return panel;
  };

  // Rebuild only when a new build was injected: the old panel's handlers would
  // otherwise survive, but rebuilding on every toggle would drop plugin sections.
  const existing = document.getElementById(PANEL);
  if (existing && existing.dataset.build === (window.__vdBuild || "")) {
    // keep the current panel
  } else if (existing) {
    // Plugin sections live inside the panel; move their root back to the page so a
    // rebuild does not destroy everything plugins already rendered.
    const pluginRoot = existing.querySelector("#exxa-plugin-root");
    if (pluginRoot) document.body.appendChild(pluginRoot);
    existing.remove();
  }
  const panel = document.getElementById(PANEL) || build();
  if (!panel) return false;

  if (opts && opts.open === false) {
    panel.classList.remove("open");
    window.__vdPanelOpen = false;
    return true;
  }

  const saved = readPos();
  const start = saved || { x: Number((opts && opts.x) || 8), y: Number((opts && opts.y) || 44) };
  const placed = clampPos(start.x, start.y);
  panel.style.left = placed.x + "px";
  panel.style.top = placed.y + "px";
  panel.classList.add("open");
  window.__vdPanelOpen = true;
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
  const PANEL = "__PANEL__";

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

  // The icon opens the full dashboard, not the floating quick panel. Rust owns
  // the window launch, so the shell target only raises the request flag.
  const open_app = () => {
    window.__vdPanelRequest = { close: true };
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
    if (event.key === "Escape") {
      const shell = document.getElementById("__PANEL__-dash");
      if (shell && shell.classList.contains("open")) { shell.classList.remove("open"); return; }
      window.__vdPanelRequest = { close: true };
      if (window.__vdPanel) window.__vdPanel({ open: false });
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
  window.__vdAppDismissed = null;
  window.__vdAppWidgetId = null;

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
      } else if (state.can_add) {
        html = button("add", "Add", "primary", plusIcon);
      } else if (state.can_sync) {
        html = button("sync", "Sync", "secondary", glyph("sync"));
        if (state.can_fix) html += sep + button("fix", "Fix update", "secondary", glyph("fix"));
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
  const key = JSON.stringify(state || null) + "|" + (window.__vdPageBusy || "");
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


// Top-level so a fresh injection repairs the route without rebuilding the panel.
window.__vdOpenDashboard = (route) => {
  const API = "__API__";
  const id = "__PANEL__-dash";
  let shell = document.getElementById(id);
  if (!shell) {
    shell = document.createElement("div");
    shell.id = id;
    shell.innerHTML = '<div class="vd-dash-bar">ExxaTools<span>dashboard</span>'
      + '<button class="vd-dash-x" title="Close">×</button></div>'
      + '<iframe title="ExxaTools dashboard"></iframe>';
    shell.querySelector(".vd-dash-x").addEventListener("click", () => shell.classList.remove("open"));
    document.body.appendChild(shell);
  }
  const frame = shell.querySelector("iframe");
  const url = API + (typeof route === "string" && route ? route : "/");
  if (frame && frame.src !== url) frame.src = url;
  shell.classList.add("open");
  return true;
};

window.__vdInject();
