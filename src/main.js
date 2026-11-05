const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
let running = false;

const messages = {
  missing: "Bonjour is not installed. Install it, then press Recheck.",
  stopped: "The Bonjour Service is installed but not running. Start it in Windows Services, then press Recheck.",
};

async function checkGate() {
  const status = await invoke("bonjour_status");
  const ok = status === "running";
  $("gate").hidden = ok;
  $("app").hidden = !ok;
  if (!ok) $("gate-msg").textContent = messages[status];
}

function setRunning(value) {
  running = value;
  $("toggle").textContent = running ? "Stop receiver" : "Start receiver";
  $("state").textContent = running ? "Running" : "Stopped";
  $("name").disabled = running;
  $("record").disabled = running;
  $("toggle").disabled = false;
}

$("recheck").addEventListener("click", checkGate);

$("toggle").addEventListener("click", async () => {
  try {
    if (running) {
      $("toggle").disabled = true;
      $("state").textContent = "Stopping";
      await invoke("stop_receiver");
    } else {
      await invoke("start_receiver", { name: $("name").value, record: $("record").checked });
      setRunning(true);
    }
  } catch (e) {
    $("state").textContent = String(e);
  }
});

listen("receiver-line", (e) => {
  const log = $("log");
  log.textContent += e.payload + "\n";
  log.scrollTop = log.scrollHeight;
});

listen("receiver-exit", (e) => {
  setRunning(false);
  if (e.payload) $("state").textContent = "Saved: " + e.payload;
});

checkGate();
