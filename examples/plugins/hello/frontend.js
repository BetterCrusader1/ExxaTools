// Runs inside its own isolated world in the Steam main window. `exxa` is provided
// by the host; nothing on the page itself is visible to this script.
//
// The section is mounted synchronously so a slow host call can never delay it;
// the status line fills in when the bridge answers.
const line = document.createElement("div");
line.style.cssText = "flex:1;font-size:11.5px;color:#93A0AE;line-height:1.4";
line.textContent = "hello — asking the host…";

const button = document.createElement("button");
button.className = "vd-mini";
button.textContent = "Say hi";
button.style.flex = "none";
button.addEventListener("click", async () => {
  try {
    await exxa.config.set("greeting", "hi from the plugin");
    exxa.ui.toast("Hello from the example plugin");
  } catch (error) {
    exxa.ui.toast("Host refused: " + error);
  }
});

const row = document.createElement("div");
row.style.cssText = "display:flex;align-items:center;gap:10px";
row.append(line, button);
exxa.ui.section("Example", row);

exxa.steam.status().then((status) => {
  const plans = status.plans === 1 ? "1 plan" : `${status.plans} plans`;
  line.textContent = `Steam ${status.steam_running ? "running" : "closed"} · ${plans}`;
}).catch((error) => {
  line.textContent = "hello — host refused: " + error;
});
