"""Inject our icon and menu into the Steam client's own menu bar.

Steam's UI is Chromium, and with ``-cef-enable-debugging`` it exposes the CDP
socket on localhost. That is the only stable way to put something next to
"Help" without patching Steam's bundles: clone the Help menu item, swap its
text for our mark, and hang a panel off it that talks to the local API.
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from . import cdp, steamctl, vfsys

DEBUG_PORT = 8080
FLAG = "-cef-enable-debugging"
MAIN_TARGET_PREFIX = "about:blank?createflags=18"
ROOT_ID = "vd-menu-root"
PANEL_ID = "vd-menu-panel"
ICON_ID = "vd-menu-icon"
API_BASE = "http://127.0.0.1:8765"

ICON_SVG = (
    '<svg viewBox="0 0 24 24" fill="none" stroke="#E6B45C" stroke-width="1.9" '
    'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'
    '<path d="M4 6.5 7.5 18 11 6.5"/><path d="M13.5 6.5h2.4a5.75 5.75 0 0 1 0 11.5h-2.4z"/>'
    "</svg>"
)

SCRIPT = r"""
window.__vdGlyphs = __GLYPHS__;
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
  item.title = "ExxaTools";
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
      position: fixed; z-index: 2147483000; width: 264px; padding: 6px;
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
    #${PANEL} .vd-item { display: flex; align-items: center; width: 100%; min-height: 34px;
      padding: 7px 10px; gap: 9px; background: transparent; border: 1px solid transparent;
      border-radius: 9px; color: #9DA8B7; font: 500 12.5px Montserrat, "Motiva Sans", Arial, sans-serif;
      text-align: left; cursor: pointer; }
    #${PANEL} .vd-item:hover { background: #1A212B; color: #E9EDF2; border-color: transparent; }
    #${PANEL} .vd-item svg { width: 15px; height: 15px; flex: none; opacity: .9; }
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
    if (!engine) { text.textContent = "ExxaTools is not answering"; state.className = "vd-state bad"; return; }
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

  const refresh = async () => {
    try {
      const response = await fetch(API + "/api/engine", { cache: "no-store" });
      setState(await response.json());
    } catch (error) {
      setState(null);
    }
  };

  const place = () => {
    const rect = item.getBoundingClientRect();
    const width = 264;
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

  item.addEventListener("click", (event) => {
    event.stopPropagation();
    if (panel.classList.contains("open")) close(); else open();
  });
  document.addEventListener("click", (event) => {
    if (!panel.contains(event.target) && !item.contains(event.target)) close();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") close();
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
        msg(data.error ? data.error : "provider script written");
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
"""

GLYPHS = {
    "app": '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.5 3.5h11v9h-11z"/><path d="M2.5 6.5h11"/></svg>',
    "restart": '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.8 8a5.2 5.2 0 1 0 1.9-4"/><path d="M2.6 2.6v3.2h3.2"/></svg>',
    "provider": '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8" cy="8" r="2.1"/><path d="M8 1.5v3M8 11.5v3M1.5 8h3M11.5 8h3"/></svg>',
    "about": '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="8" cy="8" r="6"/><path d="M8 7.2v4M8 5.1v.2"/></svg>',
}

DESKTOP_MARK = "X-Vapor-Forge-Override=true"


class SteamUiError(RuntimeError):
    pass


def debug_ready() -> bool:
    return cdp.version(DEBUG_PORT) is not None


def main_target() -> dict | None:
    for target in cdp.targets(DEBUG_PORT):
        if str(target.get("url", "")).startswith(MAIN_TARGET_PREFIX):
            return target
    for target in cdp.targets(DEBUG_PORT):
        if target.get("title") == "Steam" and target.get("type") == "page":
            return target
    return None


def _script() -> str:
    import json

    script = SCRIPT
    script = script.replace("__GLYPHS__", json.dumps(GLYPHS))
    script = script.replace("__API__", API_BASE)
    script = script.replace("__ROOT__", ROOT_ID)
    script = script.replace("__PANEL__", PANEL_ID)
    script = script.replace("__ICON__", ICON_ID)
    return script.replace("ICON_ICON", json.dumps(ICON_SVG))


def status() -> dict:
    running = steamctl.is_running()
    ready = debug_ready() if running else False
    target = main_target() if ready else None
    injected = False
    if target is not None:
        try:
            injected = bool(
                cdp.evaluate(
                    target["webSocketDebuggerUrl"],
                    f"Boolean(document.getElementById({ROOT_ID!r}))",
                    timeout=6.0,
                )
            )
        except cdp.CdpError as error:
            return {
                "steam_running": running,
                "debug_port": DEBUG_PORT,
                "debug_ready": ready,
                "injected": False,
                "error": str(error),
            }
    return {
        "steam_running": running,
        "debug_port": DEBUG_PORT,
        "debug_ready": ready,
        "target": (target or {}).get("url"),
        "injected": injected,
        "flag": FLAG,
    }


def inject() -> dict:
    if not steamctl.is_running():
        raise SteamUiError("Steam is not running")
    if not debug_ready():
        raise SteamUiError(
            "Steam has no debugging port: press Enable, which restarts Steam with "
            f"{FLAG}"
        )
    target = main_target()
    if target is None:
        raise SteamUiError("the Steam main window is not available over CDP")
    result = cdp.evaluate(target["webSocketDebuggerUrl"], _script(), timeout=20.0)
    if result not in ("injected", "present"):
        raise SteamUiError(f"injection failed: {result}")
    return {"injected": True, "result": result, **status()}


def remove() -> dict:
    target = main_target()
    if target is None:
        return {"injected": False, "removed": False}
    removed = cdp.evaluate(
        target["webSocketDebuggerUrl"],
        f"""(() => {{
          const root = document.getElementById({ROOT_ID!r});
          const panel = document.getElementById({PANEL_ID!r});
          const style = document.getElementById({ROOT_ID!r} + "-style");
          if (root) root.remove();
          if (panel) panel.remove();
          if (style) style.remove();
          return {bool(True)!r};
        }})()""",
        timeout=10.0,
    )
    return {"removed": bool(removed), **status()}


def enable_debug() -> dict:
    """Persist the debugging flag in the launcher, then restart Steam."""
    patched = []
    for path in (desktop_override(), Path("/usr/share/applications/steam.desktop")):
        if path is None or not path.is_file():
            continue
        try:
            text = path.read_text("utf-8", "replace")
        except OSError:
            continue
        if FLAG in text:
            patched.append(str(path))
            continue
        if "Exec=" not in text:
            continue
        if path.name == "steam.desktop" and not path.read_text("utf-8", "replace").count(DESKTOP_MARK):
            continue
        new = text.replace("Exec=", f"Exec={FLAG} ", 1)
        try:
            path.write_text(new, "utf-8")
            patched.append(str(path))
        except OSError:
            continue
    result = steamctl.restart([FLAG])
    return {"patched": patched, "restarted": result, "flag": FLAG}


def desktop_override() -> Path | None:
    from .desktop import DATA_HOME

    path = DATA_HOME / "applications" / "steam.desktop"
    return path if path.is_file() else None


def ensure_running() -> dict:
    """Best-effort: make the injected item exist right now."""
    info = status()
    if not info.get("debug_ready"):
        return info
    if info.get("injected"):
        return info
    return inject()