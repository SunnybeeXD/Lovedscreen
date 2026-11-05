async function sha256b64(text) {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return btoa(String.fromCharCode(...new Uint8Array(digest)));
}

function connectObs(url, password) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    const pending = new Map();
    let counter = 0;
    const request = (requestType, requestData = {}) =>
      new Promise((res, rej) => {
        const requestId = String(++counter);
        pending.set(requestId, { res, rej });
        ws.send(JSON.stringify({ op: 6, d: { requestType, requestId, requestData } }));
      });
    ws.onmessage = async (m) => {
      const { op, d } = JSON.parse(m.data);
      if (op === 0) {
        const identify = { rpcVersion: 1 };
        if (d.authentication) {
          const secret = await sha256b64(password + d.authentication.salt);
          identify.authentication = await sha256b64(secret + d.authentication.challenge);
        }
        ws.send(JSON.stringify({ op: 1, d: identify }));
      } else if (op === 2) {
        resolve({ request, close: () => ws.close() });
      } else if (op === 7) {
        const p = pending.get(d.requestId);
        pending.delete(d.requestId);
        if (d.requestStatus.result) p.res(d.responseData);
        else p.rej(new Error(d.requestStatus.comment || "OBS request failed"));
      }
    };
    ws.onerror = () => reject(new Error("Cannot reach OBS. Enable WebSocket server in OBS (Tools, WebSocket Server Settings)."));
    ws.onclose = (e) => {
      if (e.code === 4009) reject(new Error("Wrong OBS WebSocket password"));
    };
  });
}

async function setupInput(obs, scene, name, kind, extra) {
  let exists = true;
  try {
    await obs.request("GetInputSettings", { inputName: name });
  } catch {
    exists = false;
  }
  if (!exists) {
    await obs.request("CreateInput", { sceneName: scene, inputName: name, inputKind: kind, inputSettings: {} });
  }
  const { propertyItems } = await obs.request("GetInputPropertiesListPropertyItems", {
    inputName: name,
    propertyName: "window",
  });
  const item = propertyItems.find((i) => /uxplay\.exe/i.test(i.itemName));
  if (!item) throw new Error("Mirror window not found. Start mirroring from your iPhone first.");
  await obs.request("SetInputSettings", { inputName: name, inputSettings: { window: item.itemValue, ...extra } });
}

async function addToObs(url, password) {
  const obs = await connectObs(url, password);
  try {
    const { currentProgramSceneName } = await obs.request("GetCurrentProgramScene");
    await setupInput(obs, currentProgramSceneName, "Lovedscreen Video", "window_capture", { method: 2, cursor: false });
    await setupInput(obs, currentProgramSceneName, "Lovedscreen Audio", "wasapi_process_output_capture", {});
  } finally {
    obs.close();
  }
}

document.getElementById("obs-add").addEventListener("click", async () => {
  const msg = document.getElementById("obs-msg");
  msg.textContent = "Connecting";
  try {
    await addToObs(document.getElementById("obs-url").value, document.getElementById("obs-pass").value);
    msg.textContent = "Added Lovedscreen Video and Lovedscreen Audio to the current scene";
  } catch (e) {
    msg.textContent = e.message;
  }
});
