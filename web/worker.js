import init, { start_mesh_worker } from "./torch-client.js";

const queued = [];

const buffer = (e) => queued.push(e);

self.onmessage = async (first) => {
  self.onmessage = buffer;
  try {
    await init({ module_or_path: first.data.module });
    start_mesh_worker(new Uint8Array(first.data.assets));
  } catch (e) {
    self.postMessage(`${e}`);
    self.onmessage = null;
    return;
  }
  const handler = self.onmessage;
  if (handler === buffer) {
    self.onmessage = null;
    queued.length = 0;
    return;
  }
  for (const e of queued) handler(e);
  queued.length = 0;
};
