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
        background: #171a21; border: 1px solid #3b4a5a; border-radius: 8px;
        box-shadow: 0 16px 42px rgba(0,0,0,.46);
        font: 500 12px/1.4 "Motiva Sans", "Segoe UI", Arial, sans-serif;
        color: #dfe6ed; display: none;
        max-height: calc(100vh - 16px); overflow-y: auto; overscroll-behavior: contain;
      }
      #${PANEL}.open { display: block; }
      #${PANEL} [hidden] { display: none !important; }
      #${PANEL} .vd-head { cursor: grab; user-select: none; padding: 10px 12px;
        border-bottom: 1px solid #2a313b; }
      #${PANEL}.dragging, #${PANEL}.dragging .vd-head { cursor: grabbing; }
      #${PANEL} .vd-brand { display: flex; align-items: center; gap: 8px; }
      #${PANEL} .vd-name { flex: 1; padding: 0; border: 0; background: transparent;
        color: #dfe6ed; font: 600 14px/1.2 "Motiva Sans", "Segoe UI", Arial, sans-serif;
        text-align: left; cursor: pointer; }
      #${PANEL} .vd-name:hover { color: #66c0f4; }
      #${PANEL} .vd-x, #${PANEL} .vd-more {
        width: 28px; height: 28px; flex: none; display: grid; place-items: center;
        border: 0; border-radius: 6px; background: transparent;
        color: #aab7c4; font: 500 17px/1 Arial, sans-serif; cursor: pointer;
      }
      #${PANEL} .vd-x:hover, #${PANEL} .vd-more:hover { background: #2a475e; color: #fff; }
      #${PANEL} .vd-state { display: flex; align-items: center; gap: 7px; margin-top: 8px;
        font-size: 11px; color: #b8c4cf; }
      #${PANEL} .vd-dot { width: 7px; height: 7px; border-radius: 50%; background: #8294a5; flex: none; }
      #${PANEL} .vd-state.ok .vd-dot { background: #8fc7a5; }
      #${PANEL} .vd-state.warn .vd-dot { background: #d8b27a; }
      #${PANEL} .vd-state.bad .vd-dot { background: #d96a60; }
      #${PANEL} .vd-body { padding: 10px 12px 12px; }
      #${PANEL} .vd-alert { display: flex; align-items: center; gap: 7px; margin-bottom: 10px;
        padding: 8px 9px; border: 1px solid rgba(216,178,122,.32); border-radius: 6px;
        background: rgba(216,178,122,.08); font-size: 11px; color: #e9c48a; }
      #${PANEL} .vd-alert-text { flex: 1; line-height: 1.35; }
      #${PANEL} .vd-mini { min-height: 26px; padding: 3px 9px; border: 1px solid #46647a;
        border-radius: 6px; background: #2a475e; color: #e6eef4;
        font: 600 11px "Motiva Sans", Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-quick { display: grid; grid-template-columns: 1fr auto; gap: 8px; }
      #${PANEL} .vd-action { min-height: 32px; padding: 6px 11px; border: 1px solid #3b4a5a;
        border-radius: 6px; background: #202a35; color: #d7e0e8;
        font: 600 12px "Motiva Sans", Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-action:hover { background: #2a475e; border-color: #66c0f4; color: #fff; }
      #${PANEL} .vd-action.primary { background: #66c0f4; border-color: #66c0f4;
        color: #102331; font-weight: 700; }
      #${PANEL} .vd-action.primary:hover { background: #8bd0f7; border-color: #8bd0f7; }
      #${PANEL} .vd-label { margin: 14px 1px 7px; color: #8fa4b5; font-size: 11px; font-weight: 600; }
      #${PANEL} .vd-games { display: flex; flex-direction: column; gap: 6px; }
      #${PANEL} .vd-games:empty { display: none; }
      #${PANEL} .vd-game { position: relative; display: grid;
        grid-template-columns: 34px minmax(0,1fr) auto 28px; align-items: center; gap: 8px;
        min-height: 58px; padding: 5px 6px; border: 1px solid #2f3b47; border-radius: 8px;
        background: #1b232d; }
      #${PANEL} .vd-game.expanded { border-color: #46647a; }
      #${PANEL} .vd-art { width: 34px; height: 48px; object-fit: cover; border-radius: 4px;
        background: #202a35; }
      #${PANEL} .vd-art-fallback { width: 34px; height: 48px; display: grid; place-items: center;
        border-radius: 4px; background: #2a475e; color: #8fa4b5; font-weight: 700; }
      #${PANEL} .vd-game-copy { min-width: 0; }
      #${PANEL} .vd-game-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
        font-size: 12px; font-weight: 600; color: #e3eaf0; }
      #${PANEL} .vd-game-state { margin-top: 3px; font-size: 10.5px; color: #8fa4b5; }
      #${PANEL} .vd-game.ready .vd-game-state { color: #8fc7a5; }
      #${PANEL} .vd-game.keys_missing .vd-game-state { color: #d96a60; }
      #${PANEL} .vd-play { min-height: 28px; padding: 4px 9px; border: 1px solid #46647a;
        border-radius: 6px; background: #2a475e; color: #e6eef4;
        font: 600 11px "Motiva Sans", Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-play:hover { background: #66c0f4; border-color: #66c0f4; color: #102331; }
      #${PANEL} .vd-more { font-size: 16px; line-height: 1; }
      #${PANEL} .vd-game-more { grid-column: 1 / -1; display: grid; grid-template-columns: 1fr 1fr;
        gap: 6px; padding: 7px 0 1px; border-top: 1px solid #2f3b47; }
      #${PANEL} .vd-game-more button { min-height: 28px; padding: 4px 7px;
        border: 1px solid #3b4a5a; border-radius: 6px; background: #202a35; color: #cdd9e3;
        font: 500 11px "Motiva Sans", Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-game-more button:hover { background: #2a475e; color: #fff; }
      #${PANEL} .vd-empty { padding: 18px 10px; border: 1px dashed #3b4a5a; border-radius: 8px;
        text-align: center; color: #8fa4b5; }
      #${PANEL} .vd-empty strong { display: block; margin-bottom: 4px; color: #dfe6ed; font-size: 12px; }
      #${PANEL} .vd-msg { padding: 8px 2px 0; font-size: 11px; line-height: 1.35; color: #9eb3c3; }
      #${PANEL} .vd-msg:empty { display: none; }
      #${PANEL} .vd-plugins { display: flex; flex-direction: column; gap: 7px; margin-top: 10px; }
      #${PANEL} .vd-plugins section { padding: 10px; border: 1px solid #2f3b47;
        border-radius: 8px; background: #1b232d; }
      #${PANEL} .vd-plugins section .vd-label { margin: 0 0 6px; color: #b8c4cf; }
      #${PANEL} .vd-plugins:empty { display: none; }
      #${PANEL} .vd-foot-row { display: flex; align-items: center; gap: 10px; margin-top: 12px;
        padding-top: 9px; border-top: 1px solid #2a313b; }
      #${PANEL} .vd-link { padding: 4px 0; border: 0; background: none;
        color: #9eb3c3; font: 500 11px "Motiva Sans", Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-link:hover { color: #66c0f4; }
      #${PANEL} .vd-ver { margin-left: auto; font-size: 10.5px; color: #8294a5;
        font-variant-numeric: tabular-nums; }
      #${PANEL} button:focus-visible { outline: 2px solid #8bd0f7; outline-offset: 2px; }
      #${PANEL} button:disabled { opacity: .55; cursor: default; }
      #${PANEL}-dash {
        position: fixed; inset: 0; z-index: 2147483647;
        display: none; flex-direction: column; background: #171a21;
        font-family: "Motiva Sans", "Segoe UI", Arial, sans-serif;
      }
      #${PANEL}-dash.open { display: flex; }
      #${PANEL}-dash .vd-dash-bar {
        display: flex; align-items: center; gap: 10px; height: 40px; flex: none;
        padding: 0 10px 0 14px; background: #171a21; border-bottom: 1px solid #2a313b;
        color: #dfe6ed; font-size: 12.5px; font-weight: 600;
      }
      #${PANEL}-dash .vd-dash-bar span { flex: 1; color: #8fa4b5; font-weight: 400; }
      #${PANEL}-dash .vd-dash-x {
        width: 27px; height: 27px; display: grid; place-items: center;
        border: 1px solid #3b4a5a; border-radius: 6px; background: transparent;
        color: #aab7c4; font: 500 15px/1 Arial, sans-serif; cursor: pointer;
      }
      #${PANEL}-dash .vd-dash-x:hover { background: #2a475e; color: #ffffff; }
      #${PANEL}-dash iframe { flex: 1; width: 100%; border: 0; background: #171a21; }
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

  // The page has no HTTP path to the app: Steam's CSP blocks loopback, and the
  // API now refuses unauthenticated writes. A flag read over CDP is enough.
  const toggle = () => {
    // The menu icon lives in the shell, but the store and library are separate
    // CEF windows layered above it. Opening locally would put the panel behind
    // those windows, so ask Rust which window is actually under the icon.
    const rect = item.getBoundingClientRect();
    window.__vdPanelRequest = {
      toggle: true,
      x: Math.round(rect.left),
      y: Math.round(rect.bottom),
      screen_x: Math.round(window.screenX),
      screen_y: Math.round(window.screenY),
    };
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


// A small card on Steam store pages for the game being viewed. It asks Rust for
// the app state over the same push channel the panel uses, so no CSP applies.
window.__vdAppPage = () => {
  const match = (window.location.pathname || "").match(/^\/app\/(\d+)/);
  if (!match) return false;
  if (!/(^|\.)store\.steampowered\.com$/.test(window.location.hostname || "")) return false;
  const appid = Number(match[1]);
  if (window.__vdAppDismissed === appid) return false;
  if (window.__vdAppWidgetId !== appid) {
    const old = document.getElementById("vd-app-card");
    if (old) old.remove();
    window.__vdPageState = null;
  }
  const stale = document.getElementById("vd-app-card");
  if (stale && stale.dataset.build !== (window.__vdBuild || "")) stale.remove();
  if (window.__vdAppWidgetId === appid && document.getElementById("vd-app-card")) return true;
  window.__vdAppWidgetId = appid;

  let style = document.getElementById("vd-app-style");
  if (!style) {
    style = document.createElement("style");
    style.id = "vd-app-style";
    style.textContent = `
      #vd-app-card { position: fixed; right: 18px; bottom: 64px; z-index: 2147483000;
        width: 308px; padding: 13px; border-radius: 8px; background: #171a21;
        border: 1px solid #3b4a5a; box-shadow: 0 16px 40px rgba(0,0,0,.48);
        font-family: "Motiva Sans", "Segoe UI", Arial, sans-serif; color: #dfe6ed; }
      #vd-app-card .t { display: flex; align-items: center; gap: 8px; font-size: 11px;
        font-weight: 600; color: #8fa4b5; }
      #vd-app-card .t span { flex: 1; }
      #vd-app-card .n { margin-top: 6px; font-size: 14px; font-weight: 600; color: #eef3f7; }
      #vd-app-card .s { margin-top: 4px; font-size: 11.5px; color: #a9b8c5; line-height: 1.4; }
      #vd-app-card .a { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; margin-top: 11px; }
      #vd-app-card button { box-sizing: border-box; min-height: 30px; padding: 6px 9px;
        border-radius: 7px; white-space: nowrap; line-height: 1.15;
        border: 1px solid #3b4a5a; background: #202a35; color: #d7e0e8;
        font: 600 11px "Motiva Sans", Arial, sans-serif; cursor: pointer;
        transition: background-color 120ms ease, border-color 120ms ease, color 120ms ease; }
      #vd-app-card button.primary { background: #66c0f4; border-color: #66c0f4; color: #102331; }
      #vd-app-card button:hover { border-color: #57809e; background: #2a475e; color: #ffffff; }
      #vd-app-card button.primary:hover { background: #8bd0f7; border-color: #8bd0f7; color: #102331; }
      #vd-app-card button[data-want="close"] { position: absolute; right: 8px; top: 8px;
        width: 25px; min-height: 25px; padding: 0; border: 0; background: transparent;
        color: #8fa4b5; font-size: 15px; }
      #vd-app-card button[data-want="close"]:hover { color: #ffffff; background: #2a475e; }
      #vd-app-card button[disabled] { opacity: .55; cursor: default; }
      #vd-app-card [hidden] { display: none !important; }
    `;
    document.head.appendChild(style);
  }

  let card = document.getElementById("vd-app-card");
  if (card && card.dataset.build !== (window.__vdBuild || "")) {
    card.remove();
    card = null;
  }
  if (!card) {
    card = document.createElement("div");
    card.id = "vd-app-card";
    card.dataset.build = window.__vdBuild || "";
    card.innerHTML = '<div class="t"><span>ExxaTools</span></div>'
      + '<div class="n" data-name>App ' + appid + '</div>'
      + '<div class="s" data-status>Looking up this game…</div>'
      + '<div class="a"><button class="primary" data-want="add" hidden>Add to library</button>'
      + '<button data-want="launch" hidden>Launch</button>'
      + '<button data-want="sync" hidden>Sync manifests</button>'
      + '<button data-want="fix" hidden title="Repair update flags and restart Steam">Fix update</button>'
      + '<button data-want="settings" hidden>Setup source</button>'
      + '<button data-want="dashboard">Dashboard</button>'
      + '<button data-want="close" title="Hide">×</button></div>';
    card.addEventListener("click", (event) => {
      const button = event.target.closest("button[data-want]");
      if (!button) return;
      const want = button.dataset.want;
      if (want === "close") {
        window.__vdAppDismissed = appid;
        card.remove();
        window.__vdAppWidgetId = null;
        return;
      }
      window.__vdPageState = null;
      window.__vdPageRequest = { appid: appid, want: want };
      card.querySelectorAll("button").forEach((node) => { node.disabled = true; });
    });
    document.body.appendChild(card);
  }

  const render = (state) => {
    if (!state || Number(state.appid) !== appid) return;
    const name = card.querySelector("[data-name]");
    const status = card.querySelector("[data-status]");
    const add = card.querySelector('[data-want="add"]');
    const sync = card.querySelector('[data-want="sync"]');
    if (state.name) name.textContent = state.name;
    status.textContent = state.status || "";
    add.hidden = !state.can_add;
    sync.hidden = !state.can_sync;
    const setup = card.querySelector('[data-want="settings"]');
    if (setup) setup.hidden = !state.can_setup;
    const launch = card.querySelector('[data-want="launch"]');
    if (launch) launch.hidden = !state.can_launch;
    const fix = card.querySelector('[data-want="fix"]');
    if (fix) fix.hidden = !state.can_fix;
    card.querySelectorAll("button").forEach((node) => { node.disabled = false; });
  };

  window.__vdAppCardRender = render;
  if (!window.__vdAppPoll) {
    window.__vdAppPoll = setInterval(() => {
      const state = window.__vdPageState;
      if (state && window.__vdAppCardRender) window.__vdAppCardRender(state);
    }, 400);
  }
  window.__vdPageState = null;
  window.__vdPageRequest = { appid: appid, want: "probe" };
  return true;
};

if (!window.__vdAppWatcher) {
  window.__vdAppWatcher = setInterval(() => { try { window.__vdAppPage(); } catch (error) { } }, 1200);
}


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
