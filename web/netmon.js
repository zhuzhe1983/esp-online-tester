// The board page's Network panel: the Ethernet frames the emulated WiFi station sends and
// receives, streamed by `--web` runs with `--wifi` as binary messages
//   [5][dir u8: 0 = from the station, 1 = to it][emulated time, µs, u64 le][Ethernet frame…]
// after a {"t":"netmon","linktype":1} hello. Frames are dissected with dpkt-js 0.1.0 (BSD-3-Clause),
// vendored under vendor/dpkt-js and loaded the first time the panel is opened or a row clicked;
// until then, or if it fails to load, rows show length and hex.
(function () {
  'use strict';
  const DPKT_URL = new URL('vendor/dpkt-js/index.js', (document.currentScript && document.currentScript.src) || document.baseURI).href;
  const KEEP = 5000;                 // frames kept in the page; the oldest go first
  const ETHERTYPES = { 0x0800: 'IPv4', 0x0806: 'ARP', 0x86dd: 'IPv6', 0x888e: 'EAPOL', 0x8100: '802.1Q' };
  let frames = [], seq = 0, dissector = null, dpkt = null, dpktState = 'idle', panel = null;
  let paused = false, filter = '', selected = null;
  const $ = id => document.getElementById(id);
  const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const hex = (b, n) => Array.from(b.subarray(0, n), x => x.toString(16).padStart(2, '0')).join(' ');

  function build() {
    const card = document.createElement('div');
    card.className = 'card'; card.id = 'netmonpanel';
    card.innerHTML = `<details id="netmondetails"><summary class="row" style="cursor:pointer;list-style:revert"><strong>Network</strong><span class="st" id="netmonstat">the station's Ethernet frames, both ways</span></summary>
      <div class="row" style="margin-top:8px"><input type="text" id="netmonfilter" placeholder="filter: dhcp, icmp, 10.0.2.2, ↑…" style="flex:1;min-width:120px">
        <label class="st"><input type="checkbox" id="netmonscroll" checked> auto-scroll</label>
        <button class="act" id="netmonpause">pause</button><button class="act" id="netmonclear">clear</button><button class="act" id="netmonpcap">download pcap</button></div>
      <div id="netmonlist" style="margin-top:8px;background:#0b0d10;color:#e6e6e6;border-radius:8px;height:220px;overflow:auto;font:12px/1.5 ui-monospace,Menlo,monospace"></div>
      <div id="netmontree" style="margin-top:8px;background:#0b0d10;color:#e6e6e6;border-radius:8px;max-height:300px;overflow:auto;padding:8px 10px;font:12px/1.45 ui-monospace,Menlo,monospace;display:none"></div>
    </details>`;
    const right = document.querySelector('section.right');
    const anchor = $('campanel');
    if (anchor && anchor.parentNode === right) right.insertBefore(card, anchor); else (right || document.body).appendChild(card);
    $('netmonfilter').oninput = e => { filter = e.target.value.trim().toLowerCase(); render(); };
    $('netmonpause').onclick = () => { paused = !paused; $('netmonpause').textContent = paused ? 'resume' : 'pause'; if (!paused) render(); };
    $('netmonclear').onclick = () => { frames = []; selected = null; $('netmontree').style.display = 'none'; render(); };
    $('netmonpcap').onclick = download;
    $('netmondetails').ontoggle = () => { if ($('netmondetails').open) { loadDpkt(); render(); } };
    $('netmonlist').onclick = e => { loadDpkt(); const r = e.target.closest('[data-seq]'); if (r) select(+r.dataset.seq); };
    return card;
  }

  function loadDpkt() {
    if (dpktState !== 'idle') return;
    dpktState = 'loading';
    import(DPKT_URL).then(m => {
      dpkt = m; dpktState = 'ready';
      try { dissector = new m.Dissector(1); } catch (e) { dissector = null; }
      frames.forEach(f => { f.summary = summarize(f); });
      render();
      if (selected !== null) select(selected);
    }).catch(() => {
      dpktState = 'failed';
      $('netmonstat').textContent = 'dpkt-js could not load: frames are shown as length and hex';
    });
  }

  // the deepest layer's summary along the first-child chain, with the layers' names
  function summarize(f) {
    if (!dpkt) return `${f.data.length} B  ${hex(f.data, 24)}${f.data.length > 24 ? ' …' : ''}`;
    let tree;
    try { tree = dissector ? dissector.dissect(f.data, f.ts / 1e6) : dpkt.dissectFrame(1, f.data); } catch (e) { return `${f.data.length} B  (dissection failed: ${e.message})`; }
    const parts = [];
    for (let n = tree, depth = 0; n && depth < 12; n = (n.children || [])[0], depth++) {
      if (n.label === 'Ethernet' && (n.children || []).length) continue;
      if (n.label === 'Data' && parts.length) break;
      parts.push(n.summary ? `${n.label} ${n.summary}` : n.label);
    }
    if (tree && !(tree.children || []).length && f.data.length >= 14) {
      const et = (f.data[12] << 8) | f.data[13];
      parts.push(`${ETHERTYPES[et] || 'type 0x' + et.toString(16).padStart(4, '0')}`);
    }
    return parts.join(' · ') + (tree && tree.error ? ` ⚠ ${tree.error}` : '');
  }

  function rowHtml(f) {
    const t = (f.ts / 1e6).toFixed(6), arrow = f.dir === 0 ? '↑' : '↓';
    const style = f.seq === selected ? 'background:#1f2937;' : '';
    return `<div data-seq="${f.seq}" style="${style}padding:1px 10px;cursor:pointer;white-space:nowrap;overflow:hidden;text-overflow:ellipsis" title="${arrow === '↑' ? 'sent by the station' : 'received by the station'}"><span style="color:#9ca3af">${t}</span> <span style="color:${f.dir === 0 ? '#7dd3fc' : '#86efac'}">${arrow}</span> ${esc(f.summary)}</div>`;
  }
  const matches = f => !filter || (f.dir === 0 ? '↑ ' : '↓ ').concat(f.summary).toLowerCase().includes(filter);

  function render() {
    if (!panel || paused) return;
    const list = $('netmonlist');
    list.innerHTML = frames.filter(matches).map(rowHtml).join('');
    if ($('netmonscroll').checked) list.scrollTop = list.scrollHeight;
    status();
  }
  function status() {
    if (dpktState === 'failed') return;
    const up = frames.filter(f => f.dir === 0).length;
    $('netmonstat').textContent = `${frames.length} frames · ${up} ↑ sent by the station · ${frames.length - up} ↓ received` + (dpktState === 'loading' ? ' · loading dpkt-js…' : '');
  }

  function treeHtml(n) {
    if (!n) return '';
    const fields = (n.fields || []).map(([k, v]) => `<div style="padding-left:14px"><span style="color:#9ca3af">${esc(k)}:</span> ${esc(v)}</div>`).join('');
    const kids = (n.children || []).map(treeHtml).join('');
    const err = n.error ? `<div style="padding-left:14px;color:#fca5a5">⚠ ${esc(n.error)}</div>` : '';
    return `<details open style="padding-left:6px"><summary style="cursor:pointer"><b>${esc(n.label)}</b>${n.summary ? ' ' + esc(n.summary) : ''}</summary>${fields}${err}<div style="padding-left:8px">${kids}</div></details>`;
  }
  function select(s) {
    const f = frames.find(x => x.seq === s); if (!f) return;
    selected = s;
    const box = $('netmontree'); box.style.display = '';
    let body;
    if (dpkt) { try { body = treeHtml(dpkt.dissectFrame(1, f.data)); } catch (e) { body = `<div style="color:#fca5a5">dissection failed: ${esc(e.message)}</div>`; } }
    else body = `<div style="color:#9ca3af">dpkt-js is not loaded: raw frame</div>`;
    box.innerHTML = `<div style="color:#9ca3af">#${f.seq} · ${(f.ts / 1e6).toFixed(6)} s · ${f.dir === 0 ? '↑ sent by the station' : '↓ received by the station'} · ${f.data.length} bytes</div>${body}<pre style="margin:6px 0 0;white-space:pre-wrap;color:#9ca3af">${hex(f.data, f.data.length)}</pre>`;
    if (!paused) { const prev = $('netmonscroll').checked; $('netmonscroll').checked = false; render(); $('netmonscroll').checked = prev; }
  }

  // a classic pcap (microsecond timestamps, linktype 1) of the frames the page holds
  function download() {
    const size = 24 + frames.reduce((a, f) => a + 16 + f.data.length, 0);
    const out = new Uint8Array(size), dv = new DataView(out.buffer);
    dv.setUint32(0, 0xa1b2c3d4, true); dv.setUint16(4, 2, true); dv.setUint16(6, 4, true);
    dv.setUint32(16, 65535, true); dv.setUint32(20, 1, true);
    let o = 24;
    for (const f of frames) {
      dv.setUint32(o, Math.floor(f.ts / 1e6), true); dv.setUint32(o + 4, f.ts % 1e6, true);
      dv.setUint32(o + 8, f.data.length, true); dv.setUint32(o + 12, f.data.length, true);
      out.set(f.data, o + 16); o += 16 + f.data.length;
    }
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([out], { type: 'application/vnd.tcpdump.pcap' }));
    a.download = 'esp32sim-station.pcap';
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  }

  window.NetMon = {
    // the server's {"t":"netmon"} hello: this run has a WiFi station
    show() { if (!panel) { panel = build(); status(); } },
    // one binary message of type 5
    frame(buf) {
      if (!panel) this.show();
      const b = new Uint8Array(buf);
      if (b.length < 10) return;
      const ts = Number(new DataView(buf).getBigUint64(2, true));
      const f = { seq: ++seq, dir: b[1], ts, data: b.slice(10) };
      f.summary = summarize(f);
      frames.push(f);
      if (frames.length > KEEP) frames.splice(0, frames.length - KEEP);
      if (paused) return;
      const list = $('netmonlist');
      while (list.firstElementChild && +list.firstElementChild.dataset.seq < frames[0].seq) list.firstElementChild.remove();
      if (!matches(f)) { status(); return; }
      list.insertAdjacentHTML('beforeend', rowHtml(f));
      if ($('netmonscroll').checked) list.scrollTop = list.scrollHeight;
      status();
    },
  };
})();
