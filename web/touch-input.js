// A point on a panel view rotated clockwise by `rotate` degrees (0, 90, 180, 270), whose canvas
// is width x height, back in the panel's own coordinates: the controller never sees the rotation.
window.panelPoint = function (x, y, width, height, rotate) {
  switch (rotate) {
    case 90: return { x: y, y: width - 1 - x };
    case 180: return { x: width - 1 - x, y: height - 1 - y };
    case 270: return { x: height - 1 - y, y: x };
    default: return { x, y };
  }
};

// One active contact, sampled at most once per interval. Always deliver the
// newest pending move, including before a release; never assign guest times.
window.installTouchInput = function (canvas, send, { intervalMs = 40, trace = null, rotation = () => 0 } = {}) {
  let pointer = null, last = 0, pending = null, timer = null, sequence = 0;
  const now = () => performance.timeOrigin + performance.now();
  const sample = (e) => {
    const r = canvas.getBoundingClientRect();
    // The workbench bezel belongs to the CSS box, not to the panel pixels.
    const style = getComputedStyle(canvas), px = key => parseFloat(style[key]) || 0;
    const left = px('borderLeftWidth') + px('paddingLeft'), right = px('borderRightWidth') + px('paddingRight');
    const top = px('borderTopWidth') + px('paddingTop'), bottom = px('borderBottomWidth') + px('paddingBottom');
    const x = Math.max(0, Math.min(canvas.width - 1, Math.round((e.clientX - r.left - left) * canvas.width / Math.max(1, r.width - left - right))));
    const y = Math.max(0, Math.min(canvas.height - 1, Math.round((e.clientY - r.top - top) * canvas.height / Math.max(1, r.height - top - bottom))));
    return { ...window.panelPoint(x, y, canvas.width, canvas.height, rotation()), id: ++sequence, event: e.type, arrivalMs: now() };
  };
  const publish = (point, down) => {
    last = performance.now();
    const timing = trace ? { ...point, down: down ? 1 : 0, sentMs: now() } : undefined;
    if (timing) trace({ stage: 'page-send', ...timing });
    send(JSON.stringify({ t: 'touch', x: point.x, y: point.y, down: down ? 1 : 0 }), timing);
  };
  const flush = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    if (pending) { const point = pending; pending = null; publish(point, true); }
  };
  canvas.addEventListener('pointerdown', (e) => {
    if (pointer !== null) return;
    pointer = e.pointerId;
    canvas.setPointerCapture(pointer);
    publish(sample(e), true);
  });
  canvas.addEventListener('pointermove', (e) => {
    if (e.pointerId !== pointer) return;
    pending = sample(e);
    if (trace) trace({ stage: 'page-arrival', ...pending });
    const remaining = intervalMs - (performance.now() - last);
    if (remaining <= 0) flush();
    else if (timer === null) timer = setTimeout(flush, remaining);
  });
  const release = (e) => {
    if (e.pointerId !== pointer) return;
    flush();
    publish(sample(e), false);
    pointer = null;
  };
  canvas.addEventListener('pointerup', release);
  canvas.addEventListener('pointercancel', release);
  canvas.addEventListener('lostpointercapture', release);
};
