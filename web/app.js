"use strict";

const $ = (sel) => document.querySelector(sel);

const CDN = "https://cdn.cloudflare.steamstatic.com/steam/apps/";

const state = {
  view: "library",
  status: null,
  engine: null,
  provider: null,
  hubcap: null,
  steamui: null,
  app: null,
  branch: "public",
  selected: new Set(),
  dlc: new Set(),
  step: 0,
  searchSeq: 0,
};

// ---------------------------------------------------------------- transport

async function api(path, { method = "GET", body } = {}) {
  const response = await fetch(`/api/${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  let data = {};
  try {
    data = await response.json();
  } catch {
    data = { error: `unreadable response (${response.status})` };
  }
  if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);
  return data;
}

let toastTimer = null;
function toast(message, kind = "") {
  const node = $("#toast");
  node.textContent = message;
  node.className = `toast ${kind}`.trim();
  node.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { node.hidden = true; }, kind === "bad" ? 7000 : 3800);
}

function h(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value == null || value === false) continue;
    if (key === "class") node.className = value;
    else if (key === "text") node.textContent = value;
    else if (key === "dataset") Object.assign(node.dataset, value);
    else if (key.startsWith("on") && typeof value === "function") node.addEventListener(key.slice(2), value);
    else node.setAttribute(key, value === true ? "" : value);
  }
  for (const child of children.flat()) {
    if (child == null || child === false) continue;
    node.append(child.nodeType ? child : document.createTextNode(String(child)));
  }
  return node;
}

function fmtSize(bytes) {
  if (!bytes) return "—";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let value = Number(bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function artUrl(appid, size = "library_600x900") {
  return `${CDN}${appid}/${size}.jpg`;
}

// -------------------------------------------------------------------- views

function switchView(name) {
  state.view = name;
  document.querySelectorAll(".view").forEach((view) => {
    view.classList.toggle("is-active", view.id === `view-${name}`);
  });
  document.querySelectorAll(".nav-item").forEach((item) => {
    item.classList.toggle("is-active", item.dataset.view === name);
  });
  if (name === "system") loadSystem();
}

// ------------------------------------------------------------------ library

async function refreshStatus() {
  try {
    state.status = await api("status");
  } catch (error) {
    toast(`status: ${error.message}`, "bad");
    return;
  }
  renderPlans();
  loadEngine();
}

function renderPlans() {
  const plans = state.status?.apps || [];
  const head = $("#plansHead");
  const box = $("#plans");
  head.hidden = !plans.length;
  box.replaceChildren(
    ...plans.map((plan) => {
      const missing = (plan.missing_keys || []).length;
      const badge = plan.installed
        ? missing
          ? { text: "needs keys", kind: "warn" }
          : { text: "installed", kind: "ok" }
        : { text: "planned", kind: "idle" };
      return h("button", { class: "card", type: "button", onclick: () => openApp(plan.appid, plan.branch) },
        h("div", { class: "card-art empty", style: `background-image:url('${artUrl(plan.appid)}')` },
          h("span", { class: `card-badge ${badge.kind}`, text: badge.text })
        ),
        h("div", { class: "card-body" },
          h("span", { class: "card-name", text: plan.name || String(plan.appid) }),
          h("span", { class: "card-sub", text: missing ? `${plan.depot_count} depots · ${missing} keyless` : `${plan.depot_count} depots` })
        )
      );
    })
  );
}

const DOCTOR_ACTIONS = {
  engine_install: () => api("engine/install", { method: "POST", body: {} }),
  restart_steam: () => api("steam/restart", { method: "POST", body: {} }),
  provider_install: () => api("provider", { method: "POST", body: {} }),
  steamui_enable: () => api("steamui", { method: "POST", body: { action: "enable" } }),
  watch: () => api("watch", { method: "POST", body: { apply: true } }),
};

const DOCTOR_LABELS = {
  engine_install: "Install",
  restart_steam: "Restart",
  provider_install: "Install",
  steamui_enable: "Enable",
  watch: "Fix",
};

async function loadDoctor() {
  const box = $("#doctorList");
  const hint = $("#doctorHint");
  if (!box) return;
  try {
    const data = await api("doctor");
    hint.textContent = data.ok
      ? data.warnings
        ? `No blocking problems, ${data.warnings} warning(s).`
        : "Everything checks out."
      : "Something blocks a download, see the red rows.";
    box.replaceChildren(
      ...(data.items || []).map((item) =>
        h("div", { class: `doctor-row ${item.status}`, title: item.hint || "" },
          h("span", { class: "doctor-dot" }),
          h("div", { class: "doctor-main" },
            h("span", { class: "doctor-label", text: item.label }),
            h("span", { class: "doctor-detail", text: item.detail || "" })
          ),
          item.action
            ? h("button", {
                class: "btn small ghost doctor-action",
                text: DOCTOR_LABELS[item.action] || "Fix",
                onclick: async () => {
                  if (!DOCTOR_ACTIONS[item.action]) return;
                  try {
                    await DOCTOR_ACTIONS[item.action]();
                    toast(`${item.label}: done`, "ok");
                  } catch (error) {
                    toast(`${item.label}: ${error.message}`, "bad");
                  }
                  loadDoctor();
                  refreshStatus();
                },
              })
            : null
        )
      )
    );
  } catch (error) {
    box.replaceChildren(h("p", { class: "hint", text: `doctor: ${error.message}` }));
  }
}

async function importLua(text, name) {
  try {
    const result = await api("import", { method: "POST", body: { text } });
    const summary = result.summary || {};
    toast(`${name || "lua"}: ${summary.keys || 0} key(s), ${summary.depots || 0} depot(s)`, "ok");
    refreshStatus();
    loadDoctor();
  } catch (error) {
    toast(`${name || "lua"}: ${error.message}`, "bad");
  }
}

async function loadDiscover() {
  const box = $("#discover");
  box.replaceChildren(h("p", { class: "hint", text: "loading picks…" }));
  try {
    const data = await api("discover?limit=18");
    const items = data.items || [];
    box.replaceChildren(...items.map(resultCard));
  } catch (error) {
    box.replaceChildren(h("p", { class: "hint", text: `discover: ${error.message}` }));
  }
}

function showDiscover(visible) {
  $("#discoverHead").hidden = !visible;
  $("#discover").hidden = !visible;
}

let searchTimer = null;
async function runSearch(term) {
  const query = term.trim();
  const box = $("#results");
  if (!query) {
    box.replaceChildren();
    showDiscover(true);
    return;
  }
  showDiscover(false);
  const seq = ++state.searchSeq;
  box.replaceChildren(h("p", { class: "hint", text: "searching…" }));
  try {
    const data = await api(`search?q=${encodeURIComponent(query)}`);
    if (seq !== state.searchSeq) return;
    const items = data.items || [];
    if (!items.length) {
      box.replaceChildren(h("p", { class: "hint", text: "Nothing on the store for that. An app id still works." }));
      return;
    }
    box.replaceChildren(...items.slice(0, 24).map(resultCard));
  } catch (error) {
    if (seq !== state.searchSeq) return;
    box.replaceChildren(h("p", { class: "hint", text: `search failed: ${error.message}` }));
  }
}

function resultCard(item) {
  const art = h("div", { class: "card-art" });
  const fallback = h("div", { class: "card-art empty", text: (item.name || "?").slice(0, 1).toUpperCase() });
  const sizes = ["library_600x900", "header", "capsule_231x87", "logo"];
  let index = 0;
  const img = new Image();
  img.onload = () => { art.style.backgroundImage = `url('${img.src}')`; };
  img.onerror = () => {
    index += 1;
    if (index < sizes.length) img.src = artUrl(item.appid, sizes[index]);
    else art.replaceWith(fallback);
  };
  img.src = artUrl(item.appid, sizes[0]);
  return h("button", { class: "card", type: "button", onclick: () => openApp(item.appid) },
    art,
    h("div", { class: "card-body" },
      h("span", { class: "card-name", text: item.name }),
      h("span", { class: "card-sub", text: item.price || String(item.appid) })
    )
  );
}

// ------------------------------------------------------------------ app view

async function openApp(appid, branch) {
  switchView("app");
  $("#appName").textContent = "loading…";
  $("#appMeta").textContent = "";
  $("#depotList").replaceChildren(h("p", { class: "hint", text: "reading app info…" }));
  try {
    const target = branch || state.branch || "public";
    const app = await api(`app?appid=${appid}&branch=${encodeURIComponent(target)}`);
    state.app = app;
    state.branch = app.branch;
    state.selected = new Set((app.plan?.depots || []).map((depot) => depot.depot_id));
    state.dlc = new Set((app.plan?.dlc || []).map(Number));
    state.step = 0;
    if (!state.selected.size) {
      for (const depot of app.depots) {
        const preferred = state.status?.settings?.default_os || "linux";
        const platform = !depot.oslist.length || depot.oslist.includes(preferred);
        if (platform && !depot.dlc_appid) state.selected.add(depot.depot_id);
      }
    }
    renderApp();
    loadGuard(appid);
  } catch (error) {
    $("#appName").textContent = "could not open that app";
    $("#depotList").replaceChildren(h("p", { class: "hint", text: error.message }));
    toast(`app ${appid}: ${error.message}`, "bad");
  }
}

function renderApp() {
  const app = state.app;
  if (!app) return;
  const total = app.depots.reduce((sum, depot) => sum + (depot.size || 0), 0);
  const dlcNames = app.dlc_names || [];
  $("#appName").textContent = app.name;
  $("#appMeta").textContent = [
    app.appid,
    app.type,
    app.developers,
    `${app.depots.length} depots`,
    total ? fmtSize(total) : null,
    dlcNames.length
      ? `${dlcNames.length} dlc: ${dlcNames.slice(0, 3).map((item) => item.name).join(", ")}${dlcNames.length > 3 ? "…" : ""}`
      : null,
  ].filter(Boolean).join(" · ");

  setHero(app.appid);

  const select = $("#branchSelect");
  select.replaceChildren(
    ...app.branches.map((branch) =>
      h("option", {
        value: branch.name,
        text: branch.name.charAt(0).toUpperCase() + branch.name.slice(1),
        selected: branch.name === state.branch,
      })
    )
  );
  if (!app.branches.length) select.append(h("option", { value: "public", text: "public" }));
  select.value = state.branch;

  renderDepots();
  renderSteps();
  renderDlc();
  const platform = state.status?.settings?.default_os || "linux";
  document.querySelectorAll("[data-select]").forEach((item) => {
    item.classList.toggle("is-active", item.dataset.select === platform);
  });
  loadScript(app.appid);
}

function setHero(appid) {
  const art = $("#heroArt");
  art.style.backgroundImage = "";
  const attempt = (size) => {
    const img = new Image();
    img.onload = () => { art.style.backgroundImage = `url('${img.src}')`; };
    img.onerror = () => {
      if (size === "library_hero") attempt("header");
    };
    img.src = artUrl(appid, size);
  };
  attempt("library_hero");
}

function renderSteps() {
  const done = [true, true, state.selected.size > 0, Boolean(state.app?.installed)];
  document.querySelectorAll("#steps .step").forEach((button) => {
    const index = Number(button.dataset.step);
    button.classList.toggle("is-active", index === state.step);
    button.classList.toggle("done", done[index] && index !== state.step);
  });
}

function renderDlc() {
  const list = $("#dlcList");
  const hint = $("#dlcHint");
  const count = $("#dlcCount");
  if (!list) return;
  const items = state.app?.dlc_names || [];
  count.textContent = items.length ? String(items.length) : "";
  if (!items.length) {
    list.replaceChildren(h("p", { class: "hint", text: "This app has no DLC entries." }));
    hint.textContent = "";
    return;
  }
  list.replaceChildren(
    ...items.map((item) =>
      h("label", { class: "dlc-row" },
        h("input", {
          type: "checkbox",
          checked: state.dlc.has(item.appid),
          onchange: (event) => {
            if (event.target.checked) state.dlc.add(item.appid);
            else state.dlc.delete(item.appid);
            renderSteps();
            const hintNode = $("#dlcHint");
            if (hintNode) hintNode.textContent = `${state.dlc.size} of ${items.length} selected`;
          },
        }),
        h("span", { text: item.name }),
        h("span", { class: "id", text: item.appid })
      )
    )
  );
  hint.textContent = `${state.dlc.size} of ${items.length} selected`;
}

function renderDepots() {
  const app = state.app;
  const box = $("#depotList");
  if (!app) return;
  const rows = app.depots.map((depot) => {
    const checked = state.selected.has(depot.depot_id);
    const key = depot.key || "";
    const keyInput = h("input", {
      class: `key-input${key ? " has-key" : ""}`,
      type: "text",
      spellcheck: "false",
      placeholder: "depot key",
      value: key,
      title: key || "64 hex characters",
      onchange: (event) => saveKey(depot.depot_id, event.target.value.trim()),
    });
    return h("div", { class: "depot", dataset: { depot: depot.depot_id } },
      h("input", {
        type: "checkbox",
        checked,
        "aria-label": `Include depot ${depot.depot_id}`,
        onchange: (event) => {
          if (event.target.checked) state.selected.add(depot.depot_id);
          else state.selected.delete(depot.depot_id);
          updateBuildBar();
        },
      }),
      h("div", { class: "depot-main" },
        h("div", { class: "depot-title" },
          `Depot ${depot.depot_id}`,
          depot.oslist.length ? h("span", { class: "os", text: depot.oslist.join(" / ") }) : null,
          depot.dlc_appid
            ? h("span", {
                class: "os",
                text: (state.app?.dlc_names || []).find((item) => item.appid === depot.dlc_appid)?.name
                  || `dlc ${depot.dlc_appid}`,
              })
            : null
        ),
        h("div", { class: "depot-meta", text: depot.gid ? `manifest ${depot.gid}` : "no manifest on this branch" })
      ),
      h("div", { class: "depot-side" },
        h("span", { class: "depot-size", text: fmtSize(depot.size) }),
        keyInput
      )
    );
  });
  box.replaceChildren(...rows);
  $("#depotCount").textContent = `(${app.depots.length})`;
  updateBuildBar();
}

function updateBuildBar() {
  const app = state.app;
  const bar = $("#buildBar");
  if (!app) return;
  const total = app.depots.reduce((sum, depot) => sum + (depot.size || 1), 0) || 1;
  bar.replaceChildren(
    ...app.depots.map((depot) =>
      h("span", {
        class: state.selected.has(depot.depot_id) ? "on" : "",
        style: `flex:${Math.max(1, Math.round(((depot.size || 1) / total) * 1000))}`,
        title: `Depot ${depot.depot_id} · ${fmtSize(depot.size)}`,
      })
    )
  );
  const picked = app.depots.filter((depot) => state.selected.has(depot.depot_id));
  const missing = picked.filter((depot) => !depot.key).length;
  const size = picked.reduce((sum, depot) => sum + (depot.size || 0), 0);
  const note = $("#buildNote");
  if (note) {
    note.replaceChildren(
      h("span", { text: `${picked.length} of ${app.depots.length} depots` }),
      h("span", { text: fmtSize(size) })
    );
  }
  $("#depotHint").textContent = missing
    ? `${missing} of ${picked.length} selected depots have no key yet`
    : picked.length
      ? "every selected depot has a key"
      : "nothing selected";
}

async function saveKey(depotId, value) {
  try {
    if (!value) {
      await api(`key?depot_id=${depotId}`, { method: "DELETE" });
    } else {
      await api("key", { method: "POST", body: { depot_id: depotId, key: value } });
    }
    const depot = state.app?.depots.find((item) => item.depot_id === depotId);
    if (depot) {
      depot.key = value || null;
      depot.has_key = Boolean(value);
    }
    renderDepots();
    toast(value ? `key saved for ${depotId}` : `key cleared for ${depotId}`, "ok");
  } catch (error) {
    toast(`key: ${error.message}`, "bad");
  }
}

async function loadScript(appid) {
  try {
    const data = await api(`script?appid=${appid}`);
    $("#scriptBody").textContent = data.script;
    $("#scriptHint").textContent = data.installed
      ? "Installed in Vapor Forge's script folder."
      : "Not installed yet.";
  } catch {
    $("#scriptBody").textContent = "no plan yet";
    $("#scriptHint").textContent = "Save a plan to see the lua.";
  }
}

async function loadGuard(appid) {
  const box = $("#guardState");
  box.replaceChildren(h("p", { class: "note", text: "checking Steam…" }));
  try {
    const data = await api(`appmanifest?appid=${appid}`);
    if (!data.installed) {
      box.replaceChildren(h("p", { class: "note", text: "Steam has not downloaded this app yet." }));
      return;
    }
    box.replaceChildren(
      h("div", { class: "line" }, h("span", { text: "appmanifest" }), h("span", { text: data.path.split("/").pop() })),
      h("div", { class: "line" }, h("span", { text: "steam state" }), h("span", { text: (data.state || []).join(", ") || "unknown" })),
      h("div", { class: "line" }, h("span", { text: "auto update" }), h("span", { text: `${data.auto_update ?? "-"} / schedule ${data.scheduled_auto_update ?? "-"}` })),
      data.problems?.length ? h("p", { class: "note", text: data.problems.join("; ") }) : h("p", { class: "note", text: "flags look healthy." })
    );
  } catch (error) {
    box.replaceChildren(h("p", { class: "note", text: error.message }));
  }
}

async function savePlan() {
  const app = state.app;
  if (!app) return;
  const depots = app.depots
    .filter((depot) => state.selected.has(depot.depot_id))
    .map((depot) => ({ depot_id: depot.depot_id, gid: depot.gid, size: depot.size }));
  if (!depots.length) { toast("pick at least one depot", "bad"); return; }
  try {
    const record = await api("plan", {
      method: "POST",
      body: { appid: app.appid, name: app.name, branch: state.branch, depots, dlc: [...state.dlc] },
    });
    app.plan = record;
    toast("plan saved", "ok");
    await refreshStatus();
    await loadScript(app.appid);
  } catch (error) {
    toast(`plan: ${error.message}`, "bad");
  }
}

async function installPlan() {
  const app = state.app;
  if (!app) return;
  try {
    const data = await api("install", { method: "POST", body: { appid: app.appid, enabled: true } });
    $("#scriptBody").textContent = data.script;
    $("#scriptHint").textContent = data.missing_keys.length
      ? `Written. Missing keys: ${data.missing_keys.join(", ")}`
      : "Written to Vapor Forge.";
    toast("installed — restart Steam if it was closed", "ok");
    await refreshStatus();
  } catch (error) {
    toast(`install: ${error.message}`, "bad");
  }
}

async function checkPlan() {
  const app = state.app;
  if (!app) return;
  try {
    const report = await api("script/check", { method: "POST", body: { appid: app.appid } });
    $("#scriptHint").textContent = report.ok
      ? `Parses clean (${report.tool || "lint only"})`
      : `Parse failed: ${report.output || report.lint.join("; ")}`;
    toast(report.ok ? "plan parses" : "plan has errors", report.ok ? "ok" : "bad");
  } catch (error) {
    toast(`check: ${error.message}`, "bad");
  }
}

async function hubcapFetch() {
  const app = state.app;
  if (!app) return;
  try {
    const data = await api("hubcap", {
      method: "POST",
      body: { action: "fetch", appid: app.appid, name: app.name },
    });
    const summary = data.summary;
    toast(`${summary.keys}/${summary.depots} keys from Hubcap`, summary.keys ? "ok" : "bad");
    await openApp(app.appid, state.branch);
  } catch (error) {
    toast(`hubcap: ${error.message}`, "bad");
  }
}

async function patchManifest(mode) {
  const app = state.app;
  if (!app) return;
  try {
    const data = await api("appmanifest", { method: "POST", body: { appid: app.appid, mode, restart: true } });
    toast(data.restarted ? `${mode} applied, Steam restarted` : `${mode} applied`, "ok");
    await loadGuard(app.appid);
  } catch (error) {
    toast(`appmanifest: ${error.message}`, "bad");
  }
}

// ------------------------------------------------------------------- system

async function loadSystem() {
  loadEngine();
  loadSteamUi();
  loadHubcap();
  loadProvider();
  loadLog();
  renderSettings();
}

async function loadEngine() {
  try {
    state.engine = await api("engine");
  } catch (error) {
    state.engine = { error: error.message };
  }
  renderEngine();
  renderMenuStatus();
}

function renderEngine() {
  const box = $("#engineState");
  const info = state.engine;
  if (!info) return;
  if (info.error) {
    box.replaceChildren(h("p", { class: "note", text: info.error }));
    return;
  }
  const libs = Object.values(info.libs || {}).reduce((sum, list) => sum + list.length, 0);
  const steam = !info.steam_running
    ? "stopped"
    : info.steam_engine_loaded
      ? `running, engine loaded (pid ${info.steam_pid || "?"})`
      : "running, engine not loaded";
  box.replaceChildren(
    h("div", { class: "line" }, h("span", { text: "engine" }), h("span", { text: info.installed ? info.version || "installed" : "not installed" })),
    h("div", { class: "line" }, h("span", { text: "libraries" }), h("span", { text: `${libs}/4` })),
    h("div", { class: "line" }, h("span", { text: "ld_audit" }), h("span", { text: info.ld_audit?.configured ? "wired" : "not wired" })),
    h("div", { class: "line" }, h("span", { text: "steam" }), h("span", { text: steam })),
    h("div", { class: "line" }, h("span", { text: "scripts" }), h("span", { text: info.scripts_dir }))
  );
  $("#installEngine").textContent = info.installed ? "Update engine" : "Install engine";
}

async function loadSteamUi() {
  try {
    state.steamui = await api("steamui");
  } catch (error) {
    state.steamui = { error: error.message };
  }
  const box = $("#steamUiState");
  const info = state.steamui;
  if (!box) return;
  if (info.error) {
    box.replaceChildren(h("p", { class: "note", text: info.error }));
    return;
  }
  box.replaceChildren(
    h("div", { class: "line" },
      h("span", { text: "menu" }),
      h("span", { text: info.injected ? "icon is next to Help" : "not injected" })
    ),
    h("div", { class: "line" },
      h("span", { text: "debug port" }),
      h("span", { text: info.debug_ready ? String(info.debug_port) : "closed" })
    )
  );
}

async function loadHubcap() {
  try {
    state.hubcap = await api("hubcap");
  } catch (error) {
    state.hubcap = { error: error.message };
  }
  const hint = $("#hubcapHint");
  const info = state.hubcap;
  if (!hint || !info) return;
  if (info.error) { hint.textContent = info.error; return; }
  if (!info.configured) { hint.textContent = "No key: depot keys stay manual."; return; }
  const stats = info.stats || {};
  hint.textContent = `${info.hint} · ${stats.usage ?? "?"}/${stats.limit ?? "?"} requests today`;
}

async function loadProvider() {
  try {
    state.provider = await api("provider");
  } catch (error) {
    state.provider = { error: error.message };
  }
  const node = $("#providerState");
  const info = state.provider;
  if (!node || !info) return;
  if (info.error) { node.textContent = info.error; return; }
  const current = info.state || {};
  node.textContent = current.installed
    ? `${current.path.split("/").pop()} · ${(current.providers || []).join(", ")}`
    : "not installed";
}

async function loadLog() {
  const box = $("#logBody");
  try {
    const data = await api("log?lines=120");
    if (data.error) {
      box.textContent = data.error;
      $("#logHint").textContent = data.path || "";
      return;
    }
    if (!data.lines.length) {
      box.textContent = "no engine lines yet — restart Steam";
    } else {
      box.replaceChildren(...data.lines.map(logLine));
    }
    $("#logHint").textContent = `${data.lines.length} of ${data.total} lines from ${data.path}`;
    box.scrollTop = box.scrollHeight;
  } catch (error) {
    box.textContent = `log: ${error.message}`;
  }
}

function logLine(line) {
  const kind = /\[(WARN|WARNING)\]/.test(line) ? "warn" : /\[(ERROR|FATAL)\]/.test(line) ? "error" : "";
  const node = h("span", { class: `log-line ${kind}`.trim() });
  const head = line.match(/^(\[[^\]]+\])(.*)$/);
  if (!head) {
    node.textContent = line;
    return node;
  }
  node.append(h("span", { class: "ts", text: `${head[1]} ` }));
  const level = head[2].match(/^(.*?)(\[(?:INFO|WARN|WARNING|ERROR|DEBUG|TRACE)\])([\s\S]*)$/);
  if (level) {
    node.append(document.createTextNode(level[1]), h("span", { text: level[2] }), document.createTextNode(level[3]));
  } else {
    node.append(document.createTextNode(head[2]));
  }
  return node;
}

function renderSettings() {
  const settings = state.status?.settings;
  if (!settings) return;
  $("#providerInput").value = settings.manifest_providers
    .map((entry) => (typeof entry === "string" ? entry : entry.name))
    .join(", ");
  $("#keySourceInput").value = settings.key_source_url || "";
  $("#countryInput").value = settings.country || "us";
}

async function saveSettings() {
  const providers = $("#providerInput").value.split(",").map((item) => item.trim()).filter(Boolean);
  try {
    await api("settings", {
      method: "POST",
      body: {
        manifest_providers: providers,
        key_source_url: $("#keySourceInput").value.trim(),
        country: $("#countryInput").value.trim() || "us",
      },
    });
    await refreshStatus();
    toast("settings saved", "ok");
  } catch (error) {
    toast(`settings: ${error.message}`, "bad");
  }
}

// --------------------------------------------------------------------- menu

const menuButton = $("#menuButton");
const menuPanel = $("#appMenu");

function menuOpen() {
  menuPanel.hidden = false;
  menuButton.setAttribute("aria-expanded", "true");
  renderMenuStatus();
  menuPanel.querySelector(".menu-item")?.focus();
}

function menuClose({ focus = false } = {}) {
  if (menuPanel.hidden) return;
  menuPanel.hidden = true;
  menuButton.setAttribute("aria-expanded", "false");
  if (focus) menuButton.focus();
}

function renderMenuStatus() {
  const engine = state.engine || {};
  $("#menuVersion").textContent = engine.version || "—";
  const node = $("#menuStatus");
  let text = "engine not installed";
  let kind = "bad";
  if (engine.installed) {
    if (!engine.steam_running) { text = "Steam is not running"; kind = "warn"; }
    else if (engine.steam_engine_loaded) { text = "engine loaded in Steam"; kind = "ok"; }
    else { text = "restart Steam to load the engine"; kind = "warn"; }
  }
  node.className = `menu-status ${kind}`;
  node.replaceChildren(h("span", { class: "dot" }), document.createTextNode(text));
}

async function menuAction(action) {
  menuClose();
  if (action === "library") { switchView("library"); return; }
  if (action === "system") { switchView("system"); return; }
  if (action === "restart") { await restartSteam(); return; }
  if (action === "readme") {
    try {
      const info = await api("open", { method: "POST", body: { target: "readme" } });
      if (!info.opened) toast(`README: ${info.path}`, "warn");
    } catch (error) {
      toast(`open: ${error.message}`, "bad");
    }
    return;
  }
  if (action === "about") {
    const engine = state.engine || {};
    toast(`ExxaTools · engine ${engine.version || "—"} · ${state.status?.state_path || ""}`, "ok");
  }
}

async function restartSteam() {
  try {
    toast("stopping Steam…");
    await api("steam/restart", { method: "POST", body: {} });
    toast("Steam restarted", "ok");
    state.engine = null;
    await loadEngine();
  } catch (error) {
    toast(`restart: ${error.message}`, "bad");
  }
}

// ------------------------------------------------------------------- wiring

document.querySelectorAll(".nav-item").forEach((item) => {
  item.addEventListener("click", () => switchView(item.dataset.view));
});

$("#backToLibrary").addEventListener("click", () => switchView("library"));

$("#searchInput").addEventListener("input", (event) => {
  clearTimeout(searchTimer);
  const value = event.target.value;
  searchTimer = setTimeout(() => runSearch(value), 380);
});
$("#searchInput").addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    clearTimeout(searchTimer);
    runSearch(event.target.value);
  }
});

$("#branchSelect").addEventListener("change", (event) => {
  if (state.app) openApp(state.app.appid, event.target.value);
});

document.querySelectorAll("[data-select]").forEach((button) => {
  button.addEventListener("click", () => {
    const mode = button.dataset.select;
    document.querySelectorAll("[data-select]").forEach((item) => item.classList.remove("is-active"));
    button.classList.add("is-active");
    state.selected.clear();
    if (mode !== "none" && state.app) {
      for (const depot of state.app.depots) {
        if (!depot.oslist.length || depot.oslist.includes(mode)) state.selected.add(depot.depot_id);
      }
    }
    renderDepots();
  });
});

$("#savePlan").addEventListener("click", savePlan);
$("#installPlan").addEventListener("click", installPlan);
$("#checkPlan").addEventListener("click", checkPlan);
$("#hubcapFetch").addEventListener("click", hubcapFetch);
$("#fixManifest").addEventListener("click", () => patchManifest("fix"));
$("#blockUpdates").addEventListener("click", () => patchManifest("block"));

$("#installEngine").addEventListener("click", async () => {
  const button = $("#installEngine");
  button.disabled = true;
  button.textContent = "installing…";
  try {
    const data = await api("engine/install", { method: "POST", body: {} });
    toast(`engine ${data.version} installed`, "ok");
    await loadEngine();
    await refreshStatus();
  } catch (error) {
    toast(`engine: ${error.message}`, "bad");
  } finally {
    button.disabled = false;
    renderEngine();
  }
});
$("#restartSteam").addEventListener("click", restartSteam);
$("#steamUiInject").addEventListener("click", () => steamUiAction("inject"));
$("#steamUiRemove").addEventListener("click", () => steamUiAction("remove"));
$("#steamUiEnable").addEventListener("click", () => steamUiAction("enable"));
$("#saveHubcap").addEventListener("click", async () => {
  const value = $("#hubcapInput").value.trim();
  try {
    const data = await api("hubcap", { method: "POST", body: { action: "save", key: value } });
    toast(data.configured ? "key saved" : "key cleared", "ok");
    $("#hubcapInput").value = "";
    await loadHubcap();
  } catch (error) {
    toast(`hubcap: ${error.message}`, "bad");
  }
});
$("#saveSettings").addEventListener("click", saveSettings);
$("#installProvider").addEventListener("click", async () => {
  try {
    await api("provider", { method: "POST", body: {} });
    toast("provider installed", "ok");
    await loadProvider();
  } catch (error) {
    toast(`provider: ${error.message}`, "bad");
  }
});
$("#removeProvider").addEventListener("click", async () => {
  try {
    await api("provider", { method: "DELETE" });
    toast("provider removed", "ok");
    await loadProvider();
  } catch (error) {
    toast(`provider: ${error.message}`, "bad");
  }
});
$("#reloadLog").addEventListener("click", loadLog);
$("#shuffleDiscover").addEventListener("click", loadDiscover);
$("#runDoctor").addEventListener("click", loadDoctor);

document.querySelectorAll("#steps .step").forEach((button) => {
  button.addEventListener("click", () => {
    state.step = Number(button.dataset.step);
    renderSteps();
    const target = [".hero", "#dlcPanel", "#depotList", "#buildBar"][state.step];
    document.querySelector(target)?.scrollIntoView({ behavior: "smooth", block: "start" });
    if (state.step === 3) updateBuildBar();
  });
});

const dropZone = h("div", { class: "doctor-drop", id: "dropZone", text: "Drop a .lua or .vdf to import depot keys" });
document.body.append(dropZone);
let dragDepth = 0;
window.addEventListener("dragenter", (event) => {
  if (!event.dataTransfer?.types?.includes("Files")) return;
  dragDepth += 1;
  dropZone.classList.add("on");
});
window.addEventListener("dragleave", () => {
  dragDepth = Math.max(0, dragDepth - 1);
  if (!dragDepth) dropZone.classList.remove("on");
});
window.addEventListener("dragover", (event) => event.preventDefault());
window.addEventListener("drop", async (event) => {
  event.preventDefault();
  dragDepth = 0;
  dropZone.classList.remove("on");
  for (const file of event.dataTransfer?.files || []) {
    if (!/\.(lua|vdf|txt|st)$/i.test(file.name)) continue;
    await importLua(await file.text(), file.name);
  }
});

async function steamUiAction(action) {
  try {
    await api("steamui", { method: "POST", body: { action } });
    toast(action === "enable" ? "Steam restarted with the debug port" : `menu ${action}`, "ok");
    await loadSteamUi();
    await loadEngine();
  } catch (error) {
    toast(`steam menu: ${error.message}`, "bad");
  }
}

menuButton.addEventListener("click", (event) => {
  event.stopPropagation();
  if (menuPanel.hidden) menuOpen(); else menuClose({ focus: true });
});
menuPanel.addEventListener("click", (event) => {
  const item = event.target.closest("[data-menu]");
  if (item) menuAction(item.dataset.menu);
});
document.addEventListener("click", (event) => {
  if (!event.target.closest(".menu-wrap")) menuClose();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !menuPanel.hidden) {
    menuClose({ focus: true });
  }
});

refreshStatus();
loadDiscover();
loadDoctor();
loadSystem();