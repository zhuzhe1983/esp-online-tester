#!/usr/bin/env node
// Deterministic host-time input checks; these make no claim about guest CPU-cycle timing.
import assert from 'node:assert/strict';
import { createHumanButtonController } from '../web/platform/scene-view.js';

function fixture({ send } = {}) {
  let time = 0, sequence = 0, running = true;
  const timers = new Map(), events = [], errors = [];
  const controller = createHumanButtonController({
    send: event => { events.push({ ...event, time }); return send?.(event); },
    isRunning: () => running,
    now: () => time,
    setTimer: (callback, delay) => { const id = ++sequence; timers.set(id, { callback, at: time + delay }); return id; },
    clearTimer: id => timers.delete(id),
    onError: error => errors.push(error),
  });
  async function advance(milliseconds) {
    const target = time + milliseconds;
    while (true) {
      const next = [...timers].filter(([, timer]) => timer.at <= target).sort((a, b) => a[1].at - b[1].at)[0];
      if (!next) break;
      time = next[1].at; timers.delete(next[0]); next[1].callback();
      await Promise.resolve(); await Promise.resolve();
    }
    time = target; await Promise.resolve(); await Promise.resolve();
  }
  return {
    controller, events, timers, errors, advance,
    setRunning: value => { running = value; },
    down: (source = 'pointer', id = 'button-a') => controller.input({ id, pressed: true }, source),
    up: (source = 'pointer', id = 'button-a') => controller.input({ id, pressed: false }, source),
  };
}

const tests = [
  ['short clicks remain down for 150 host milliseconds', async () => {
    const test = fixture(); await test.down(); await test.up();
    assert.equal(test.events.length, 1);
    await test.advance(149); assert.equal(test.events.length, 1);
    await test.advance(1); assert.deepEqual(test.events.map(event => [event.pressed, event.time]), [[true, 0], [false, 150]]);
    assert.equal(test.timers.size, 0);
  }],
  ['long presses remain held until the human releases them', async () => {
    const test = fixture(); await test.down('key:KeyA'); await test.advance(700);
    assert.equal(test.events.length, 1);
    await test.up('key:KeyA'); assert.deepEqual(test.events.map(event => [event.pressed, event.time]), [[true, 0], [false, 700]]);
  }],
  ['pointerup plus lostpointercapture does not release twice', async () => {
    const test = fixture(); await test.down(); await test.up(); await test.up(); await test.advance(500);
    assert.deepEqual(test.events.map(event => event.pressed), [true, false]);
  }],
  ['keyboard and pointer holds on one button share a single physical state', async () => {
    const test = fixture(); await test.down('key:KeyA'); await test.down('pointer:1');
    await test.up('pointer:1'); await test.advance(300); assert.equal(test.events.length, 1);
    await test.up('key:KeyA'); assert.deepEqual(test.events.map(event => event.pressed), [true, false]);
  }],
  ['a former session timer cannot release a button in the replacement session', async () => {
    const test = fixture(); await test.down(); await test.up(); await test.advance(50);
    test.controller.invalidate(); await test.down(); await test.advance(100);
    assert.deepEqual(test.events.map(event => [event.pressed, event.time]), [[true, 0], [true, 50]]);
    await test.up(); await test.advance(49); assert.equal(test.events.length, 2);
    await test.advance(1); assert.deepEqual(test.events.at(-1), { type: 'button', id: 'button-a', pressed: false, time: 200 });
  }],
  ['blur and UI pause can force immediate release and cancel a pending pulse', async () => {
    const test = fixture(); await test.down(); await test.up(); await test.advance(20);
    await test.controller.releaseAll(); assert.equal(test.events.at(-1).time, 20); assert.equal(test.events.at(-1).pressed, false);
    await test.advance(500); assert.equal(test.events.length, 2); assert.equal(test.timers.size, 0);
  }],
  ['an externally paused machine receives its outstanding release on resume', async () => {
    const test = fixture(); await test.down(); await test.up(); test.controller.suspend(); test.setRunning(false);
    await test.advance(500); assert.equal(test.events.length, 1);
    test.setRunning(true); await test.controller.releaseAll(); assert.equal(test.events.at(-1).pressed, false);
    assert.equal(test.events.at(-1).time, 500);
  }],
  ['the minimum pulse starts after the runtime accepts the down event', async () => {
    let acceptDown;
    const test = fixture({ send: event => event.pressed ? new Promise(resolve => { acceptDown = resolve; }) : undefined });
    const down = test.down(); await Promise.resolve(); const up = test.up(); await test.advance(500);
    assert.equal(test.events.length, 1); acceptDown(); await down; await up;
    await test.advance(149); assert.equal(test.events.length, 1);
    await test.advance(1); assert.equal(test.events.at(-1).time, 650);
  }],
  ['a late down acknowledgment cannot schedule a release after session invalidation', async () => {
    let acceptDown;
    const test = fixture({ send: event => event.pressed ? new Promise(resolve => { acceptDown = resolve; }) : undefined });
    const down = test.down(); await Promise.resolve(); const up = test.up();
    test.controller.invalidate(); acceptDown(); await down; await up; await test.advance(500);
    assert.deepEqual(test.events.map(event => event.pressed), [true]); assert.equal(test.timers.size, 0);
  }],
  ['invalidation before dispatch cancels the old down event too', async () => {
    const test = fixture(); const down = test.down(); test.controller.invalidate(); await down; await test.advance(500);
    assert.equal(test.events.length, 0); assert.equal(test.timers.size, 0);
  }],
];

for (const [name, test] of tests) { await test(); console.log(`ok ${name}`); }
console.log(`Passed ${tests.length} human UI input checks (injected host clock; no browser or firmware assets required).`);
