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
}

$("recheck").addEventListener("click", checkGate);

$("toggle").addEventListener("click", async () => {
  try {
    if (running) {
      await invoke("stop_receiver");
      setRunning(false);
    } else {
      await invoke("start_receiver", { name: $("name").value });
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

listen("receiver-exit", () => setRunning(false));

checkGate();
