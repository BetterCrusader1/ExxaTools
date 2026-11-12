/* ExxaTools dashboard: search, add, fix. Nothing else needs a manual. */

const api = async (path, init) => {
  const response = await fetch(path, init);
  const text = await response.text();
  let data = {};
  try { data = text ? JSON.parse(text) : {}; } catch { data = { error: text }; }
  if (!response.ok) {
    const message = data && data.error ? data.error : `request failed (${response.status})`;
    throw new Error(message);
  }
  return data;
};

const post = (path, body) => api(path, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body || {}),
});

const el = (tag, className, text) => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};

const artUrls = (appid) => [
  `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/header.jpg`,
  `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/capsule_616x353.jpg`,
  `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/capsule_231x87.jpg`,
];
const cover = (appid) => artUrls(appid)[0];

/// Steam only ships a header image for released games, so walk down the capsule
/// sizes before admitting there is no artwork.
function attachArt(container, appid, name) {
  const urls = artUrls(appid);
  const img = document.createElement("img");
  img.loading = "lazy";
  img.alt = "";
  let index = 0;
  img.addEventListener("error", () => {
    index += 1;
    if (index < urls.length) {
      img.src = urls[index];
      return;
    }
    img.remove();
    const fallback = el("div", "fallback");
    fallback.append(el("span", "fallback-name", "no artwork"));
    container.append(fallback);
  });
  img.src = urls[0];
  container.append(img);
  return img;
}

const state = { items: [], settings: {}, sources: [], term: "" };

/* ---------- toasts ---------- */

function toast(text, options = {}) {
  const box = document.getElementById("toasts");
  const node = el("div", `toast ${options.kind || ""}`.trim());
  node.append(el("span", "", text));
  if (options.action) {
    const button = el("button", "", options.action.label);
    button.addEventListener("click", () => { node.remove(); options.action.run(); });
    node.append(button);
  }
  box.append(node);
  setTimeout(() => node.remove(), options.sticky ? 15000 : 5200);
}

const fail = (error) => toast(error.message || String(error), { kind: "bad" });

/* ---------- header ---------- */

function renderHeader() {
  const chip = document.getElementById("steamChip");
  const text = document.getElementById("steamText");
  const engine = state.engine || {};
  const steam = state.steam || {};
  chip.className = "chip";
  if (!steam.running) {
    chip.classList.add("warn");
    text.textContent = "Steam is closed";
    chip.title = "Steam is not running — start it to use the library";
  } else if (engine.steam_engine_loaded) {
    chip.classList.add("ok");
    text.textContent = "Steam connected";
    chip.title = "Vapor Forge is loaded inside Steam";
  } else {
    chip.classList.add("warn");
    text.textContent = "Restart Steam";
    chip.title = "The engine is installed but not loaded: restart Steam";
  }
  chip.onclick = () => (steam.running ? openSettings() : restartSteam());
}

/* ---------- library ---------- */

function gameBadge(item) {
  return item.installed ? { text: "in Steam", kind: "ok" } : { text: "not installed", kind: "" };
}

function gameCard(item) {
  const card = el("article", "card");
  const art = el("div", "card-art");
  attachArt(art, item.appid, item.name);
  const badge = gameBadge(item);
  art.append(el("span", `card-badge ${badge.kind}`.trim(), badge.text));
  art.addEventListener("click", () => openGame(item.appid));

  const body = el("div", "card-body");
  const name = el("h3", "card-name", item.name || String(item.appid));
  name.addEventListener("click", () => openGame(item.appid));
  body.append(name);
  const parts = [`${item.depot_count} depots`];
  if ((item.dlc || []).length) parts.push(`${item.dlc.length} DLC`);
  parts.push(item.launch_options ? "custom launch flags" : item.branch || "public");
  body.append(el("div", "card-meta", parts.join(" · ")));

  const actions = el("div", "card-actions");
  if (item.installed) {
    const open = el("button", "btn ghost grow", "Details");
    open.addEventListener("click", () => openGame(item.appid));
    actions.append(open);
    const fix = el("button", "btn tiny", "Fix");
    fix.addEventListener("click", () => fixGame(item.appid, "manifest", "Repairing the manifest…"));
    actions.append(fix);
  } else {
    const install = el("button", "btn primary grow", "Add to Steam");
    install.addEventListener("click", () => installGame(item.appid, item.name, install));
    actions.append(install);
  }

  card.append(art, body, actions);
  return card;
}

function renderDiagnose() {
  const banner = document.getElementById("diagnoseBanner");
  const incidents = (state.diagnose && state.diagnose.incidents) || [];
  if (!incidents.length) { banner.hidden = true; return; }
  banner.hidden = false;
  banner.replaceChildren(el("div", "diagnose-head", `Steam reported ${incidents.length} problem${incidents.length === 1 ? "" : "s"}`));
  for (const incident of incidents.slice(0, 4)) {
    const row = el("div", "diagnose-row");
    const text = el("div", "row-text");
    text.append(el("b", "", incident.title), el("span", "", incident.detail || ""));
    row.append(text);
    if (incident.action === "keys") {
      const run = el("button", "btn tiny", "Refresh keys");
      run.addEventListener("click", () => fixGame(0, "keys", "Refreshing keys…"));
      row.append(run);
    } else if (incident.action === "engine") {
      const run = el("button", "btn tiny", "Update engine");
      run.addEventListener("click", async () => {
        run.disabled = true;
        run.textContent = "Installing…";
        try { await post("/api/engine/install", {}); toast("Engine installed"); await loadDiagnose(); }
        catch (error) { fail(error); }
        finally { run.disabled = false; run.textContent = "Update engine"; }
      });
      row.append(run);
    } else if (incident.action) {
      const open = el("button", "btn tiny ghost", "Open fixes");
      open.addEventListener("click", () => document.getElementById("fixes").scrollIntoView({ behavior: "smooth" }));
      row.append(open);
    }
    banner.append(row);
  }
}

function renderLibrary() {
  const cards = document.getElementById("cards");
  const empty = document.getElementById("empty");
  const note = document.getElementById("libNote");
  cards.replaceChildren(...state.items.map(gameCard));
  empty.hidden = state.items.length > 0;
  cards.hidden = state.items.length === 0;
  const installed = state.items.filter((item) => item.installed).length;
  note.textContent = state.items.length
    ? `${state.items.length} game${state.items.length === 1 ? "" : "s"} · ${installed} active`
    : "";
}

async function loadDiagnose() {
  try { state.diagnose = await api("/api/diagnose"); } catch { state.diagnose = { incidents: [] }; }
  renderDiagnose();
}

async function loadLibrary() {
  const data = await api("/api/library");
  state.items = data.items || [];
  state.engine = data.engine || {};
  state.steam = data.steam || {};
  state.settings = data.settings || {};
  renderHeader();
  renderLibrary();
  renderDiagnose();
  loadSuggestions();
  const foot = document.getElementById("footSteam");
  if (foot) foot.textContent = state.steam && state.steam.running ? "Steam running" : "Steam closed";
}

function suggestCard(item) {
  const card = el("article", "card");
  const art = el("div", "card-art");
  attachArt(art, item.appid, item.name);
  art.addEventListener("click", () => openGame(item.appid));
  const body = el("div", "card-body");
  body.append(
    el("h3", "card-name", item.name || String(item.appid)),
    el("div", "card-meta", `Steam app ${item.appid}`),
  );
  const actions = el("div", "card-actions");
  const add = el("button", "btn primary grow", "Add to Steam");
  add.addEventListener("click", async () => {
    add.disabled = true;
    add.textContent = "Adding…";
    await installGame(item.appid, item.name);
    add.textContent = "Added";
  });
  actions.append(add);
  card.append(art, body, actions);
  return card;
}

async function loadSuggestions() {
  const section = document.getElementById("suggestSection");
  const box = document.getElementById("suggest");
  if (state.items.length) { section.hidden = true; return; }
  section.hidden = false;
  if (box.childElementCount) return;
  try {
    const data = await api("/api/discover?limit=8");
    box.replaceChildren(...(data.items || []).slice(0, 8).map(suggestCard));
  } catch { section.hidden = true; }
}

/* ---------- install flow ---------- */

async function installGame(appid, name, button, dlcSelection) {
  if (button) { button.disabled = true; button.textContent = "Adding…"; }
  try {
    const app = await api(`/api/app?appid=${appid}`);
    const selection = await api(`/api/autoselect?appid=${appid}`);
    const picked = new Set((selection.items || []).filter((row) => row.selected).map((row) => row.depot_id));
    const depots = (app.depots || [])
      .filter((depot) => picked.has(depot.depot_id))
      .map((depot) => ({ depot_id: depot.depot_id, gid: depot.gid || "", size: depot.size ?? null }));
    if (!depots.length) throw new Error("this game has no downloadable depots in the public branch");

    await post("/api/plan", {
      appid,
      name: name || app.name || String(appid),
      branch: "public",
      depots,
      dlc: dlcSelection || (app.plan && app.plan.dlc) || [],
      fixes: { ticket: "forge", steam_id: "", avatar: 0, purchase_time: 0, online_fix: false },
    });
    await post("/api/install", { appid, enabled: true });
    await loadLibrary();
    toast(`${name || appid} is ready. Restart Steam to finish.`, {
      action: { label: "Restart Steam", run: restartSteam },
      sticky: true,
    });
  } catch (error) {
    fail(error);
  } finally {
    if (button) { button.disabled = false; button.textContent = "Add to Steam"; }
  }
}

/* ---------- import ---------- */

function openImport() {
  document.getElementById("importModal").hidden = false;
  document.getElementById("importText").value = "";
  document.getElementById("dropText").textContent = "Drop a .lua file, or click to choose";
  document.getElementById("importText").focus();
}

function closeImport() {
  document.getElementById("importModal").hidden = true;
}

async function readFileIntoBox(file) {
  if (!file) return;
  const text = await file.text();
  document.getElementById("importText").value = text;
  document.getElementById("dropText").textContent = file.name;
}

async function runImport() {
  const text = document.getElementById("importText").value.trim();
  if (!text) return toast("Paste a plan or drop a file first", { kind: "warn" });
  const create = document.getElementById("importCreate").checked;
  const button = document.getElementById("importRun");
  button.disabled = true;
  button.textContent = "Importing…";
  try {
    const result = await post("/api/import", { text, create });
    const summary = result.summary || {};
    const keys = summary.keys || 0;
    const parts = [`${keys} key${keys === 1 ? "" : "s"}`];
    if (summary.depots) parts.push(`${summary.depots} depots`);
    if (summary.created_plan) parts.push("added to the library");
    toast(`Imported: ${parts.join(", ")}`);
    closeImport();
    await loadLibrary();
  } catch (error) {
    fail(error);
  } finally {
    button.disabled = false;
    button.textContent = "Import";
  }
}

async function getPlan(appid, name, button) {
  if (button) { button.disabled = true; button.textContent = "Fetching…"; }
  try {
    const result = await post("/api/plan/fetch", { appid, name });
    const summary = result.summary || {};
    const keys = summary.keys || 0;
    const created = summary.created_plan ? "plan created" : "keys updated";
    toast(`${created} by ${result.provider}: ${keys} key${keys === 1 ? "" : "s"}`, {
      action: { label: "Restart Steam", run: restartSteam },
    });
    await loadLibrary();
    closeDrawer();
  } catch (error) {
    fail(error);
  } finally {
    if (button) { button.disabled = false; button.textContent = "Get plan"; }
  }
}

/* ---------- fixes ---------- */

async function restartSteam() {
  toast("Restarting Steam…");
  try {
    await post("/api/steam/restart", {});
    setTimeout(loadLibrary, 9000);
  } catch (error) { fail(error); }
}

async function fixGame(appid, action, label) {
  if (label) toast(label);
  try {
    const result = await post("/api/fix", { appid, action });
    toast(result.message || "done");
    if (action === "remove") { closeDrawer(); }
    await loadLibrary();
    return result;
  } catch (error) {
    if (/Steam is running/i.test(error.message)) {
      toast("Steam must be closed for this. Restart it and the fix applies on boot.", {
        kind: "warn",
        sticky: true,
        action: { label: "Restart Steam now", run: restartSteam },
      });
    } else {
      fail(error);
    }
  }
}

const fixes = [
  {
    title: "Restart Steam",
    text: "Pick up new keys and scripts",
    icon: '<path d="M20 12a8 8 0 1 1-2.3-5.6"/><path d="M20 4v4h-4"/>',
    run: restartSteam,
  },
  {
    title: "Repair all manifests",
    text: "Fix stuck updates and download loops",
    icon: '<path d="M14.7 6.3a4 4 0 0 0 5 5L21 21H3l1.3-9.7a4 4 0 0 0 5-5L12 3z"/>',
    async run() {
      const targets = state.items.filter((item) => item.installed);
      if (!targets.length) return toast("Nothing installed yet", { kind: "warn" });
      try {
        await post("/api/steam/shutdown", {});
        toast("Steam closed, repairing…", { sticky: true });
        let repaired = 0;
        for (const item of targets) {
          try { await post("/api/fix", { appid: item.appid, action: "manifest" }); repaired += 1; } catch { /* not installed */ }
        }
        await post("/api/steam/restart", {});
        toast(`${repaired} manifest${repaired === 1 ? "" : "s"} repaired, Steam is starting`);
        setTimeout(loadLibrary, 12000);
      } catch (error) { fail(error); }
    },
  },
  {
    title: "Refresh keys",
    text: "Re-sync the keys Steam reads at boot",
    icon: '<path d="M15 7a4 4 0 1 0 0 10 4 4 0 0 0 0-10z"/><path d="M2 12h6M17 12h5M7 9v6"/>',
    run: () => fixGame(0, "keys", "Refreshing keys…"),
  },
  {
    title: "Install provider",
    text: "Let Steam look up manifests itself",
    icon: '<path d="M12 3v12M7 10l5 5 5-5"/><path d="M4 20h16"/>',
    run: () => fixGame(0, "provider", "Installing the provider…"),
  },
  {
    title: "Steam menu",
    text: "Show ExxaTools next to Steam Help",
    icon: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M3 9h18"/>',
    async run() {
      try { await post("/api/steamui", { action: "inject" }); toast("Menu injected — restart Steam if it does not show"); }
      catch (error) { fail(error); }
    },
  },
];

function renderFixes() {
  const box = document.getElementById("fixes");
  box.replaceChildren(...fixes.map((entry) => {
    const node = el("button", "fix");
    node.type = "button";
    const icon = el("span", "fix-icon");
    icon.innerHTML = `<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${entry.icon}</svg>`;
    const text = el("span", "fix-text");
    text.append(el("b", "", entry.title), el("span", "", entry.text));
    const chevron = el("span", "fix-chevron");
    chevron.innerHTML = '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m9 6 6 6-6 6"></path></svg>';
    node.append(icon, text, chevron);
    node.addEventListener("click", async () => {
      node.classList.add("busy");
      try { await entry.run(); } finally { setTimeout(() => node.classList.remove("busy"), 600); }
    });
    return node;
  }));
}

/* ---------- search ---------- */

async function runSearch() {
  const input = document.getElementById("q");
  const term = input.value.trim();
  if (!term) return;
  state.term = term;
  const box = document.getElementById("results");
  box.hidden = false;
  box.replaceChildren(el("div", "results-empty", "Searching Steam…"));
  try {
    const data = await api(`/api/search?q=${encodeURIComponent(term)}`);
    const items = data.items || [];
    if (!items.length) { box.replaceChildren(el("div", "results-empty", "Nothing found. Try the app id.")); return; }
    box.replaceChildren(...items.slice(0, 12).map(resultRow));
  } catch (error) {
    box.replaceChildren(el("div", "results-empty", error.message));
  }
}

function resultRow(item) {
  const row = el("div", "result");
  const img = document.createElement("img");
  img.src = item.image || cover(item.appid);
  img.alt = "";
  img.loading = "lazy";
  img.addEventListener("error", () => img.remove());
  const text = el("div", "result-name");
  text.append(el("b", "", item.name || String(item.appid)), el("span", "", String(item.appid)));
  const add = el("button", "result-add", state.items.some((entry) => entry.appid === item.appid) ? "In library" : "Add");
  add.disabled = state.items.some((entry) => entry.appid === item.appid);
  add.addEventListener("click", async () => {
    add.disabled = true;
    add.textContent = "Adding…";
    await installGame(item.appid, item.name);
    add.textContent = "Added";
    document.getElementById("results").hidden = true;
    document.getElementById("q").value = "";
  });
  row.style.cursor = "pointer";
  row.addEventListener("click", (event) => {
    if (event.target.closest(".result-add")) return;
    document.getElementById("results").hidden = true;
    openGame(item.appid);
  });
  row.append(img, text, add);
  return row;
}

/* ---------- drawer ---------- */

let drawerFor = null;

async function openGame(appid) {
  drawerFor = appid;
  const drawer = document.getElementById("drawer");
  const inner = document.getElementById("drawerInner");
  document.getElementById("scrim").hidden = false;
  drawer.hidden = false;
  drawer.setAttribute("aria-hidden", "false");
  inner.replaceChildren(el("div", "drawer-body", "Loading…"));

  let app = null;
  try { app = await api(`/api/app?appid=${appid}`); } catch (error) { fail(error); }

  const item = state.items.find((entry) => entry.appid === appid) || { appid, depot_count: 0 };
  const name = (app && app.name) || item.name || String(appid);
  inner.replaceChildren();

  const top = el("div", "drawer-top");
  attachArt(top, appid, name);
  const close = el("button", "drawer-close");
  close.innerHTML = '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"></path></svg>';
  close.addEventListener("click", closeDrawer);
  top.append(close);

  const body = el("div", "drawer-body");
  body.append(el("h3", "", name));
  const sub = [String(appid)];
  if (app && app.developers) sub.push(app.developers);
  if (app && app.branch) sub.push(app.branch);
  body.append(el("div", "drawer-sub", sub.join(" · ")));

  const facts = el("div", "facts");
  const missing = item.missing_keys || [];
  facts.append(el("span", `fact ${item.installed ? "ok" : ""}`.trim(), item.installed ? "In Steam" : "Not installed"));
  facts.append(el("span", `fact ${missing.length ? "warn" : ""}`.trim(), missing.length ? `${missing.length} keys missing` : "Keys ready"));
  if (app) facts.append(el("span", "fact", `${(app.depots || []).length} depots`));
  if (app && (app.dlc || []).length) facts.append(el("span", "fact", `${app.dlc.length} DLC`));
  body.append(facts);

  const actions = el("div", "drawer-actions");
  const needsKeys = !app || !app.plan || (item.missing_keys || []).length > 0;
  if (needsKeys) {
    const fetchPlan = el("button", "btn primary", "Get plan");
    fetchPlan.title = "Fetch keys and manifests from the configured sources";
    fetchPlan.addEventListener("click", () => getPlan(appid, name, fetchPlan));
    actions.append(fetchPlan);
  }
  if (item.installed) {
    const installed = el("button", "btn ghost", "In Steam");
    installed.disabled = true;
    actions.append(installed);
  } else {
    const install = el("button", "btn primary", "Add to Steam");
    install.addEventListener("click", async () => { await installGame(appid, name, install, [...selectedDlc]); closeDrawer(); });
    actions.append(install);
  }
  const restart = el("button", "btn", "Restart Steam");
  restart.addEventListener("click", restartSteam);
  actions.append(restart);
  body.append(actions);

  const selectedDlc = new Set((app && app.plan && app.plan.dlc) || []);
  if (app && (app.dlc_names || []).length) {
    const dlcBox = el("div", "drawer-list");
    const head = el("div", "row");
    const headText = el("div", "row-text");
    headText.append(el("b", "", `DLC (${app.dlc_names.length})`), el("span", "", "Selected DLC is added with the game"));
    head.append(headText);
    if (app.plan) {
      const saveDlc = el("button", "btn tiny ghost", "Save");
      saveDlc.addEventListener("click", async () => {
        const body = {
          appid, name: app.plan.name || name, branch: app.plan.branch || "public",
          depots: app.plan.depots || [], dlc: [...selectedDlc], fixes: app.plan.fixes || {},
        };
        try {
          await post("/api/plan", body);
          await post("/api/install", { appid, enabled: true });
          toast("DLC saved");
          await loadLibrary();
        } catch (error) { fail(error); }
      });
      head.append(saveDlc);
    }
    dlcBox.append(head);
    for (const entry of app.dlc_names.slice(0, 12)) {
      const row = el("label", "dlc-row");
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = selectedDlc.has(entry.appid);
      box.addEventListener("change", () => {
        if (box.checked) selectedDlc.add(entry.appid); else selectedDlc.delete(entry.appid);
      });
      row.append(box, el("span", "", entry.name || String(entry.appid)), el("span", "hint", String(entry.appid)));
      dlcBox.append(row);
    }
    body.append(dlcBox);
  }

  const appIncidents = ((state.diagnose && state.diagnose.incidents) || [])
    .filter((incident) => (incident.line || "").includes(String(appid)));
  if (appIncidents.length) {
    const box = el("div", "drawer-list");
    for (const incident of appIncidents) {
      const row = el("div", "row");
      const text = el("div", "row-text");
      text.append(el("b", "", incident.title), el("span", "", incident.detail || ""));
      row.append(text);
      if (incident.action) {
        const run = el("button", "btn tiny", "Fix");
        run.addEventListener("click", () => fixGame(appid, incident.action));
        row.append(run);
      }
      box.append(row);
    }
    body.append(box);
  }

  const list = el("div", "drawer-list");
  const cloud = item.cloud || {};
  const cloudNote = cloud.found
    ? (cloud.state ? `Last sync: ${cloud.state}` : "Never synced")
    : "";
  const rows = [
    { title: "Repair update loop", text: "Reset the flags that stall downloads", action: "manifest" },
    { title: "Refresh keys", text: (item.missing_keys || []).length ? `${item.missing_keys.length} keys are not on disk yet` : "Rewrite this game's script with keys on disk", action: "keys" },
    { title: "Reset cloud sync", text: cloudNote || "Clear the stuck cloud marker for this game", action: "cloud" },
    { title: "Clear launch options", text: "Remove custom flags for this game", action: "launch" },
  ];
  for (const row of rows) {
    const node = el("div", "row");
    const text = el("div", "row-text");
    text.append(el("b", "", row.title), el("span", "", row.text));
    const button = el("button", "btn tiny ghost", "Run");
    button.setAttribute("aria-label", `Run ${row.title}`);
    button.addEventListener("click", () => fixGame(appid, row.action));
    node.append(text, button);
    list.append(node);
  }
  body.append(list);

  if (item.installed) {
    const zone = el("div", "danger-zone");
    const text = el("div", "row-text");
    text.append(el("b", "", "Remove from Steam"), el("span", "", "Deletes the script and the plan"));
    const remove = el("button", "btn tiny danger", "Remove");
    remove.addEventListener("click", () => {
      if (remove.dataset.armed) { fixGame(appid, "remove", "Removing…"); return; }
      remove.dataset.armed = "1";
      remove.textContent = "Confirm";
      setTimeout(() => { delete remove.dataset.armed; remove.textContent = "Remove"; }, 4000);
    });
    zone.append(text, remove);
    body.append(zone);
  }

  if (app) {
    const details = document.createElement("details");
    details.className = "advanced";
    details.append(el("summary", "", "Depots"));
    const planDepots = (app.plan && app.plan.depots) || [];
    const catalogByid = new Map((app.depots || []).map((depot) => [depot.depot_id, depot]));
    if (!planDepots.length) {
      details.append(el("p", "hint", "No plan yet: a depot editor appears once the game is added."));
    }
    for (const depot of planDepots) {
      const depotId = Number(depot.depot_id);
      const catalog = catalogByid.get(depotId) || {};
      const row = el("div", "depot-row");

      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = depot.enabled !== false;
      box.title = "Include this depot in the script";
      box.addEventListener("change", async () => {
        try {
          await post("/api/depot", { appid, depot_id: depotId, enabled: box.checked });
          toast(`Depot ${depotId} ${box.checked ? "enabled" : "disabled"}`);
        } catch (error) { fail(error); box.checked = !box.checked; }
      });

      const label = el("span", "depot-id", String(depotId));
      label.title = catalog.oslist && catalog.oslist.length ? `only for ${catalog.oslist.join(", ")}` : "";

      const gid = document.createElement("input");
      gid.className = "depot-gid";
      gid.value = depot.gid || "";
      gid.placeholder = "manifest id";
      gid.spellcheck = false;
      gid.addEventListener("change", async () => {
        try {
          await post("/api/depot", { appid, depot_id: depotId, gid: gid.value.trim() });
          toast(`Depot ${depotId} pinned`);
        } catch (error) { fail(error); }
      });

      const key = el("span", `depot-key ${catalog.has_key ? "ok" : ""}`.trim(), catalog.has_key ? "key" : "no key");
      row.append(box, label, gid, key);
      details.append(row);
    }
    body.append(details);
  }

  inner.append(top, body);
}

function closeDrawer() {
  drawerFor = null;
  document.getElementById("drawer").hidden = true;
  document.getElementById("drawer").setAttribute("aria-hidden", "true");
  document.getElementById("scrim").hidden = true;
}

/* ---------- settings ---------- */

async function openSettings() {
  const modal = document.getElementById("settings");
  const body = document.getElementById("settingsBody");
  modal.hidden = false;
  body.replaceChildren(el("p", "hint", "Loading…"));

  const settings = state.settings || {};
  let doctor = null;
  try { doctor = await api("/api/doctor"); } catch { /* health stays empty */ }

  body.replaceChildren();

  const engineField = el("div", "field");
  engineField.append(el("span", "", "Engine"));
  const engineLine = el("div", "row-text");
  const engine = state.engine || {};
  engineLine.append(
    el("b", "", engine.installed ? `Vapor Forge ${engine.version || ""}` : "Not installed"),
    el("span", "", engine.installed
      ? (engine.steam_engine_loaded ? "loaded inside Steam" : "installed, not loaded — restart Steam")
      : "install it to make any game work"),
  );
  engineField.append(engineLine);
  body.append(engineField);

  if (!engine.installed) {
    const install = el("button", "btn primary", "Install engine");
    install.addEventListener("click", async () => {
      install.disabled = true;
      install.textContent = "Installing…";
      try { await post("/api/engine/install", {}); toast("Engine installed"); await loadLibrary(); openSettings(); }
      catch (error) { fail(error); install.disabled = false; install.textContent = "Install engine"; }
    });
    body.append(install);
  }

  const source = el("label", "field");
  source.append(el("span", "", "Where manifests come from"));
  const select = document.createElement("select");
  const options = state.sources.length ? state.sources : [{ name: settings.default_source || "auto", label: "Auto (all providers, in order)" }];
  for (const entry of options) {
    const option = document.createElement("option");
    option.value = entry.name;
    option.textContent = entry.label || entry.name;
    if ((settings.default_source || "auto") === entry.name) option.selected = true;
    select.append(option);
  }
  select.addEventListener("change", async () => {
    const chosen = options.find((entry) => entry.name === select.value);
    try {
      await post("/api/settings", { default_source: select.value, manifest_providers: (chosen && chosen.providers) || [] });
      state.settings.default_source = select.value;
      toast("Source updated");
    } catch (error) { fail(error); }
  });
  source.append(select, el("p", "hint", "One entry covers every provider in the chain — no URLs to type. Changes save on their own."));
  body.append(source);

  const country = el("label", "field");
  country.append(el("span", "", "Store region"));
  const countrySelect = document.createElement("select");
  const regions = [["US", "United States"], ["GB", "United Kingdom"], ["DE", "Germany"], ["UA", "Ukraine"], ["PL", "Poland"], ["TR", "Türkiye"], ["BR", "Brazil"], ["JP", "Japan"], ["KR", "Korea"], ["CN", "China"]];
  const current = (settings.country || "US").toUpperCase();
  for (const [code, label] of regions) {
    const option = document.createElement("option");
    option.value = code;
    option.textContent = `${label} (${code})`;
    if (code === current) option.selected = true;
    countrySelect.append(option);
  }
  countrySelect.addEventListener("change", async () => {
    try { await post("/api/settings", { country: countrySelect.value }); toast("Region updated"); }
    catch (error) { fail(error); }
  });
  country.append(countrySelect);
  body.append(country);

  const hubcap = el("label", "field");
  hubcap.append(el("span", "", "Hubcap key (optional, for key lookups)"));
  const hubcapInput = document.createElement("input");
  hubcapInput.type = "password";
  hubcapInput.placeholder = settings.hubcap_key ? "saved" : "smm_…";
  hubcapInput.value = "";
  const saveKey = el("button", "btn tiny", "Save");
  saveKey.addEventListener("click", async () => {
    try {
      await post("/api/hubcap", { action: "save", key: hubcapInput.value });
      toast("Key saved");
      hubcapInput.value = "";
    } catch (error) { fail(error); }
  });
  const keyRow = el("div", "field-row");
  keyRow.append(hubcapInput, saveKey);
  hubcap.append(keyRow);
  body.append(hubcap);

  const ryuu = el("label", "field");
  ryuu.append(el("span", "", "Ryuu auth key (plan source)"));
  const ryuuInput = document.createElement("input");
  ryuuInput.type = "password";
  ryuuInput.placeholder = settings.ryuu_key ? "saved" : "from generator.ryuu.lol";
  const saveRyuu = el("button", "btn tiny", "Save");
  saveRyuu.addEventListener("click", async () => {
    try {
      await post("/api/settings", { ryuu_key: ryuuInput.value.trim() });
      toast("Ryuu key saved");
      ryuuInput.value = "";
    } catch (error) { fail(error); }
  });
  const ryuuRow = el("div", "field-row");
  ryuuRow.append(ryuuInput, saveRyuu);
  ryuu.append(ryuuRow);
  body.append(ryuu);

  const selfTest = el("button", "btn tiny", "Run self-test");
  const selfTestBox = el("div", "checks");
  selfTest.addEventListener("click", async () => {
    selfTest.disabled = true;
    selfTestBox.replaceChildren(el("p", "hint", "Running…"));
    try {
      const report = await fetch("/api/selftest").then((response) => response.json());
      selfTestBox.replaceChildren(...(report.steps || []).map((step) => {
        const row = el("div", `check ${step.ok ? "ok" : "bad"}`);
        row.append(el("span", "check-dot"));
        const text = el("div", "check-text");
        text.append(el("b", "", step.name), el("span", "", step.detail || ""));
        row.append(text);
        return row;
      }));
      if (report.ok) toast("Self-test passed");
      else toast("Self-test failed", { kind: "bad" });
    } catch (error) {
      fail(error);
    } finally {
      selfTest.disabled = false;
    }
  });
  const selfTestRow = el("div", "field");
  selfTestRow.append(el("span", "", "Plan pipeline"));
  selfTestRow.append(selfTest);
  body.append(selfTestRow, selfTestBox);

  body.append(el("div", "divider"));
  body.append(el("h3", "", "Health"));

  if (doctor) {
    const checks = el("div", "checks");
    for (const item of doctor.items || []) {
      const row = el("div", `check ${item.status}`);
      row.append(el("span", "check-dot"));
      const text = el("div", "check-text");
      text.append(el("b", "", item.label), el("span", "", item.detail || ""));
      row.append(text);
      checks.append(row);
    }
    body.append(checks);
    body.append(el("p", "hint", doctor.ok ? "All critical checks pass." : `${doctor.warnings} warning(s) — fixes are in the Fixes row on the main screen.`));
  } else {
    body.append(el("p", "hint", "Could not read the health checks."));
  }
}

/* ---------- boot ---------- */

async function boot() {
  try {
    const sources = await api("/api/sources");
    state.sources = sources.sources || [];
  } catch { state.sources = []; }

  try { await loadLibrary(); } catch (error) { fail(error); }
  await loadDiagnose();

  renderFixes();

  const input = document.getElementById("q");
  input.addEventListener("keydown", (event) => { if (event.key === "Enter") runSearch(); });
  document.getElementById("emptyCta").addEventListener("click", () => {
    window.scrollTo({ top: 0, behavior: "smooth" });
    input.focus();
  });
  document.getElementById("searchGo").addEventListener("click", runSearch);

  document.getElementById("settingsBtn").addEventListener("click", openSettings);
  document.getElementById("importBtn").addEventListener("click", openImport);
  document.getElementById("importClose").addEventListener("click", closeImport);
  document.getElementById("importCancel").addEventListener("click", closeImport);
  document.getElementById("importRun").addEventListener("click", runImport);
  document.getElementById("importModal").addEventListener("click", (event) => {
    if (event.target.id === "importModal") closeImport();
  });
  const dropzone = document.getElementById("dropzone");
  const filePicker = document.getElementById("importFile");
  dropzone.addEventListener("click", () => filePicker.click());
  filePicker.addEventListener("change", () => readFileIntoBox(filePicker.files[0]));
  dropzone.addEventListener("dragover", (event) => { event.preventDefault(); dropzone.classList.add("over"); });
  dropzone.addEventListener("dragleave", () => dropzone.classList.remove("over"));
  dropzone.addEventListener("drop", async (event) => {
    event.preventDefault();
    dropzone.classList.remove("over");
    await readFileIntoBox(event.dataTransfer.files[0]);
  });
  document.getElementById("settingsClose").addEventListener("click", () => { document.getElementById("settings").hidden = true; });
  document.getElementById("settings").addEventListener("click", (event) => {
    if (event.target.id === "settings") document.getElementById("settings").hidden = true;
  });
  document.getElementById("scrim").addEventListener("click", closeDrawer);

  document.addEventListener("click", (event) => {
    const box = document.getElementById("results");
    if (box.hidden) return;
    if (!event.target.closest(".search")) box.hidden = true;
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      document.getElementById("results").hidden = true;
      document.getElementById("settings").hidden = true;
      closeImport();
      closeDrawer();
    }
  });

  try {
    const status = await api("/api/status");
    document.getElementById("footVersion").textContent = `ExxaTools ${status.version || ""}`.trim();
  } catch { /* footer keeps its default */ }

  document.body.dataset.ready = "1";
  setInterval(() => { loadLibrary().catch(() => {}); }, 30000);
}

boot();