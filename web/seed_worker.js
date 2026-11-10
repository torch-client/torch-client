"use strict";

const queued = [];

const fail = (why) => {
  self.postMessage(String(why));
  self.onmessage = null;
};

const serve = (x) => (e) => {
  if (!(e.data instanceof ArrayBuffer) || e.data.byteLength === 0) {
    fail("a message that is not a buffer");
    return;
  }
  const bytes = new Uint8Array(e.data);
  const view = new DataView(e.data);
  if (bytes[0] === 0) {
    const len = bytes.length - 1;
    const ptr = x.seed_alloc(len) >>> 0;
    new Uint8Array(x.memory.buffer, ptr, len).set(bytes.subarray(1));
    if (x.seed_set_task(ptr, len) !== 1) fail("a task this build cannot read");
    return;
  }
  if (bytes[0] !== 1 || bytes.length !== 25) {
    fail("a message this build cannot read");
    return;
  }
  const job = view.getBigUint64(1, true);
  const start = view.getBigUint64(9, true);
  const end = view.getBigUint64(17, true);
  if (BigInt.asUintN(64, x.seed_task_id()) !== job) {
    fail("a slice of a job this worker holds no task for");
    return;
  }
  const count = x.seed_scan(start, end) >>> 0;
  const ptr = x.seed_out() >>> 0;
  const out = new ArrayBuffer(24 + count * 8);
  const head = new DataView(out);
  head.setBigUint64(0, job, true);
  head.setBigUint64(8, start, true);
  head.setBigUint64(16, end, true);
  new Uint8Array(out, 24).set(new Uint8Array(x.memory.buffer, ptr, count * 8));
  self.postMessage(out, [out]);
};

self.onmessage = async (first) => {
  self.onmessage = (e) => queued.push(e);
  let handler;
  try {
    const instance = await WebAssembly.instantiate(first.data.module, {});
    handler = serve(instance.exports);
  } catch (e) {
    queued.length = 0;
    fail(e);
    return;
  }
  self.onmessage = handler;
  for (const e of queued) {
    if (self.onmessage !== handler) break;
    handler(e);
  }
  queued.length = 0;
};
