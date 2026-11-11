window.__vdGlyphs = __GLYPHS__;

// Steam draws the store, the library and Community in their own windows above
// the main frame, so the panel is a separate always-on-top window (overlay.py).
// This script owns only the icon next to Help and asks the server to show it.
window.__vdInject = () => {
  const API = "__API__";
  const ROOT = "__ROOT__";
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
  if (!wrapper) return "no-anchor";

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
    })
      .then((response) => response.json())
      .then((data) => { window.__vdOpen = Boolean(data.open); })
      .catch(() => {});
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
    if (event.key === "Escape" && window.__vdOpen) {
      fetch(API + "/api/steamui/panel", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ open: false }),
      }).catch(() => {});
      window.__vdOpen = false;
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
      if (!document.getElementById("__ROOT__") && document.body) {
        try { window.__vdInject(); } catch (error) { /* Steam is mid-render */ }
      }
    }, 250);
  });
  window.__vdWatcher.observe(document.body, { childList: true, subtree: true });
}

window.__vdInject();