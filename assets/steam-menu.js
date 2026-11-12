window.__vdGlyphs = __GLYPHS__;

// Steam runs several CEF windows: the shell that owns the menu bar, and separate
// windows for the store, the library and Community. The panel is built in
// whichever window the user is looking at, so it renders inside Steam instead of
// floating above the desktop.
window.__vdPanel = (opts) => {
  const API = "__API__";
  const PANEL = "__PANEL__";
  const glyph = (name) => (window.__vdGlyphs || {})[name] || "";

  const openDashboard = () => {
    let shell = document.getElementById(PANEL + "-dash");
    if (!shell) {
      shell = document.createElement("div");
      shell.id = PANEL + "-dash";
      shell.innerHTML = '<div class="vd-dash-bar">ExxaTools<span>dashboard</span>'
        + '<button class="vd-dash-x" title="Close">×</button></div>'
        + '<iframe title="ExxaTools dashboard"></iframe>';
      shell.querySelector("iframe").src = API + "/";
      shell.querySelector(".vd-dash-x").addEventListener("click", () => shell.classList.remove("open"));
      document.body.appendChild(shell);
    }
    shell.classList.add("open");
    return true;
  };
  window.__vdOpenDashboard = openDashboard;

  const build = () => {
    if (!document.body) return null;
    const style = document.createElement("style");
    style.id = PANEL + "-style";
    style.textContent = `
      #${PANEL} {
        position: fixed; z-index: 2147483647; width: 320px;
        background: #141922; border: 1px solid #242E3A; border-radius: 14px;
        box-shadow: 0 20px 48px rgba(0,0,0,.55);
        font-family: Montserrat, "Motiva Sans", Arial, sans-serif;
        color: #E9EDF2; display: none;
        max-height: calc(100vh - 16px);
        overflow-y: auto;
        overscroll-behavior: contain;
      }
      #${PANEL}.open { display: block; }
      #${PANEL} .vd-head { padding: 14px 14px 11px; border-bottom: 1px solid #1C2430; }
      #${PANEL} .vd-brand { display: flex; align-items: center; gap: 8px; }
      #${PANEL} .vd-name { flex: 1; font-size: 13.5px; font-weight: 650; letter-spacing: .005em; }
      #${PANEL} .vd-x {
        width: 28px; height: 28px; flex: none; display: grid; place-items: center;
        border: 0; border-radius: 7px; background: transparent;
        color: #A9B4C3; font: 500 16px/1 Montserrat, Arial, sans-serif; cursor: pointer;
      }
      #${PANEL} .vd-x:hover { background: #1C2430; color: #E9EDF2; }
      #${PANEL} .vd-state { display: flex; align-items: center; gap: 7px; margin-top: 8px;
        font-size: 11.5px; color: #B7C1CD; }
      #${PANEL} .vd-ver { margin-left: auto; font-size: 10.5px; color: #8A95A3;
        font-variant-numeric: tabular-nums; }
      #${PANEL} .vd-dot { width: 7px; height: 7px; border-radius: 50%; background: #6B7686; flex: none; }
      #${PANEL} .vd-state.ok .vd-dot { background: #58B77E; box-shadow: 0 0 0 3px rgba(88,183,126,.14); }
      #${PANEL} .vd-state.warn .vd-dot { background: #D9A15A; box-shadow: 0 0 0 3px rgba(217,161,90,.14); }
      #${PANEL} .vd-state.bad .vd-dot { background: #D9655C; box-shadow: 0 0 0 3px rgba(217,101,92,.14); }
      #${PANEL} .vd-body { padding: 4px 8px 8px; }
      #${PANEL} .vd-label { margin: 20px 6px 8px; padding: 0;
        font-size: 10.5px; font-weight: 700; letter-spacing: .07em;
        text-transform: uppercase; color: #A9B4C3; }
      #${PANEL} .vd-body > .vd-label:first-of-type { margin-top: 10px; }
      #${PANEL} .vd-alert[hidden] { display: none; }
      #${PANEL} .vd-alert { display: flex; align-items: center; gap: 8px; margin: 6px 6px 2px;
        padding: 8px 10px; border: 1px solid rgba(237,180,95,.28); border-radius: 8px;
        background: rgba(237,180,95,.06); font-size: 11.5px; color: #E9C48A; }
      #${PANEL} .vd-alert-text { flex: 1; line-height: 1.35; }
      #${PANEL} .vd-plans { display: flex; flex-direction: column; gap: 1px; }
      #${PANEL} .vd-plan { display: flex; flex-direction: column; align-items: stretch; gap: 8px;
        min-height: 36px; padding: 8px; border-radius: 9px; font-size: 12.5px; color: #B7C1CD; }
      #${PANEL} .vd-plan:hover { background: #171E28; }
      #${PANEL} .vd-plan-top { display: flex; align-items: center; gap: 8px; min-width: 0; }
      #${PANEL} .vd-plan-top b { flex: 1; min-width: 0; font-weight: 550; color: #E9EDF2;
        overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
      #${PANEL} .vd-plan-actions { display: flex; gap: 6px; justify-content: flex-start; flex-wrap: wrap; }
      #${PANEL} .vd-plan-actions .vd-mini { height: 24px; padding: 0 9px; font-size: 10.5px; }
      #${PANEL} .vd-flag { font: 600 10px Montserrat, Arial, sans-serif; padding: 2px 7px;
        border: 0; border-radius: 5px; background: #1D2632; color: #93A0AE; }
      #${PANEL} .vd-flag.ok { color: #83D3A6; background: #16261F; }
      #${PANEL} .vd-flag.warn { color: #E2B274; background: #2A2116; }
      #${PANEL} .vd-mini { height: 26px; padding: 0 11px; border: 1px solid #39485A;
        border-radius: 7px; background: #222D3A; color: #E2E9F1;
        font: 600 11px Montserrat, Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-mini:hover { background: #1F2935; border-color: #35414F; color: #EEF3F8; }
      #${PANEL} .vd-empty { display: flex; flex-direction: column; align-items: flex-start;
        gap: 5px; padding: 2px 8px 2px; font-size: 12px; color: #8A95A3; }
      #${PANEL} .vd-empty span { font-size: 12.5px; font-weight: 600; color: #C3CCD7; }
      #${PANEL} .vd-empty .vd-hint { font-size: 11.5px; font-weight: 400; color: #9AA6B5; }
      #${PANEL} .vd-primary { height: 28px; margin-top: 4px; padding: 0 11px;
        border: 0; border-radius: 7px; background: #EDB45F; color: #171C24;
        font: 600 11.5px Montserrat, Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-primary:hover { filter: brightness(1.06); }
      #${PANEL} .vd-primary:active { filter: brightness(.94); }
      #${PANEL} .vd-link { align-self: flex-start; padding: 0; border: 0; background: none;
        color: #EDB45F; font: 500 11.5px Montserrat, Arial, sans-serif; cursor: pointer; }
      #${PANEL} .vd-link:hover { text-decoration: underline; }
      #${PANEL} .vd-actions { display: flex; flex-direction: column; gap: 1px; }
      #${PANEL} .vd-item { display: flex; align-items: center; width: 100%; min-height: 36px;
        padding: 7px 8px; gap: 10px; background: transparent; border: 0;
        border-radius: 8px; color: #C3CCD7;
        font: 500 12.5px Montserrat, "Motiva Sans", Arial, sans-serif;
        text-align: left; cursor: pointer; }
      #${PANEL} .vd-item:hover { background: #1A212B; color: #EEF3F8; }
      #${PANEL} .vd-item:focus-visible, #${PANEL} .vd-mini:focus-visible,
      #${PANEL} .vd-primary:focus-visible, #${PANEL} .vd-x:focus-visible {
        outline: 2px solid #EDB45F; outline-offset: 2px; }
      #${PANEL} .vd-item svg { width: 16px; height: 16px; flex: none; opacity: .7; }
      #${PANEL} .vd-item:hover svg { opacity: .9; }
      #${PANEL} .vd-foot { height: 1px; margin: 14px 6px 4px; background: #1C2430; }
      #${PANEL} .vd-about { margin-top: 2px; color: #9AA6B5; }
      #${PANEL} .vd-msg { padding: 8px 8px 0; font-size: 11.5px; color: #9AA6B5; }
      #${PANEL} .vd-msg:empty { display: none; }
      #${PANEL} .vd-plugins { display: flex; flex-direction: column; gap: 8px; }
      #${PANEL} .vd-plugins section { padding: 10px 10px 10px; border: 1px solid #202834;
        border-radius: 10px; background: #171D26; }
      #${PANEL} .vd-plugins section .vd-label { margin: 0 0 6px; padding: 0;
        font-size: 11.5px; font-weight: 650; letter-spacing: 0; text-transform: none;
        color: #C3CCD7; }
      #${PANEL} .vd-plugins:empty { display: none; }
      #${PANEL} .vd-plugins .vd-label { margin: 0 0 4px; padding: 0; font-weight: 600; }
      #${PANEL}-dash {
        position: fixed; inset: 0; z-index: 2147483647;
        display: none; flex-direction: column; background: #0c0f13;
        font-family: Montserrat, "Motiva Sans", Arial, sans-serif;
      }
      #${PANEL}-dash.open { display: flex; }
      #${PANEL}-dash .vd-dash-bar {
        display: flex; align-items: center; gap: 10px; height: 38px; flex: none;
        padding: 0 10px 0 14px; background: #141922; border-bottom: 1px solid #242E3A;
        color: #E9EDF2; font-size: 12.5px; font-weight: 600;
      }
      #${PANEL}-dash .vd-dash-bar span { flex: 1; color: #8A95A3; font-weight: 400; }
      #${PANEL}-dash .vd-dash-x {
        width: 26px; height: 26px; display: grid; place-items: center;
        border: 1px solid #2A3543; border-radius: 7px; background: transparent;
        color: #8A95A3; font: 500 14px/1 Montserrat, Arial, sans-serif; cursor: pointer;
      }
      #${PANEL}-dash .vd-dash-x:hover { background: #1C2430; color: #EFF3F8; }
      #${PANEL}-dash iframe { flex: 1; width: 100%; border: 0; background: #0c0f13; }
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
        <div class="vd-brand"><span class="vd-name">ExxaTools</span><button class="vd-x" data-act="close" title="Close">×</button></div>
        <div class="vd-state" id="vd-state"><span class="vd-dot"></span><span id="vd-state-text">checking engine</span><span class="vd-ver" id="vd-ver"></span></div>
      </div>
      <div class="vd-body">
        <div class="vd-alert" id="vd-alert" hidden>
          <span class="vd-alert-text" id="vd-alert-text"></span>
          <button class="vd-mini" data-act="keys">Fix</button>
          <button class="vd-mini" data-act="dismiss" title="Hide until it happens again">×</button>
        </div>
        <div class="vd-label">Plans</div>
        <div class="vd-plans" id="vd-plans"><div class="vd-empty"><span>No plans yet</span><span class="vd-hint">Planned games appear here.</span><button class="vd-primary" data-act="open">Add a game</button></div></div>
        <div class="vd-label">Actions</div>
        <div class="vd-actions">
          <button class="vd-item" data-act="open">ICON_GLYPH_APP Open dashboard</button>
          <button class="vd-item" data-act="restart">ICON_GLYPH_RESTART Restart Steam</button>
          <button class="vd-item" data-act="provider">ICON_GLYPH_PROVIDER Install provider</button>
        </div>
        <div class="vd-label vd-label-plugins" hidden>Plugins</div>
        <div class="vd-plugins" id="vd-plugins-host"></div>
        <div class="vd-msg" id="vd-msg"></div>
        <div class="vd-foot"></div>
        <button class="vd-item vd-about" data-act="about">ICON_GLYPH_ABOUT About</button>
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
        version.textContent = engine.version ? `engine ${engine.version}` : "—";
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
        box.innerHTML = '<div class="vd-empty"><span>No plans yet</span>'
          + '<span class="vd-hint">Planned games appear here.</span>'
          + '<button class="vd-primary" data-act="open">Add a game</button></div>';
      } else {
        box.replaceChildren(...plans.slice(0, 8).map((plan) => {
          const missing = (plan.missing_keys || []).length;
          const flag = plan.installed
            ? (missing ? { text: "keys", kind: "warn" } : { text: "ready", kind: "ok" })
            : { text: "plan", kind: "" };
          const row = document.createElement("div");
          row.className = "vd-plan";
          const top = document.createElement("div");
          top.className = "vd-plan-top";
          const actions = document.createElement("div");
          actions.className = "vd-plan-actions";
          const name = document.createElement("b");
          name.textContent = plan.name || String(plan.appid);
          const chip = document.createElement("span");
          chip.className = "vd-flag " + flag.kind;
          chip.textContent = flag.text;
          const install = document.createElement("button");
          install.className = "vd-mini";
          install.dataset.act = "install:" + plan.appid;
          install.textContent = plan.installed ? "Sync" : "Install";
          install.title = "Write this plan into the Vapor Forge scripts folder";
          const rebuild = document.createElement("button");
          rebuild.className = "vd-mini";
          rebuild.dataset.act = "rebuild:" + plan.appid;
          rebuild.textContent = "Rebuild";
          rebuild.title = "Rebuild this plan from the current catalog";
          const flagOn = String(plan.launch_options || "").split(/\s+/).includes("-onlinefix");
          const flagButton = document.createElement("button");
          flagButton.className = "vd-mini";
          flagButton.dataset.act = "launch:" + plan.appid;
          flagButton.textContent = flagOn ? "Online fix on" : "Online fix";
          flagButton.title = "Toggle the -onlinefix launch option";
          top.append(name, chip);
          actions.append(install, rebuild, flagButton);
          row.append(top, actions);
          return row;
        }));
      }
      window.__vdLastState = state;
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

    panel.addEventListener("click", (event) => {
      const button = event.target.closest("[data-act]");
      if (!button) return;
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
        openDashboard();
        msg("");
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

  // The page has no HTTP path to the app: Steam's CSP blocks loopback, and the
  // API now refuses unauthenticated writes. A flag read over CDP is enough.
  const toggle = () => {
    const rect = item.getBoundingClientRect();
    window.__vdWantPanel = {
      toggle: true,
      x: Math.round(rect.left),
      y: Math.round(rect.bottom),
      screen_x: Math.round(window.screenX || 0),
      screen_y: Math.round(window.screenY || 0),
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
      const shell = document.getElementById(PANEL + "-dash");
      if (shell && shell.classList.contains("open")) { shell.classList.remove("open"); return; }
      window.__vdWantPanel = { close: true };
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