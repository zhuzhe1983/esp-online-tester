// The workbench: the presentation layer over run.html. It preserves the existing elements, IDs
// and handlers and only moves them into a device stage, a console and a side rail; ?plain
// leaves the page exactly as it was.
(() => {
  const query = new URLSearchParams(location.search);
  if (query.has('plain')) return;
  // wasm mode, as emu.js decides it: asked for, or a static host that cannot be the native emulator
  const wasmMode = query.has('wasm') || /\.github\.io$/.test(location.hostname);

  const make = (tag, className, text) => {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };
  const link = (label, href) => {
    const node = make('a', '', label);
    node.href = href;
    if (/^https:/.test(href)) { node.target = '_blank'; node.rel = 'noreferrer'; }
    return node;
  };

  const BOARD = {
    'waveshare-lcd4b': { soc: 'ESP32-S3 rev 0', cpu: '2× Xtensa LX7 @ 240 MHz', board: 'Waveshare Touch-LCD-4B', peripherals: 'ST7701S 480×480 RGB · GT911 touch · I²S audio' },
    'waveshare-amoled18-v2': { soc: 'ESP32-S3 rev 0', cpu: '2× Xtensa LX7 @ 240 MHz', board: 'Waveshare AMOLED-1.8 V2', peripherals: 'CO5300 368×448 QSPI · CST820 touch' },
    stopwatch: { soc: 'ESP32-S3 rev 0', cpu: '2× Xtensa LX7 @ 240 MHz', board: 'M5Stack StopWatch', peripherals: 'CO5300 468×466 QSPI · CST820 touch · M5PM1 · M5IOE1' },
    atech14: { soc: 'ESP32-S3 rev 0', cpu: '2× Xtensa LX7 @ 240 MHz', board: 'Atech 14-port', peripherals: 'ST7735 · WS2812 · encoder · buttons · I²S audio' },
    'waveshare-cam': { soc: 'ESP32-S3 rev 0', cpu: '2× Xtensa LX7 @ 240 MHz', board: 'Waveshare S3-CAM-OV5640', peripherals: 'OV5640 DVP camera · I²S audio · CH32V003' },
    esp32c3: { soc: 'ESP32-C3 rev 3', cpu: '1× RISC-V RV32IMC @ 160 MHz', board: 'Bare ESP32-C3', peripherals: 'UART · USB-Serial/JTAG · timers · flash' },
    esp32c6: { soc: 'ESP32-C6 rev 0', cpu: '1× RISC-V RV32IMAC @ 160 MHz', board: 'Bare ESP32-C6', peripherals: 'UART · USB-Serial/JTAG · timers · flash' },
    'waveshare-c6-lcd147': { soc: 'ESP32-C6 rev 0', cpu: '1× RISC-V RV32IMAC @ 160 MHz', board: 'Waveshare C6-LCD-1.47', peripherals: 'ST7789 172×320 · SPI2/GDMA · WS2812 · 802.15.4' },
    'ttgo-tdisplay': { soc: 'ESP32-D0WD rev 3', cpu: '2× Xtensa LX6 @ 240 MHz', board: 'LilyGO TTGO T-Display', peripherals: 'ST7789 135×240 · VSPI · two buttons' },
    esp32p4: { soc: 'ESP32-P4 rev < 3 (v1.0)', cpu: '2× RISC-V RV32IMAFC @ 360 MHz', board: 'Bare ESP32-P4', peripherals: 'UART · USB-Serial/JTAG · timers · SDMMC · flash' },
    'esp32p4-rev3': { soc: 'ESP32-P4 rev ≥ 3 (v3.1)', cpu: '2× RISC-V RV32IMAFC @ 360 MHz', board: 'Bare ESP32-P4', peripherals: 'UART · USB-Serial/JTAG · timers · flash' },
    'esp32p4-c6': { soc: 'ESP32-P4 rev < 3 + ESP32-C6', cpu: '2× RV32IMAFC @ 360 MHz + 1× RV32IMAC @ 160 MHz', board: 'P4 with a C6 on SDIO, as the Waveshare ESP32-P4-WIFI6', peripherals: 'SDIO 4-bit · ESP-Hosted 2.12.13 · the C6\'s WiFi on a virtual AP' },
    'waveshare-p4-lcd35': { soc: 'ESP32-P4 rev ≥ 3 (v3.1)', cpu: '2× RISC-V RV32IMAFC @ 360 MHz', board: 'Waveshare ESP32-P4-WIFI6-Touch-LCD-3.5', peripherals: 'ST7796 320×480 SPI · FT5x06 touch · ES8311 I²S audio' },
    'esp32p4-rev3-c6': { soc: 'ESP32-P4 rev ≥ 3 (v3.1) + ESP32-C6', cpu: '2× RV32IMAFC @ 360 MHz + 1× RV32IMAC @ 160 MHz', board: 'P4 with a C6 on SDIO, as the Waveshare ESP32-P4-WIFI6', peripherals: 'SDIO 4-bit · ESP-Hosted 2.12.13 · the C6\'s WiFi on a virtual AP' },
    none: { soc: 'ESP32-S3 rev 0', cpu: '2× Xtensa LX7 @ 240 MHz', board: 'Bare ESP32-S3', peripherals: 'UART · USB-Serial/JTAG · timers · flash' },
  };
  const HINTS = {
    panel: ['Swipe between pages on the real LVGL UI', 'Tap a tile to toggle it', 'Enable audio, then visit the SID player'],
    'panel-sid': ['The script opens the SID player', 'Enable audio to hear the guest', 'Swipe back to inspect the energy pages'],
    atech: ['Wheel or drag the encoder', 'Click the cap or press Space', 'Press the two board buttons'],
    'atech-sid': ['Enable audio for the SID jukebox', 'Watch the firmware-driven LED ring', 'Use the console while the tune plays'],
    linux: ['Open the UART0 Terminal tab', 'Log in as root', 'Try top, vi, or MicroPython'],
    'linux-login': ['Watch the scripted login', 'Open the Terminal tab', 'Type a command into UART0'],
    'linux-term': ['Click the terminal and type', 'Try top, vi, or ls --color', 'Paste and Ctrl-C work'],
    'c6-rpl-net': ['Watch both node consoles', 'Observe sent and received counters', 'Wait for the UDP request and response'],
    'c6-contiki-net': ['Watch both motes boot', 'Observe radio counters', 'See deterministic collision behavior'],
    'c6-energy-scan': ['Watch channels 11–26', 'Press BOOT for single-channel mode', 'Observe the WS2812 heartbeat'],
    'c6-contiki': ['Watch Contiki-NG boot on the console', 'A broadcast every 5 s', 'The WS2812 blinks through the RMT'],
    'pocket-tank': ['Tap or drag to touch the tank', 'Type feed 6 or shadow in the line box', 'help lists the director commands'],
    hello: ['Watch the ROM, the bootloader, then the app', 'The watchdog reboots it: same output again', 'Compare UART0 and USB-CDC'],
    'c3-hello': ['The RISC-V mask ROM boots the app', 'Open UART0 for the console', 'Verified line for line against a real C3'],
    'p4-lcd35-synth': ['Enable audio, then play the keys', 'Tap the red display to pick a preset', 'INSTRUMENT edits the sound while it plays'],
    'p4-c6-hosted': ['Both chips boot from their ROMs: [p4] and [c6]', 'The C6 joins the virtual AP for the P4', 'The P4 gets its address over SDIO'],
    'c6-hello': ['The C6 mask ROM boots the app', 'Open UART0 for the console', 'Verified against a Waveshare C6-LCD-1.47'],
  };
  const CHIP = { esp32c3: 'c3', esp32c6: 'c6', 'waveshare-c6-lcd147': 'c6', 'ttgo-tdisplay': 'esp32', esp32p4: 'p4', 'esp32p4-rev3': 'p4', 'esp32p4-c6': 'p4', 'esp32p4-rev3-c6': 'p4', 'esp32p4+c6': 'p4', 'esp32p4-rev3+c6': 'p4', 'waveshare-p4-lcd35': 'p4' };
  // The board the running machine announced (run.html's `board` message): a native --web run has no
  // manifest or Configure form, and a board picked in Configure can change after the rail is built.
  let liveBoard = null, renderMachine = null;
  // The same run on the command line: the manifest's files, so the demo reproduces from a checkout.
  function cliCommand(fw, manifest, boardName) {
    if (!manifest) return 'esp32sim --board ' + boardName + ' --boot rom --bootloader build/bootloader/bootloader.bin \\\n  --ptable build/partition_table/partition-table.bin --app build/app.bin --max-seconds 5';
    const chip = CHIP[boardName]; const f = manifest.files || {};
    const parts = ['esp32sim' + (chip ? ' --chip ' + chip : ''), '--boot rom'];
    if (!chip || boardName === 'waveshare-c6-lcd147' || boardName === 'ttgo-tdisplay') parts.splice(1, 0, '--board ' + boardName);   // the RISC-V chips refuse --board except for their own board
    if (f.rom) parts.push('--rom web/wasm/fw/' + f.rom);
    if (f.flash) parts.push('--flash-image web/wasm/fw/' + f.flash);
    for (const k of ['bootloader', 'ptable', 'app']) if (f[k]) parts.push('--' + k + ' web/wasm/fw/' + f[k]);
    for (const [off, u] of Object.entries(manifest.flash_at || {})) parts.push('--flash-at ' + off + '=web/wasm/fw/' + u);
    if (manifest.flash_mb) parts.push('--flash-mb ' + manifest.flash_mb);
    if (manifest.psram_mb) parts.push('--psram-mb ' + manifest.psram_mb);
    if (manifest.wifi) parts.push('--wifi ' + manifest.wifi);
    for (const s of (manifest.stubs || []).filter((s) => !s.startsWith('c6:'))) { const [name, value] = s.split('='); const addr = (manifest.symbols || {})[name]; parts.push('--stub ' + (addr || name) + (value !== undefined ? '=' + value : '')); }
    // the co-processor: its images and its stubs (`c6:NAME`) in one --coproc
    if (manifest.coproc) {
      const c = manifest.coproc.files || {}, kv = [];
      for (const k of ['bootloader', 'ptable', 'app']) if (c[k]) kv.push(k + '=web/wasm/fw/' + c[k]);
      for (const [off, u] of Object.entries(manifest.coproc.flash_at || {})) kv.push('otadata=web/wasm/fw/' + u + '@' + off);
      if (c.rom) kv.push('rom=web/wasm/fw/' + c.rom);
      for (const s of (manifest.stubs || []).filter((s) => s.startsWith('c6:'))) kv.push('stub=' + s.slice(3));
      parts.push('--coproc c6:' + kv.join(','), '--net none');
    }
    if (f.script) parts.push('--script web/wasm/fw/' + f.script);
    parts.push('--max-seconds ' + (manifest.seconds || 8));
    if (manifest.nodes) return '# two nodes: ' + fw + '.json lists each mote; the CLI runs one at a time\n' + parts.join(' \\\n  ');
    if (boardName !== 'none' && !chip || boardName === 'waveshare-c6-lcd147' || boardName === 'ttgo-tdisplay') parts.push('--tft-png ' + fw + '.png');
    return parts.join(' \\\n  ');
  }

  function addRailSection(rail, kicker) {
    const section = make('section', 'wb-rail-section');
    section.append(make('span', 'wb-kicker', kicker));
    rail.append(section);
    return section;
  }

  function configRow(list, name, value) {
    const row = make('div', 'wb-config-row');
    row.append(make('dt', '', name), make('dd', '', value));
    list.append(row);
  }

  async function populateRail(rail) {
    const fw = query.get('fw');
    let manifest = null, demo = null;
    if (fw) {
      try {
        const [manifestResponse, demosResponse] = await Promise.all([
          fetch(`wasm/fw/${fw}.json`, { cache: 'no-cache' }),
          fetch('wasm/fw/demos.json', { cache: 'no-cache' }),
        ]);
        if (manifestResponse.ok) manifest = await manifestResponse.json();
        if (demosResponse.ok) {
          // a title starting with … continues the entry above it, as in emu.js's list: name it in full
          let parent = '';
          for (const item of await demosResponse.json()) {
            const cont = item.title.startsWith('…');
            const name = cont ? `${parent} — ${item.title.slice(1)}` : item.title;
            if (!cont) parent = item.title;
            if (item.fw === fw) { demo = { ...item, title: name }; break; }
          }
        }
      } catch (_) {}
    }
    window.Workbench.manifest = manifest;

    if (demo) document.title = demo.title + ' · esp32sim';
    const intro = addRailSection(rail, fw ? 'Now running' : 'Local machine');
    intro.append(make('h2', 'wb-rail-title', demo?.title || fw || (wasmMode ? 'Custom firmware' : 'Native run')));
    intro.append(make('p', 'wb-copy', demo?.note || (wasmMode ? 'The same emulator engine used by the local CLI, running in this browser tab.' : 'The esp32sim process on this computer, streaming its board and consoles to this page.')));
    const hints = HINTS[fw] || (fw?.startsWith('c6-') ? ['Watch the real firmware console', 'Inspect the display and board state', 'Open the manifest and source links'] : ['Select a console and inspect the boot', 'Interact with the modeled board', wasmMode ? 'Open Configure to change the machine' : 'Change the machine with esp32sim\'s flags']);
    const hintList = make('div', 'wb-hints');
    hints.forEach(text => hintList.append(make('div', 'wb-hint', text)));
    intro.append(hintList);

    const telemetry = addRailSection(rail, 'Live telemetry');
    const metrics = make('div', 'wb-metrics');
    const metric = (label, id, initial) => {
      const box = make('div', 'wb-metric');
      box.append(make('small', '', label), make('b', '', initial));
      box.lastChild.id = id;
      metrics.append(box);
    };
    metric('Emulated time', 'wbRuntime', 'starting…');
    metric('Speed', 'wbPace', 'loading…');
    telemetry.append(metrics);

    const machine = addRailSection(rail, 'Machine configuration');
    const rows = make('dl', 'wb-config');
    machine.append(rows);
    const machineLinks = make('div', 'wb-links');
    if (fw) machineLinks.append(link('manifest ↗', `wasm/fw/${fw}.json`));
    machineLinks.append(
      link('architecture ↗', 'https://github.com/mikroverk/esp32sim/blob/main/docs/architecture.md'),
      link('board ↗', 'https://github.com/mikroverk/esp32sim/blob/main/docs/boards.md'),
      link('peripherals ↗', 'https://github.com/mikroverk/esp32sim/blob/main/docs/peripherals.md'),
    );
    machine.append(machineLinks);

    const about = addRailSection(rail, 'About esp32sim');
    about.append(make('p', 'wb-copy', 'An instruction-level Rust emulator for the ESP32, S3, C3 and C6 across Xtensa and RISC-V. It boots unmodified binaries in the browser or as a deterministic local CLI tool.'));
    const aboutLinks = make('div', 'wb-links');
    aboutLinks.append(
      link('source ↗', 'https://github.com/mikroverk/esp32sim'),
      link('README ↗', 'https://github.com/mikroverk/esp32sim#readme'),
      link('CLI ↗', 'https://github.com/mikroverk/esp32sim/blob/main/docs/cli.md'),
      link('WASM ↗', 'https://github.com/mikroverk/esp32sim/blob/main/docs/wasm.md'),
    );
    about.append(aboutLinks);
    if (manifest?._source) {
      const source = make('p', 'wb-copy', `Binary source: ${manifest._source}`);
      source.style.marginTop = '11px';
      about.append(source);
    }

    const local = addRailSection(rail, 'Use it locally');
    local.append(make('p', 'wb-copy', fw ? 'The same run on the command line, from a checkout: deterministic, scriptable, with the display as a PNG. This is what a coding agent gets.' : 'The same emulator on the command line: deterministic, scriptable, with console text, PNG, WAV and VCD outputs a coding agent can read.'));
    const command = make('pre', 'wb-command');
    local.append(command);

    renderMachine = () => {
      // `none` names no chip (the C3 and a bare C6 announce it too), so it only fills in when nothing
      // more specific is known
      const announced = liveBoard && liveBoard !== 'none' ? liveBoard : null;
      const boardName = announced || manifest?.board || document.querySelector('#fw_board')?.value || liveBoard || 'none';
      const spec = BOARD[boardName] || BOARD.none;
      rows.replaceChildren();
      configRow(rows, 'SoC', spec.soc);
      configRow(rows, 'CPU', spec.cpu);
      configRow(rows, 'Board', spec.board);
      // a native run's memory and boot mode are its command-line flags, which the page is not told
      configRow(rows, 'Memory', wasmMode ? `${manifest?.flash_mb || document.querySelector('#fw_flash')?.value || 8} MB flash · ${manifest?.psram_mb ?? document.querySelector('#fw_psram')?.value ?? 2} MB PSRAM` : 'as given on the command line');
      configRow(rows, 'Boot', !wasmMode ? 'as given on the command line' : manifest?.app_direct ? 'application directly' : 'mask ROM → 2nd stage → application');
      configRow(rows, 'Peripherals', spec.peripherals);
      configRow(rows, 'Engine', wasmMode ? 'WebAssembly · block/region JIT' : 'native esp32sim process');
      configRow(rows, 'Manifest', fw ? `${fw}.json` : wasmMode ? 'local browser files' : 'command-line flags');
      if (manifest?.stubs?.length) configRow(rows, 'Stubs', manifest.stubs.join(' · '));
      command.textContent = cliCommand(fw, manifest, boardName);
    };
    renderMachine();
    const localLinks = make('div', 'wb-links');
    localLinks.append(link('for agents ↗', 'index.html#agents'), link('CLI reference ↗', 'https://github.com/mikroverk/esp32sim/blob/main/docs/cli.md'));
    local.append(localLinks);
  }

  document.addEventListener('DOMContentLoaded', () => {
    window.Workbench = { enabled: true, manifest: null, openConfiguration: () => {} };
    window.addEventListener('esp32sim-board', (e) => { liveBoard = e.detail; if (renderMachine) renderMachine(); });
    document.body.classList.add('workbench');
    const header = document.querySelector('body > header');
    const title = header?.querySelector('h1');
    const right = document.querySelector('main > .right');
    const consoleCard = document.querySelector('#console')?.closest('.card');
    if (!header || !title || !right || !consoleCard) {
      document.documentElement.classList.remove('workbench-pending');
      return;
    }

    const brand = make('a', 'wb-brand');
    brand.href = 'index.html';
    brand.title = 'esp32sim: the landing page and every demo';
    const logo = make('img');
    logo.src = 'assets/esp32sim-logo-icon.png';
    logo.alt = '';
    brand.append(logo, make('span', '', 'esp32sim'));
    header.prepend(brand, make('span', 'wb-crumb', '/'));
    if (wasmMode) {   // a demo switcher: the same list as the landing page and the configuration dialog
      const pick = make('select', 'wb-demos');
      pick.setAttribute('aria-label', 'Switch demo');
      pick.append(new Option('Demos…', '', true, true));
      fetch('wasm/fw/demos.json', { cache: 'no-cache' }).then(r => r.ok ? r.json() : []).then(demos => {
        let parent = '';
        for (const d of demos) { const name = d.title.startsWith('…') ? parent + ' — ' + d.title.slice(1) : d.title; if (!d.title.startsWith('…')) parent = d.title; pick.append(new Option(name, d.fw, false, d.fw === query.get('fw'))); }
      }).catch(() => {});
      pick.onchange = () => { if (pick.value) location.href = 'run.html?wasm&fw=' + encodeURIComponent(pick.value); };
      header.append(pick);
    }

    const stage = make('section');
    stage.id = 'wbStage';
    const board = document.querySelector('main > .boardwrap');
    if (board) stage.append(board);
    for (const id of ['lcdpanel', 'netpanel', 'campanel', 'tftcard']) {
      const node = document.getElementById(id);
      if (node) stage.append(node);
    }
    right.prepend(stage);
    consoleCard.classList.add('wb-console-card');

    const rail = make('aside');
    rail.id = 'wbRail';
    right.append(rail);
    populateRail(rail);

    const panel = document.getElementById('fwpanel');
    if (panel) {
      const dialog = make('dialog');
      dialog.id = 'wbConfigDialog';
      const shell = make('div', 'wb-config-shell');
      const head = make('div', 'wb-config-head');
      const heading = make('div');
      heading.append(make('span', 'wb-kicker', 'Local machine setup'), make('h2', '', 'Configure firmware'), make('p', '', 'Choose the emulated board and provide firmware from this computer. Nothing is uploaded.'));
      const close = make('button', 'wb-config-close', '×');
      close.type = 'button';
      close.setAttribute('aria-label', 'Close configuration');
      close.onclick = () => dialog.close();
      head.append(heading, close);
      shell.append(head, panel);
      dialog.append(shell);
      document.body.append(dialog);

      const configure = make('button', 'wb-button wb-button-primary', '⚙ Configure');
      configure.type = 'button';
      configure.id = 'wbConfigure';
      const openConfiguration = () => { if (!dialog.open) dialog.showModal(); };
      configure.onclick = openConfiguration;
      header.insertBefore(configure, document.getElementById('audioBtn'));
      window.Workbench.openConfiguration = openConfiguration;
      if (!query.get('fw')) openConfiguration();

      const status = document.getElementById('status');
      if (status) new MutationObserver(() => {
        if (dialog.open && /running/i.test(status.textContent)) dialog.close();
      }).observe(status, { childList: true, characterData: true, subtree: true });
    } else {
      const configured = make('span', 'st', 'machine configured by local CLI');
      header.insertBefore(configured, document.getElementById('audioBtn'));
    }

    const restart = make('button', 'wb-button', '↻ Restart');
    restart.type = 'button';
    // In the browser build the machine lives in this page, so a reload is a restart. A native run
    // is another process: ask it to reset, as the board's button would.
    restart.onclick = () => { if (!wasmMode && typeof send === 'function') send({ t: 'reset' }); else location.reload(); };
    header.append(restart);
    const theme = make('button', 'wb-button wb-theme');
    theme.type = 'button';
    theme.title = 'Light or dark';
    theme.setAttribute('aria-label', 'Switch between light and dark');
    theme.onclick = () => {
      const root = document.documentElement;
      const dark = root.dataset.theme ? root.dataset.theme === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
      root.dataset.theme = dark ? 'light' : 'dark';
      try { localStorage.setItem('esp32sim-theme', root.dataset.theme); } catch (_) {}
    };
    header.append(theme);

    const updateTelemetry = () => {
      const runtime = document.getElementById('wbRuntime');
      const pace = document.getElementById('wbPace');
      const net = document.getElementById('netpanel')?.style.display === 'none' ? '' : document.getElementById('netstat')?.textContent;
      if (runtime) runtime.textContent = document.getElementById('stat')?.textContent || net || 'starting…';
      if (pace) pace.textContent = wasmMode ? (document.getElementById('pace')?.textContent || document.getElementById('status')?.textContent || 'loading…') : 'native emulator, over its WebSocket';
    };
    const telemetrySource = [document.getElementById('stat'), document.getElementById('pace'), document.getElementById('netstat'), document.getElementById('status')].filter(Boolean);
    const telemetryObserver = new MutationObserver(updateTelemetry);
    telemetrySource.forEach(node => telemetryObserver.observe(node, { childList: true, characterData: true, subtree: true }));
    updateTelemetry();

    const updateMode = () => {
      const visible = [...stage.children].some(node => node.id !== 'tftcard' && getComputedStyle(node).display !== 'none');
      right.classList.toggle('wb-console-only', !visible);
    };
    new MutationObserver(updateMode).observe(stage, { attributes: true, subtree: true, attributeFilter: ['style', 'class'] });
    new MutationObserver(updateMode).observe(document.body, { attributes: true, attributeFilter: ['class'] });
    updateMode();

    document.documentElement.classList.remove('workbench-pending');
  });
})();
