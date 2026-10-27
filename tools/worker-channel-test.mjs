import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { MessageChannel, Worker } from 'node:worker_threads';
import { runInNewContext } from 'node:vm';
import { applyExperiments, HW } from '../web/wasm/experiments.mjs';
import { createPacing } from '../web/wasm/pacing.mjs';

const source = (await readFile(new URL('../web/wasm/worker.js', import.meta.url), 'utf8')).replace(/^import .*;\n/gm, '');
async function harness(cost = 0, additions = [], experiments = [], overrides = {}, init = { frameAck: true }) {
  let wall = 0, cycles = 0, input = 0, frame = null, immediate = 0;
  const timers = [], messages = [], channels = [], runs = [], deletedNetworks = [];
  let delivered;
  // Native ports dispatch the actual worker continuation asynchronously. Only the clock
  // and timers are controlled, so assertions do not depend on host machine speed.
  class Channel extends MessageChannel {
    constructor() {
      super(); channels.push(this);
      const post = this.port2.postMessage.bind(this.port2);
      this.port2.postMessage = value => { immediate++; post(value); };
      this.port1.addEventListener('message', () => { delivered?.(); delivered = null; });
    }
  }
  const wasm = {
    memory: new WebAssembly.Memory({ initial: 1 }),
    esp32sim_alloc: () => 128, esp32sim_free() {}, esp32sim_new: () => 1,
    esp32sim_delete() {}, esp32sim_set_jit() {}, esp32sim_boot: () => 0,
    esp32sim_net_new: () => 2, esp32sim_net_delete(net) { deletedNetworks.push(net); },
    esp32sim_net_add: () => additions.shift() ?? 0,
    esp32sim_cpu_hz: () => 240e6, esp32sim_cycles: () => cycles,
    esp32sim_insns: () => cycles, esp32sim_in_text() { input++; },
    esp32sim_run(_emu, amount) { runs.push({ amount, input }); cycles += amount; wall += amount * cost; return 0; },
    esp32sim_out_take() {
      if (frame === null) return 0;
      new Uint8Array(this.memory.buffer).set([1, frame]); frame = null; return 1;
    },
    esp32sim_out_kind: () => 2, esp32sim_out_ptr: () => 0, esp32sim_out_len: () => 2,
  };
  const context = {
    applyExperiments, createPacing, createJitHost: () => ({ imports: {} }), TextEncoder, TextDecoder, MessageChannel: Channel,
    performance: { now: () => wall, timeOrigin: 0 }, Date, postMessage: m => messages.push(m),
    WebAssembly: { instantiate: async () => ({ instance: { exports: wasm } }) },
    setTimeout: (callback, delay) => timers.push({ callback, delay }),
  };
  Object.assign(wasm, overrides);
  runInNewContext(source, context);
  const send = data => context.onmessage({ data });
  await send({ op: 'init', ...init }); await send({ op: 'create', board: 'test', experiments });
  return {
    send, timers, messages, runs, deletedNetworks, get immediate() { return immediate; },
    setWall(value) { wall = value; }, get wall() { return wall; },
    nextMessage() { return new Promise(resolve => { delivered = resolve; }); },
    frame(id) { frame = id; runInNewContext('drain()', context); },
    close() { channels.forEach(c => { c.port1.close(); c.port2.close(); }); },
  };
}

for (const cost of [0, 1 / 24_000_000]) {
  const h = await harness(cost);
  try {
    await h.send({ op: 'start' });
    for (let turn = 0; turn < 20; turn++) {
      assert.equal(h.timers.length, 1, 'caught-up guest schedules one sleep');
      const timer = h.timers.shift();
      assert.ok(timer.delay >= 1 && timer.delay <= 20);
      h.setWall(h.wall + timer.delay); timer.callback();
      assert.equal(h.immediate, 0, 'cheap guest never spins on the channel');
    }
    assert.ok(h.runs.length > 0, 'sleep still advances the guest');
  } finally { h.close(); }
}
{
  const h = await harness(1 / 16_000);
  try {
    await h.send({ op: 'start' });
    h.setWall(100);
    await h.send({ op: 'text', data: 'touch down' });
    const next = h.nextMessage();
    h.timers.shift().callback();
    assert.equal(h.wall, 108, 'active drawing yields after eight ms');
    assert.equal(h.immediate, 1, 'unfinished turn uses the native channel');
    assert.equal(h.timers.length, 0, 'no clamped timer while behind');
    await h.send({ op: 'text', data: 'touch move' });
    await next;
    assert.equal(h.wall, 116, 'native channel executes another interactive turn');
    assert.equal(h.runs.at(-1).input, 2, 'input queued between turns reaches the next run');
    assert.ok(h.runs.every(r => r.input && r.amount <= 64_000), 'input reaches bounded guest slices');
    await h.send({ op: 'stop' });
    const stopped = h.runs.length;
    await h.nextMessage();
    assert.equal(h.runs.length, stopped, 'queued immediate yield respects stop');
  } finally { h.close(); }
}

// A continuously lagging guest must periodically end its self-posted port chain.
// Hosts are allowed to drain one port before servicing the parent control port;
// exercising more than one short turn is essential to catch this starvation.
{
  const h = await harness(1 / 16_000);
  try {
    await h.send({op:'start'}); h.setWall(100);
    let delivered = h.nextMessage(); h.timers.shift().callback();
    while (!h.timers.length) { await delivered; delivered = h.nextMessage(); }
    assert.ok(h.runs.length > 1, 'regression executes sustained catch-up work');
    assert.ok(h.immediate <= 8, 'self-posted continuation chain stays bounded');
    assert.equal(h.timers.length,1,'catch-up gives other event sources a timer boundary');
    const boundary = h.runs.length;
    await h.send({op:'stop'});
    h.timers.shift().callback();
    assert.equal(h.runs.length,boundary,'stop remains effective across the fairness timer');
    assert.equal(h.messages.at(-1).stopped,0);
  } finally { h.close(); }
}

// Exercise a real parent control port competing with the worker's self-posting
// continuation port. A deliberately slow guest keeps catch-up work outstanding.
{
  const script = `
    import {parentPort, workerData, MessageChannel} from 'node:worker_threads';
    import {runInNewContext} from 'node:vm';
    const {createPacing} = await import(workerData.pacing);
    let cycles = 0, first = true;
    const sleeper = new Int32Array(new SharedArrayBuffer(4));
    const wasm = {
      memory: new WebAssembly.Memory({initial:1}), esp32sim_alloc:()=>128,
      esp32sim_free(){}, esp32sim_new:()=>1, esp32sim_set_jit(){}, esp32sim_boot:()=>0,
      esp32sim_cpu_hz:()=>240e6, esp32sim_cycles:()=>cycles, esp32sim_insns:()=>cycles,
      esp32sim_run(_emu,amount){
        Atomics.wait(sleeper,0,0,30); cycles+=Math.min(amount,64000);
        if(first){first=false;parentPort.postMessage({firstSlice:true});} return 0;
      }, esp32sim_out_take:()=>0,
    };
    const context = {createPacing,createJitHost:()=>({imports:{}}),applyExperiments(){},
      TextEncoder,TextDecoder,MessageChannel,performance,Date,setTimeout,
      postMessage:m=>parentPort.postMessage(m),
      WebAssembly:{instantiate:async()=>({instance:{exports:wasm}})}};
    runInNewContext(workerData.source,context);
    parentPort.on('message',data=>context.onmessage({data}));
    parentPort.postMessage({testReady:true});
  `;
  const thread = new Worker(new URL(`data:text/javascript,${encodeURIComponent(script)}`), {
    workerData:{source,pacing:new URL('../web/wasm/pacing.mjs',import.meta.url).href},
  });
  let deadline;
  try {
    await new Promise((resolve,reject)=>{
      deadline=setTimeout(()=>reject(new Error('Continuous catch-up starved the real stop control port')),5000);
      thread.on('error',reject);
      thread.on('message',m=>{
        if(m.testReady)thread.postMessage({op:'init'});
        else if(m.ready)thread.postMessage({op:'create',board:'test'});
        else if(m.created)thread.postMessage({op:'start'});
        else if(m.firstSlice)thread.postMessage({op:'stop'});
        else if(m.stopped!==undefined){assert.equal(m.stopped,0);resolve();}
      });
    });
  } finally { clearTimeout(deadline); await thread.terminate(); }
}

// Execute each supported consumer's real handler against the real worker drain/ACK
// protocol. Rendering is replaced by a recorder; messages retain FIFO delivery.
for (const [path, end] of [['../web/emu.js', '  const queue ='], ['../tools/browser-benchmark/response.mjs', 'worker.onerror']]) {
  const text = await readFile(new URL(path, import.meta.url), 'utf8');
  const handler = text.slice(text.indexOf('worker.onmessage ='), text.indexOf(end));
  const h = await harness(), rendered = [], acks = [];
  const record = buf => rendered.push(new Uint8Array(buf)[1]);
  const consumer = { worker: { postMessage(m) { acks.push(m); } }, onmessage: record, frame: record, window: {}, waiters: [] };
  runInNewContext(handler, consumer);
  try {
    for (let id = 1; id <= 6; id++) {
      h.frame(id);
      const message = h.messages.findLast(m => m.bin);
      assert.equal(new Uint8Array(message.bin)[1], id, `${path} receives every sequential frame`);
      consumer.worker.onmessage({ data: message });
      assert.equal(acks.length, 1, `${path} ACKs after rendering`);
      assert.equal(rendered.at(-1), id);
      await h.send(acks.shift());
    }
    assert.equal(rendered.length, 6);
    consumer.onmessage = consumer.frame = () => { throw new Error('draw failed'); };
    for (let id = 7; id <= 9; id++) {
      h.frame(id);
      const message = h.messages.findLast(m => m.bin);
      assert.equal(new Uint8Array(message.bin)[1], id, 'failed draws do not exhaust credits');
      assert.throws(() => consumer.worker.onmessage({ data: message }), /draw failed/);
      assert.equal(acks.length, 1, 'failed draw still returns its credit');
      await h.send(acks.shift());
    }
  } finally { h.close(); }
}

// Cached pages and custom consumers that never acknowledge must keep receiving frames.
for (const frameAck of [undefined, false, 'true']) {
  const h = await harness(0, [], [], {}, { frameAck });
  try {
    for (let id = 1; id <= 6; id++) h.frame(id);
    assert.deepEqual(h.messages.filter(m => m.bin).map(m => new Uint8Array(m.bin)[1]), [1, 2, 3, 4, 5, 6]);
    assert.ok(h.messages.filter(m => m.bin).every(m => !m.ack));
  } finally { h.close(); }
}
{
  const h = await harness(0, [], [], {}, { frameAck: true, touchTrace: true });
  try {
    h.frame(1); h.frame(2); h.setWall(10); h.frame(3); h.setWall(20); h.frame(4);
    assert.equal(h.messages.filter(m => m.bin).length, 2, 'negotiated window remains bounded');
    h.setWall(30); await h.send({ op: 'frame-ack' });
    const message = h.messages.at(-1);
    assert.equal(new Uint8Array(message.bin)[1], 4, 'only newest retained frame is released');
    assert.equal(message.frameTrace.stage, 'worker-frame');
    assert.equal(message.frameTrace.atMs, 20, 'retained trace describes original frame production');
    assert.equal(message.frameTrace.cycles, 0);
  } finally { h.close(); }
}

for (const op of ['create', 'net-create']) {
  const h = await harness();
  try {
    h.frame(11); h.frame(22); h.frame(33);
    assert.equal(h.messages.filter(m => m.bin).length, 2);
    await h.send({ op, board: 'replacement', nodes: [] });
    const boundary = h.messages.length;
    await h.send({ op: 'frame-ack' });
    assert.equal(h.messages.length, boundary, `${op} discards the old pending frame`);
    if (op === 'net-create') await h.send({ op: 'create', board: 'replacement' });
    h.frame(44); h.frame(55);
    assert.equal(new Uint8Array(h.messages.findLast(m => m.bin).bin)[1], 44, 'late ACK does not reset all outstanding credits');
    await h.send({ op: 'frame-ack' });
    assert.equal(new Uint8Array(h.messages.findLast(m => m.bin).bin)[1], 55, 'remaining old ACK releases exactly one slot');
  } finally { h.close(); }
}
for (const failure of [-1, 0xffffffff]) {
  const h = await harness(0, [0, failure]);
  try {
    await h.send({ op: 'net-create', nodes: [{}, { flash_mb: 4096 }] });
    assert.equal(h.messages.at(-1).created, false, 'partial network creation reports failure');
    assert.equal(h.messages.at(-1).nodes, 0, 'no missing node is reported as created');
    assert.deepEqual(h.deletedNetworks, [2], 'failed creation releases the entire partial network');
  } finally { h.close(); }
}
console.log('worker native-channel, consumer ACK, replacement and network failure tests passed');

// Unsafe dispatch must never call arbitrary exports or boot a partial configuration.
for (const experiments of [
  [['esp32sim_delete']], [['unknown']], [['toString']], null, {}, [null],
  [['esp32sim_set_quantum', 0]], [['esp32sim_set_quantum', NaN]],
  [['esp32sim_set_quantum', '64']], [['esp32sim_set_quantum', 64, 1]],
  [['esp32sim_set_approximate_jit_cache', 96, 160, 4]],
  [...HW, ['esp32sim_set_quantum', 64]],
]) {
  let deleted = 0, booted = 0;
  const h = await harness(0, [], experiments, {
    esp32sim_delete() { deleted++; }, esp32sim_boot() { booted++; return 0; },
  });
  try {
    await h.send({ op: 'start' });
    assert.equal(deleted, 0, 'dangerous delete experiment never dispatched');
    assert.equal(booted, 0, 'invalid experiments never boot');
    assert.equal(h.messages.at(-1).started, false, 'start failure acknowledged');
    assert.equal(h.runs.length, 0);
  } finally { h.close(); }
}
for (const failure of ['missing', 'reject', 'throw']) {
  let booted = 0, later = 0;
  const exports = Object.fromEntries(HW.map(([name]) => [name, () => 0]));
  if (failure === 'missing') delete exports[HW[1][0]];
  else exports[HW[1][0]] = () => { if (failure === 'throw') throw Error('setter trap'); return 1; };
  exports[HW[2][0]] = () => { later++; return 0; };
  const h = await harness(0, [], HW, {...exports, esp32sim_boot() { booted++; return 0; }});
  try {
    await h.send({ op: 'start' });
    assert.equal(h.messages.at(-1).started, false, failure);
    assert.equal(booted, 0); assert.equal(later, 0); assert.equal(h.runs.length, 0);
  } finally { h.close(); }
}
{
  const applied = [];
  const h = await harness(0, [], HW, Object.fromEntries(HW.map(([name]) => [name, (_emu, ...args) => { applied.push([name, ...args]); return 0; }])));
  try {
    await h.send({op: 'start'});
    assert.equal(h.messages.at(-1).started, true);
    assert.deepEqual(applied, HW);
  } finally { h.close(); }
}
console.log('worker native-channel, consumer ACK, replacement and experiment tests passed');

for (const [op, exportName, label] of [['stub', 'esp32sim_stub_spec', 'stub'], ['wifi', 'esp32sim_wifi', 'WiFi']]) {
  let booted = 0;
  const h = await harness(0, [], [], { [exportName]: () => 1, esp32sim_boot() { booted++; return 0; } });
  try {
    await h.send({ op, spec: 'invalid' });
    await h.send({ op: 'start' });
    assert.equal(booted, 0, 'failed setup never boots');
    assert.equal(h.messages.at(-1).started, false);
    assert.match(h.messages.at(-1).error, new RegExp(label));
    await h.send({ op: 'create', board: 'test' });
    await h.send({ op: 'start' });
    assert.equal(h.messages.at(-1).started, true, 'new emulator clears setup failures');
  } finally { h.close(); }
}
console.log('worker rejected stub and WiFi boot status tests passed');

for (const smoothDisplay of [undefined, false, true]) {
  const calls = [];
  const h = await harness(0, [], [], {
    esp32sim_set_smooth_display(_emu, on) { calls.push(['display', on]); return 0; },
    esp32sim_boot() { calls.push(['boot']); return 0; },
  });
  try {
    await h.send({ op: 'create', board: 'test', smoothDisplay });
    await h.send({ op: 'start' });
    assert.deepEqual(calls, smoothDisplay ? [['display', 1], ['boot']] : [['boot']], 'only explicit smooth display opt-in configures before boot');
  } finally { h.close(); }
}
for (const setter of [undefined, () => 1, () => { throw Error('unsupported'); }]) {
  let booted = false;
  const h = await harness(0, [], [], { esp32sim_set_smooth_display: setter, esp32sim_boot() { booted = true; return 0; } });
  try {
    await h.send({ op: 'create', board: 'test', smoothDisplay: true });
    await h.send({ op: 'start' });
    assert.equal(booted, false, 'unsupported smooth display must not silently boot with a different policy');
    assert.equal(h.messages.at(-1).started, false);
  } finally { h.close(); }
}
console.log('worker smooth display configuration tests passed');

// The host snapshot API never steps the guest, and opt-in watching only transfers
// changed 4 KiB pages. A stop forces the final delta before its acknowledgement.
{
  const memory=new WebAssembly.Memory({initial:1}),bytes=new Uint8Array(memory.buffer);bytes.fill(255,4096,12288);
  const h=await harness(0,[],[],{memory,esp32sim_flash_ptr:()=>4096,esp32sim_flash_len:()=>8192});
  try {
    await h.send({op:'flash-snapshot',requestId:1});
    const snapshot=h.messages.at(-1);assert.equal(snapshot.flashReply,1);assert.equal(new Uint8Array(snapshot.data)[0],255);assert.equal(h.runs.length,0);
    await h.send({op:'flash-watch',requestId:2});bytes[4096+5000]=17;
    await h.send({op:'flash-flush',requestId:3});
    let deltas=h.messages.filter(m=>m.flashDelta);assert.equal(deltas.length,1);assert.equal(deltas[0].flashDelta.watchId,2);
    assert.equal(deltas[0].flashDelta.pages.length,1);assert.equal(deltas[0].flashDelta.pages[0].offset,4096);assert.equal(new Uint8Array(deltas[0].flashDelta.pages[0].data)[904],17);
    assert.equal(h.messages.at(-1).flashReply,3);assert.equal(h.runs.length,0);
    await h.send({op:'flash-flush',requestId:4});assert.equal(h.messages.filter(m=>m.flashDelta).length,1,'unchanged pages are not resent');
    bytes[4100]=42;await h.send({op:'stop'});assert.equal(h.messages.at(-2).flashDelta.pages[0].offset,0);assert.equal(h.messages.at(-1).stopped,0);
    await h.send({op:'net-create',nodes:[]});bytes[4200]=9;await h.send({op:'stop'});deltas=h.messages.filter(m=>m.flashDelta);assert.equal(deltas.length,2,'switching to a network drops the old single-device watcher');
  } finally {h.close();}
}
console.log('worker flash snapshot, page deltas, forced flush order and replacement tests passed');
