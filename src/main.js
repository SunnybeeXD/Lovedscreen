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
  $("fps").disabled = running;
}

$("toggle").addEventListener("click", async () => {
  try {
    if (running) {
      $("toggle").disabled = true;
      setState("Stopping", "");
      await invoke("stop_receiver");
    } else {
      await invoke("start_receiver", { name: $("name").value, fps: Number($("fps").value) });
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
  if (log.textContent.length > 60000) log.textContent = log.textContent.slice(-40000);
  log.scrollTop = log.scrollHeight;
});

listen("receiver-connection", (e) => {
  if (running) setState(e.payload ? "Mirroring" : "Running", "running");
});

listen("receiver-exit", () => setRunning(false));

$("copy").addEventListener("click", async () => {
  const text = await invoke("read_log");
  try {
    await navigator.clipboard.writeText(text);
    $("copy").textContent = "Copied";
  } catch {
    $("log").textContent = text;
    $("copy").textContent = "Select the text below and copy it";
  }
  setTimeout(() => ($("copy").textContent = "Copy log"), 4000);
});
