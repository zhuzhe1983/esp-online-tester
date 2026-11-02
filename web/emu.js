// Page-side glue for the WebAssembly build. Active when the page is opened with `?wasm`; then
// window.EmuLink replaces the WebSocket transport in run.html, and a firmware panel is added.
// Firmware comes from the visitor's disk (file inputs) or from a manifest `wasm/fw/<name>.json`
// (`?wasm&fw=<name>`) listing files to fetch — for your own hosting; never publish blobs you
// may not redistribute (the Espressif mask ROM, third-party firmware).
(() => {
  const q = new URLSearchParams(location.search);
  // wasm mode: asked for explicitly, or the page is on a static host that cannot be the native emulator
  if (!q.has('wasm') && !/\.github\.io$/.test(location.hostname)) return;
  const worker = new Worker('wasm/worker.js', { type: 'module' });
  let onmessage = null, setStatus = () => {}, ready = false, started = false;
  // A manifest that cannot load keeps its message: the page may attach its status line after the
  // failure, and the module finishing its download must not replace it with "wasm loaded".
  let failure = '';
  const fail = (msg) => { failure = msg; setStatus(msg); };
  // A demo file a checkout lacks: name the script that fetches it. The Pages site should have them
  // all, so there a 404 is a deploy problem and gets no checkout advice.
  const checkout = !location.hostname.endsWith('github.io');
  const missing = (u, status) => `${u}: ${status}` + (status !== 404 ? ''
    : !checkout ? ' — missing from this site'
    : /_rom\.elf$|^linux-esp32s3-native-full\.bin$/.test(u) ? ' — not in this checkout: run tools/fetch-demo-assets.sh'
    : /^local\/model_q4\.bin$/.test(u) ? ' — not in this checkout: run tools/fetch-pocket-tank.sh'
    : /^ESP32_GENERIC-.*\.bin$/.test(u) ? ' — not in this checkout: run tools/fetch-micropython.sh'
    : '');
  const KINDS = { rom: 0, bootloader: 1, ptable: 2, app: 3, elf: 4, flash: 5, script: 6, picture: 7 };
  const pending = new Map();
  worker.onmessage = (ev) => {
    const m = ev.data;
    if (m.touchTrace) { window.recordTouchTrace?.(m.touchTrace); return; }
    if (m.text !== undefined) { onmessage && onmessage(m.text); return; }
    if (m.bin !== undefined) {
      try {
        if (m.frameTrace) window.recordTouchTrace?.(m.frameTrace);
        onmessage && onmessage(m.bin);
        if (m.frameTrace) window.recordTouchTrace?.({ stage: 'canvas-drawn', atMs: performance.timeOrigin + performance.now(), cycles: m.frameTrace.cycles });
      } finally { if (m.ack) worker.postMessage({ op: 'frame-ack' }); }
      return;
    }
    if (m.log !== undefined) { console.log(m.log); onmessage && onmessage(JSON.stringify({ t: 'emu', msg: m.log })); return; }
    if (m.ready) { ready = true; setStatus(failure || 'wasm loaded — choose firmware'); flush(); }
    if (m.created !== undefined) { const r = pending.get('created'); pending.delete('created'); r && r(m.created); }
    if (m.loaded !== undefined) { const r = pending.get('load' + m.loaded); pending.delete('load' + m.loaded); r && r(m.ok); }
    if (m.started !== undefined) { started = m.started; setStatus(started ? 'running in WebAssembly' : 'boot failed: ' + (m.error || 'see console')); }
    if (m.stopped !== undefined) { started = false; setStatus('stopped: code ' + m.stopped); }
    if (m.netText) { onmessage && onmessage(JSON.stringify({ t: 'serial', src: 'node' + m.netText.node, data: m.netText.data })); }
    if (m.netStat) { onmessage && onmessage(JSON.stringify({ t: 'net', ...m.netStat })); }
    if (m.pace) { const el = document.getElementById('pace'); if (el) el.textContent = `${(m.pace.mips || 0).toFixed(1)} Minsn/s` + (m.pace.speed == null ? '' : m.pace.speed < 0.95 ? ` · ⚠ ${Math.floor(m.pace.speed * 100)}% of real time` : ' · real time'); }
  };
  const queue = []; const flush = () => { while (ready && queue.length) worker.postMessage(...queue.shift()); };
  const post = (msg, transfer) => { queue.push([msg, transfer || []]); flush(); };
  const ask = (key, msg, transfer) => new Promise((res) => { pending.set(key, res); post(msg, transfer); });

  window.EmuLink = {
    connect(handler, status) {
      // the status line is in the header, behind the configuration dialog: mirror it into the dialog too
      onmessage = handler; setStatus = (s) => { status(s); const el = document.getElementById('fw_status'); if (el) el.textContent = s; }; if (failure) setStatus(failure);
      fetch('wasm/esp32sim.wasm').then((r) => { if (!r.ok) throw new Error('wasm/esp32sim.wasm: ' + r.status + (r.status === 404 ? ' — build it: tools/wasm-build.sh' : '')); return r.arrayBuffer(); })
        .then((buf) => worker.postMessage({ op: 'init', wasm: buf, frameAck: true, touchTrace: q.has('touchTrace') }, [buf]))
        .catch((e) => setStatus('cannot load wasm: ' + e.message));
      return { send: (d, timing) => { if (!started) return; if (typeof d === 'string') post({ op: 'text', data: d, touchTrace: timing }); else { const b = d.buffer ? d.buffer.slice(d.byteOffset, d.byteOffset + d.byteLength) : d; post({ op: 'bin', data: b }, [b]); } } };
    },
  };

  // Everything below touches the DOM; this script is loaded at the top of <body>, before the
  // header and main exist, so it waits for the document. EmuLink above is what run.html's
  // own script (at the end of <body>) needs, and that is defined synchronously.
  document.addEventListener('DOMContentLoaded', () => {
  // no firmware yet: no board to draw. The board announcement at boot switches the layout.
  document.body.classList.add('bare'); document.querySelector('h1').textContent = 'ESP32\u2011S3 emulator';
  // ---- firmware panel
  const panel = document.createElement('section');
  panel.id = 'fwpanel';
  panel.innerHTML = `<style>#fwpanel{margin:12px 18px 0;padding:12px 16px;background:#fff;border:1px solid #e5e7eb;border-radius:10px;font-size:13px}
    #fwpanel .row{display:flex;flex-wrap:wrap;gap:10px 18px;align-items:center;margin:4px 0}#fwpanel label{display:inline-flex;gap:6px;align-items:center}
    #fwpanel input[type=file]{max-width:180px}#fwpanel .go{padding:6px 14px;font-weight:600}#fwpanel .note{color:#6b7280}</style>
    <div class="row"><b>WebAssembly build</b><span class="note">everything runs in this tab — nothing is uploaded; firmware is read from your disk</span><span id="pace" class="note"></span></div>
    <div class="row" id="fw_demos" style="display:none"><span>Demos:</span></div>
    <div class="row">
      <label>board <select id="fw_board"><option>waveshare-lcd4b</option><option>waveshare-amoled18-v2</option><option>stopwatch</option><option>atech14</option><option>waveshare-cam</option><option>none</option><option>esp32</option><option>ttgo-tdisplay</option><option>ulanzi-tc001</option><option>esp32c3</option><option>esp32c6</option><option>waveshare-c6-lcd147</option><option>esp32p4</option><option>esp32p4-rev3</option><option>waveshare-p4-lcd35</option></select></label>
      <label>flash MB <input id="fw_flash" type="number" value="16" min="1" max="32" style="width:52px"></label>
      <label>PSRAM MB <input id="fw_psram" type="number" value="8" min="0" max="32" style="width:52px"></label>
      <label>WiFi <input id="fw_wifi" placeholder="ssid=…,psk=… (optional)" style="width:190px"></label>
      <label>stubs <input id="fw_stubs" placeholder="esp_wifi_start=0" style="width:150px"></label>
      <label><input id="fw_appdirect" type="checkbox"> boot app directly (no ROM)</label>
    </div>
    <div class="row">
      <label>ROM ELF <input type="file" id="fw_rom"></label><span class="note" id="fw_romnote">optional: this site's copy of the chip's mask ROM is used when empty</span>
      <label>bootloader.bin <input type="file" id="fw_bootloader"></label>
      <label>partition-table.bin <input type="file" id="fw_ptable"></label>
      <label>app.bin <input type="file" id="fw_app"></label>
      <label>app.elf (symbols) <input type="file" id="fw_elf"></label>
      <label>script <input type="file" id="fw_script"></label>
      <button class="go" id="fw_go">▶ Boot</button><span class="note" id="fw_status"></span>
    </div>
    <div class="row note">Your build's three images are in its build directory: <code>bootloader/bootloader.bin</code>, <code>partition_table/partition-table.bin</code> and the app. Booting again replaces the running machine; the files stay chosen. <a href="https://github.com/mikroverk/esp32sim/blob/main/docs/quick-start.md" target="_blank">Quick start ↗</a></div>`;
  document.body.insertBefore(panel, document.querySelector('main'));
  const $ = (id) => document.getElementById(id);
  // The memory the demos use for each board (the CLI's defaults: S3 8 / 2, P4 16 / 0, the others 4 / 0); picking
  // a board sets them, the visitor can still change them.
  const MEMORY = { 'waveshare-lcd4b': [16, 8], 'waveshare-amoled18-v2': [16, 8], 'waveshare-cam': [16, 8], atech14: [8, 2], none: [8, 2],
    esp32: [4, 0], 'ttgo-tdisplay': [4, 0], 'ulanzi-tc001': [4, 0], esp32c3: [4, 0], esp32c6: [4, 0], 'waveshare-c6-lcd147': [4, 0], esp32p4: [16, 0], 'esp32p4-rev3': [16, 0], 'waveshare-p4-lcd35': [16, 32] };
  $('fw_board').onchange = () => { const m = MEMORY[$('fw_board').value]; if (m) { $('fw_flash').value = m[0]; $('fw_psram').value = m[1]; } };
  // The mask ROM the site hosts for the chip a board name implies (the files tools/fetch-demo-assets.sh
  // puts in wasm/fw/): used when the visitor picks no ROM ELF of their own.
  const romFor = (board) => board === 'esp32c3' ? 'esp32c3_rev3_rom.elf' : board === 'esp32p4' ? 'esp32p4_rev0_rom.elf' : board === 'esp32p4-rev3' || board === 'waveshare-p4-lcd35' ? 'esp32p4_rev300_rom.elf' : /c6/.test(board) ? 'esp32c6_rev0_rom.elf'
    : /^(esp32|esp32dev.*|ttgo-tdisplay|tdisplay|ulanzi-tc001|tc001)$/.test(board) ? 'esp32_rev300_rom.elf' : 'esp32s3_rev0_rom.elf';
  const readFile = (f) => new Promise((res, rej) => { const r = new FileReader(); r.onload = () => res(r.result); r.onerror = rej; r.readAsArrayBuffer(f); });

  // Preserve the value text so the Rust parser owns all return-value rules.
  function stubSpec(spec, symbols = {}) {
    const split = spec.indexOf('='), name = split < 0 ? spec : spec.slice(0, split);
    return (symbols[name] || name) + (split < 0 ? '' : spec.slice(split));
  }
  async function boot(cfg, files) {
    // booting again replaces the running machine (the worker deletes it on `create`); the page keeps
    // its console and the dialog keeps its files
    setStatus(started ? 'restarting with the new firmware…' : 'loading firmware…');
    let experiments;
    try { const { experimentsFromParams } = await import('./wasm/experiments.mjs'); experiments = experimentsFromParams(q, cfg.board); }
    catch (err) { setStatus(err.message); return; }
    const ok = await ask('created', { op: 'create', board: cfg.board, smoothDisplay: cfg.smoothDisplay === true, flash_mb: cfg.flash_mb, psram_mb: cfg.psram_mb, jit: q.get('jit') !== '0', experiments });
    if (!ok) { setStatus('could not create emulator: check board and memory sizes (maximum 32 MiB)'); return; }
    for (const [kind, data, at] of files) {
      // `c6:KIND` is the co-processor's image (the module's COPROC_KIND bit), as is a flash offset with bit 31 set
      const k = kind.startsWith('c6:') ? KINDS[kind.slice(3)] | 0x100 : KINDS[kind];
      const key = at !== undefined ? 'loadat' + at : 'load' + k;
      const good = await ask(key, at !== undefined ? { op: 'load', at, data } : { op: 'load', kind: k, data }, [data]);
      if (!good) { setStatus('failed to load ' + (at !== undefined ? 'flash@0x' + at.toString(16) : kind) + ' (see console)'); return; }
    }
    for (const st of cfg.stubs || []) post({ op: 'stub', spec: stubSpec(st, cfg.symbols) });
    if (cfg.wifi) post({ op: 'wifi', spec: cfg.wifi });
    post({ op: 'start', appDirect: !!cfg.appDirect });
  }
  // A network manifest boots several motes on one medium (esp32sim_net_*): the same files go to
  // every node, each with its own MAC, position and power-on offset.
  async function bootNet(cfg, files, nodeFiles) {
    if (q.has('timing') || q.has('quantum')) { setStatus('timing and quantum are unsupported for network mode'); return; }
    setStatus('creating the network…');
    const r = await ask('created', { op: 'net-create', nodes: cfg.nodes, board: cfg.board, flash_mb: cfg.flash_mb, slice_ns: cfg.slice_ns || 0 });
    if (!r) { setStatus('could not create the network'); return; }
    for (let i = 0; i < cfg.nodes.length; i++) {
      // a node's own image of a kind replaces the shared one (a server next to a client)
      const own = (nodeFiles && nodeFiles[i]) || [];
      const mine = files.filter(([k]) => !own.some(([ok]) => ok === k)).concat(own);
      for (const [kind, data] of mine) {
        const copy = data.slice(0);   // each node keeps its own image: the buffer is transferred
        const good = await ask('load' + 'n' + i + ':' + KINDS[kind], { op: 'net-load', node: i, kind: KINDS[kind], data: copy }, [copy]);
        if (!good) { setStatus(`node ${i}: failed to load ${kind}`); return; }
      }
      for (const st of [].concat(cfg.nodes[i].stubs || cfg.stubs || [])) post({ op: 'net-stub', node: i, spec: stubSpec(st, cfg.nodes[i].symbols || cfg.symbols) });
    }
    post({ op: 'net-start' });
  }

  $('fw_go').onclick = async () => {
    const files = [];
    for (const k of ['rom', 'bootloader', 'ptable', 'app', 'elf', 'script']) { const f = $('fw_' + k).files[0]; if (f) files.push([k, await readFile(f)]); }
    if (!files.some((x) => x[0] === 'app')) { setStatus('an app.bin is required'); return; }
    const board = $('fw_board').value;
    if (!$('fw_appdirect').checked && !files.some((x) => x[0] === 'rom')) {
      const u = romFor(board);
      setStatus(`fetching the mask ROM (${u})…`);
      const r = await fetch(`wasm/fw/${u}`).catch(() => null);
      if (!r || !r.ok) { setStatus(missing(u, r ? r.status : 'unreachable') + ' — pick the ROM ELF file instead, or tick "boot app directly"'); return; }
      files.unshift(['rom', await r.arrayBuffer()]);
    }
    boot({ board, flash_mb: +$('fw_flash').value, psram_mb: +$('fw_psram').value, wifi: $('fw_wifi').value.trim(), stubs: $('fw_stubs').value.split(/[ ,]+/).filter(Boolean), appDirect: $('fw_appdirect').checked }, files);
  };
  fetch('wasm/fw/demos.json', { cache: 'no-cache' }).then((r) => r.ok ? r.json() : []).then((demos) => {
    const row = $('fw_demos'); if (!demos.length) return; row.style.display = '';
    let parent = '';
    for (const d of demos) {
      const a = document.createElement('a'); const u = new URL(location.href); u.searchParams.set('wasm', ''); u.searchParams.set('fw', d.fw); a.href = u.toString(); a.textContent = d.title; a.title = d.note || ''; row.appendChild(a);
      // "…logged in for you" continues the entry above it: name the tab "Linux 6.11 on the ESP32-S3 — logged in for you"
      const name = d.title.startsWith('…') ? `${parent} — ${d.title.slice(1)}` : d.title;
      if (!d.title.startsWith('…')) parent = d.title;
      if (d.fw === q.get('fw')) { window.EmuLink.demoTitle = name; document.title = name + ' · esp32sim'; }
    }
  }).catch(() => {});
  // ---- manifest: ?wasm&fw=name → wasm/fw/name.json
  const fw = q.get('fw');
  if (fw) {
    (async () => {
      const man = await (await fetch(`wasm/fw/${fw}.json`, { cache: 'no-cache' })).json();   // manifests are tiny: always revalidate, so a removed demo disappears at once
      $('fw_board').value = man.board || 'none'; $('fw_flash').value = man.flash_mb || 8; $('fw_psram').value = man.psram_mb || 2; $('fw_wifi').value = man.wifi || ''; $('fw_stubs').value = (man.stubs || []).join(' '); window.EmuLink.terminal = !!man.terminal; window.EmuLink.lineHint = man.line_hint || ''; window.EmuLink.displayRotate = man.display_rotate; if (man.line_hint) $('line').placeholder = man.line_hint;   // the page opens on the Terminal tab for a manifest that says so
      const files = [];
      for (const [kind, url] of Object.entries(man.files || {})) for (const u of [].concat(url)) { const r = await fetch(`wasm/fw/${u}`, { cache: 'no-cache' }); if (!r.ok) { fail(missing(u, r.status)); return; } files.push([kind, await r.arrayBuffer()]); }
      // flash_at: { "0x610000": "public/energydata.json" } — a data partition's contents
      for (const [off, u] of Object.entries(man.flash_at || {})) { const r = await fetch(`wasm/fw/${u}`, { cache: 'no-cache' }); if (!r.ok) { fail(missing(u, r.status)); return; } files.push(['flash', await r.arrayBuffer(), parseInt(off, 16)]); }
      // coproc: { files: {...}, flash_at: {...} } — the ESP32-C6 next to a P4 (board `esp32p4…+c6`)
      for (const [kind, u] of Object.entries((man.coproc || {}).files || {})) { const r = await fetch(`wasm/fw/${u}`, { cache: 'no-cache' }); if (!r.ok) { fail(missing(u, r.status)); return; } files.push(['c6:' + kind, await r.arrayBuffer()]); }
      for (const [off, u] of Object.entries((man.coproc || {}).flash_at || {})) { const r = await fetch(`wasm/fw/${u}`, { cache: 'no-cache' }); if (!r.ok) { fail(missing(u, r.status)); return; } files.push(['flash', await r.arrayBuffer(), (parseInt(off, 16) | 0x80000000) >>> 0]); }
      const cfg = { board: man.board, flash_mb: man.flash_mb || 8, psram_mb: man.psram_mb || 2, wifi: man.wifi || '', stubs: man.stubs || [], symbols: man.symbols || {}, appDirect: !!man.app_direct, nodes: man.nodes, slice_ns: man.slice_ns };
      const nodeFiles = [];
      for (const node of man.nodes || []) { const own = []; for (const [kind, url] of Object.entries(node.files || {})) for (const u of [].concat(url)) { const r = await fetch(`wasm/fw/${u}`, { cache: 'no-cache' }); if (!r.ok) { fail(missing(u, r.status)); return; } own.push([kind, await r.arrayBuffer()]); } nodeFiles.push(own); }
      const wait = () => ready ? (man.nodes ? bootNet(cfg, files, nodeFiles) : boot(cfg, files)) : setTimeout(wait, 50);
      wait();
    })().catch((e) => fail('manifest: ' + e.message));
  }
  });
})();
