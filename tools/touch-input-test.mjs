import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import vm from 'node:vm';

const source = readFileSync(new URL('../web/touch-input.js', import.meta.url), 'utf8');
function panel() {
  let clock = 0, nextTimer = 0;
  const timers = new Map(), messages = [];
  const canvas = Object.assign(new EventTarget(), {
    width: 368, height: 448,
    getBoundingClientRect: () => ({left: 10, top: 20, width: 368, height: 448}),
    setPointerCapture() {},
  });
  const context = {
    window: {}, performance: {timeOrigin: 1000, now: () => clock},
    getComputedStyle: () => ({}),
    setTimeout: (callback, delay) => { const id = ++nextTimer; timers.set(id, {callback, at: clock + delay}); return id; },
    clearTimeout: id => timers.delete(id),
  };
  vm.runInNewContext(source, context);
  context.window.installTouchInput(canvas, data => messages.push(JSON.parse(data)));
  return {
    messages,
    event(type, x, y = 100, pointerId = 1) {
      canvas.dispatchEvent(Object.assign(new Event(type), {clientX: x + 10, clientY: y + 20, pointerId}));
    },
    advance(ms) {
      const end = clock + ms;
      while (true) {
        const next = [...timers].filter(([,timer]) => timer.at <= end).sort((a,b) => a[1].at-b[1].at)[0];
        if (!next) break;
        const [id, timer] = next; clock = timer.at; timers.delete(id); timer.callback();
      }
      clock = end;
    },
  };
}
const touch = (x, down = 1, y = 100) => ({t: 'touch', x, y, down});

test('a dense burst delivers its newest point without needing another event', () => {
  const p = panel();
  p.event('pointerdown', 10);
  for (let x=11;x<=39;x++) { p.advance(1); p.event('pointermove', x); }
  assert.deepEqual(p.messages, [touch(10)]);
  p.advance(11);
  assert.deepEqual(p.messages, [touch(10),touch(39)]);
  p.advance(200);
  assert.equal(p.messages.length, 2, 'a stationary held contact does not repeat old moves');
});

for (const release of ['pointerup', 'pointercancel', 'lostpointercapture']) {
  test(`${release} preserves the pending point and ends the contact exactly once`, () => {
    const p = panel();
    p.event('pointerdown', 10);
    p.advance(5); p.event('pointermove', 20);
    p.event(release, 25);
    p.event('pointerup', 25);
    p.advance(100);
    assert.deepEqual(p.messages, [touch(10),touch(20),touch(25,0)]);
    p.event('pointermove', 30);
    assert.equal(p.messages.length, 3, 'movement after release cannot resurrect a contact');
  });
}

test('rapid successive contacts stay ordered and a second pointer cannot release the first', () => {
  const p = panel();
  p.event('pointerdown', 10);
  p.event('pointerdown', 200, 100, 2);
  p.event('pointermove', 220, 100, 2);
  p.event('pointerup', 220, 100, 2);
  p.advance(2); p.event('pointerup', 10);
  p.event('pointerdown', 30);
  p.advance(2); p.event('pointermove', 40);
  p.event('pointerup', 40);
  p.advance(100);
  assert.deepEqual(p.messages, [touch(10),touch(10,0),touch(30),touch(40),touch(40,0)]);
});

test('captured movement outside the panel is clamped to the controller bounds', () => {
  const p = panel();
  p.event('pointerdown', -20, -10);
  p.advance(50); p.event('pointermove', 500, 600);
  p.event('pointerup', 500, 600);
  assert.deepEqual(p.messages, [touch(0,1,0),touch(367,1,447),touch(367,0,447)]);
});

test('rotated panel views map touches back to panel coordinates', () => {
  const context = { window: {}, performance: {timeOrigin: 0, now: () => 0}, setTimeout, clearTimeout };
  vm.runInNewContext(source, context);
  const map = context.window.panelPoint;
  // A 368x448 portrait panel. Quarter turns show it as a 448x368 canvas.
  const p = (x, y) => ({x, y});
  assert.deepEqual({...map(10, 20, 368, 448, 0)}, p(10, 20));
  assert.deepEqual({...map(0, 0, 448, 368, 90)}, p(0, 447), '90: the view\'s top-left is the panel\'s bottom-left');
  assert.deepEqual({...map(447, 0, 448, 368, 90)}, p(0, 0));
  assert.deepEqual({...map(447, 367, 448, 368, 90)}, p(367, 0));
  assert.deepEqual({...map(0, 0, 368, 448, 180)}, p(367, 447));
  assert.deepEqual({...map(0, 0, 448, 368, 270)}, p(367, 0), '270: the view\'s top-left is the panel\'s top-right');
  assert.deepEqual({...map(447, 367, 448, 368, 270)}, p(0, 447));
  for (const rotate of [0, 90, 180, 270]) {
    const [w, h] = rotate % 180 ? [448, 368] : [368, 448];
    const seen = new Set();
    for (let y = 0; y < h; y += 7) for (let x = 0; x < w; x += 7) {
      const q = map(x, y, w, h, rotate);
      assert.ok(q.x >= 0 && q.x < 368 && q.y >= 0 && q.y < 448, `${rotate}: ${x},${y} -> ${q.x},${q.y} out of the panel`);
      seen.add(q.x * 448 + q.y);
    }
    assert.equal(seen.size, Math.ceil(w / 7) * Math.ceil(h / 7), `${rotate}: distinct view points stay distinct`);
  }
});

test('the touch handler sends panel coordinates for a rotated view', () => {
  let clock = 0;
  const messages = [];
  const canvas = Object.assign(new EventTarget(), {
    width: 448, height: 368,
    getBoundingClientRect: () => ({left: 0, top: 0, width: 448, height: 368}),
    setPointerCapture() {},
  });
  const context = { window: {}, performance: {timeOrigin: 0, now: () => clock}, getComputedStyle: () => ({}), setTimeout, clearTimeout };
  vm.runInNewContext(source, context);
  context.window.installTouchInput(canvas, data => messages.push(JSON.parse(data)), { rotation: () => 90 });
  canvas.dispatchEvent(Object.assign(new Event('pointerdown'), {clientX: 0, clientY: 0, pointerId: 1}));
  canvas.dispatchEvent(Object.assign(new Event('pointerup'), {clientX: 447, clientY: 367, pointerId: 1}));
  assert.deepEqual(messages, [{t: 'touch', x: 0, y: 447, down: 1}, {t: 'touch', x: 367, y: 0, down: 0}]);
});

test('scaled workbench pixels exclude the bezel and padding before rotation', () => {
  const messages = [];
  const canvas = Object.assign(new EventTarget(), {
    width: 480, height: 320,
    getBoundingClientRect: () => ({left: 241, top: 150, width: 456, height: 320}),
    setPointerCapture() {},
  });
  const context = {
    window: {}, performance: {timeOrigin: 0, now: () => 0}, setTimeout, clearTimeout,
    getComputedStyle: () => ({borderLeftWidth: '20px', borderRightWidth: '20px', borderTopWidth: '20px', borderBottomWidth: '20px', paddingLeft: '3px', paddingRight: '5px', paddingTop: '2px', paddingBottom: '6px'}),
  };
  vm.runInNewContext(source, context);
  context.window.installTouchInput(canvas, data => messages.push(JSON.parse(data)), { rotation: () => 90 });
  const event = (type, clientX, clientY) => canvas.dispatchEvent(Object.assign(new Event(type), {clientX, clientY, pointerId: 1}));
  // Content is 408x272 at (264,172); the displayed preset arrow is pixel (444,23).
  event('pointerdown', 264 + 444 * .85, 172 + 23 * .85);
  event('pointerup', 264 + 444 * .85, 172 + 23 * .85);
  // Captured movement through the bezel still clamps to the real panel edge.
  event('pointerdown', 241, 150);
  event('pointerup', 697, 470);
  assert.deepEqual(messages, [
    {t: 'touch', x: 23, y: 35, down: 1}, {t: 'touch', x: 23, y: 35, down: 0},
    {t: 'touch', x: 0, y: 479, down: 1}, {t: 'touch', x: 319, y: 0, down: 0},
  ]);
});
