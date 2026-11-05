const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
let running = false;

function setRunning(value) {
  running = value;
  $("toggle").textContent = running ? "Stop receiver" : "Start receiver";
  $("toggle").disabled = false;
  $("state").textContent = running ? "Running" : "Stopped";
  $("name").disabled = running;
}

$("toggle").addEventListener("click", async () => {
  try {
    if (running) {
      $("toggle").disabled = true;
      $("state").textContent = "Stopping";
      await invoke("stop_receiver");
    } else {
      await invoke("start_receiver", { name: $("name").value });
      setRunning(true);
    }
  } catch (e) {
    $("toggle").disabled = false;
    $("state").textContent = String(e);
  }
});

listen("receiver-line", (e) => {
  const log = $("log");
  log.textContent += e.payload + "\n";
  log.scrollTop = log.scrollHeight;
});

listen("receiver-exit", () => setRunning(false));
