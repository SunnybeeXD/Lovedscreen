const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
let running = false;

function setState(text, kind) {
  $("state").textContent = text;
  $("pill").className = "pill " + kind;
}

function setRunning(value) {
  running = value;
  $("toggle").textContent = running ? "Stop receiver" : "Start receiver";
  $("toggle").classList.toggle("stop", running);
  $("toggle").disabled = false;
  setState(running ? "Running" : "Stopped", running ? "running" : "");
  $("name").disabled = running;
}

$("toggle").addEventListener("click", async () => {
  try {
    if (running) {
      $("toggle").disabled = true;
      setState("Stopping", "");
      await invoke("stop_receiver");
    } else {
      await invoke("start_receiver", { name: $("name").value });
      setRunning(true);
    }
  } catch (e) {
    $("toggle").disabled = false;
    setState(String(e), "error");
  }
});

listen("receiver-line", (e) => {
  const log = $("log");
  log.textContent += e.payload + "\n";
  log.scrollTop = log.scrollHeight;
});

listen("receiver-exit", () => setRunning(false));
