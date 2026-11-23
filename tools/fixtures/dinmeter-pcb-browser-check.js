// Open the local platform URL with playwright_cli.sh, then run this file with run-code.
async page => {
  const base = new URL('./', page.url());
  const context = page.context();
  const sizes = {
    'm5stack-k134': {button: 'encoder-button', boardMaxZ: -6, rear: 'mounting-bracket', shell: 'shell-panel', rearOffset: -23},
    'm5stack-k134-v11': {button: 'encoder-press', boardMaxZ: -8.05, rear: 'stamp-s3a-cover', shell: 'shell', rearOffset: -22},
  };
  const views = {
    front: [0, 0, 250], back: [0, 0, -250],
    top: [0, 250, 0], bottom: [0, -250, 0],
    usb: [-250, 0, 5], rearAngle: [-140, -150, -225],
  };
  const preset = {
    gold: {color: '#d0a344', opacity: 1, finish: 'metal'},
    purple: {color: '#8456cf', opacity: .4, finish: 'plastic'},
  };
  const check = (condition, message) => { if (!condition) throw Error(message); };
  const screenshot = (canvas, id, view) => canvas.screenshot({path: `output/dinmeter-pcb/${id}-after-${view}.png`});
  const errors = [];
  const error = failure => errors.push(failure.message);
  page.on('pageerror', error);
  const scenePattern = '**/platform/scene-view.js';
  const anchor = 'scene.add(orbitGroup);orbitGroup.add(flightGroup);flightGroup.add(group);';
  const viewAnchor = 'return {stickers,setProduct, setAccessories';
  await context.route(scenePattern, async route => {
    const response = await route.fetch();
    const source = await response.text();
    check(source.includes(anchor) && source.includes(viewAnchor), 'scene-view fixture anchor missing');
    await route.fulfill({response, body: source
      .replace(anchor, anchor + ' window.dinMeterInspection={assembly:group};')
      .replace(viewAnchor, 'return window.dinMeterInspection.view = {stickers,setProduct, setAccessories')});
  });
  const report = {viewCameraPositions: views, products: {}};
  try {
    await page.setViewportSize({width: 1420, height: 1050});
    await page.goto(new URL('review-lane-02.html?id=m5stack-k134', base).href);
    await page.waitForFunction(() => document.body.dataset.ready === 'true');
    for (const [id, expected] of Object.entries(sizes)) {
      await page.evaluate(productId => laneReview.load(productId), id);
      const state = await page.evaluate(async expected => {
        const THREE = await import('./vendor/three.module.min.js');
        const {group, model} = laneReview.current;
        const {shell, rear} = expected;
        const board = model.parts['pcb-main'];
        const material = board.children[0].material;
        const partBox = part => new THREE.Box3().setFromObject(part);
        const partBounds = part => [partBox(part).min.toArray(), partBox(part).max.toArray()];
        const shellIsVisible = Object.values(model.parts).every(part => part.visible);
        return {
          board: partBounds(board.children[0]),
          assembled: partBounds(group),
          assembly: {shell: model.parts[shell].position.toArray(), rear: model.parts[rear].position.toArray()},
          label: Boolean(board.getObjectByName('pcb-main-unknown-front')?.material.map),
          color: '#' + material.color.getHexString(),
          opacity: material.opacity,
          inspection: group.userData.inspection?.expanded,
          knob: model.buttons.find(part => part.name === 'encoder-knob')?.userData.control,
          shellIsVisible,
        };
      }, expected);
      check(Math.abs(state.board[1][2] - expected.boardMaxZ) < .001, `${id} PCB seat was not loaded`);
      check(state.label && state.color === '#363c37' && state.opacity === 1, `${id} neutral UNKNOWN board`);
      check(state.knob === expected.button && state.shellIsVisible && state.inspection === false, `${id} assembled controls`);
      const canvas = page.locator('#render canvas');
      for (const [view, position] of Object.entries(views)) {
        await page.evaluate(({view, position, id}) => laneReview.show(view, {position, span: 64,
          up: view === 'top' || view === 'bottom' ? [0, 0, 1] : view === 'back' && id === 'm5stack-k134' ? [0, -1, 0] : [0, 1, 0],
        }), {view, position, id});
        await screenshot(canvas, id, view);
      }
      for (const [name, appearance] of Object.entries(preset)) {
        const independent = await page.evaluate(appearance => {
          laneReview.appearance({shell: appearance});
          const {group, model} = laneReview.current;
          const board = model.parts['pcb-main'].children[0].material;
          laneReview.show('rearAngle', {position: [-140, -150, -225], span: 64});
          return {
            board: '#' + board.color.getHexString(),
            opacity: board.opacity,
            shell: '#' + group.userData.shellMeshes[0].material.color.getHexString(),
            shellOpacity: group.userData.shellMeshes[0].material.opacity,
          };
        }, appearance);
        check(independent.board === state.color && independent.opacity === 1, `${id} PCB appearance changed`);
        check(independent.shell === appearance.color && independent.shellOpacity === appearance.opacity, `${id} shell appearance not applied`);
        await screenshot(canvas, id, name);
      }
      await page.evaluate(productId => laneReview.load(productId), id);
      await page.evaluate(() => {
        laneReview.current.group.userData.inspection.setExpanded(true);
        laneReview.show('expanded', {position: [150, 170, 160], span: 72});
      });
      await screenshot(canvas, id, 'expanded');
      report.products[id] = state;
    }
    await page.goto(new URL('index.html?lang=zh-CN', base).href);
    await page.waitForFunction(() => !!window.espTester);
    for (const [id, expected] of Object.entries(sizes)) {
      await page.evaluate(async productId => {
        const catalog = await import('./catalog.js');
        await espTester.configure(catalog.createDefaultScene(await catalog.loadCatalog(), productId));
      }, id);
      await page.waitForFunction(productId => document.querySelector('#scene-mount').dataset.modelStatus === 'ready' &&
        Boolean(window.dinMeterInspection?.assembly.getObjectByName(productId)?.userData.inspection), id);
      const slider = page.locator('#inspect-model');
      await slider.waitFor({state: 'visible'});
      check(await slider.inputValue() === '0', `${id} default assembly`);
      const positions = () => page.evaluate(({id,shell,rear}) => {
        const group = dinMeterInspection.assembly.getObjectByName(id);
        const names = [shell,rear,'pcb-main','usb-c','grove-a','grove-b',
          ...group.children.map(child=>child.name).filter(name=>name.startsWith('power-terminal-'))];
        return names.map(name=>group.getObjectByName(name).position.toArray());
      },{id,...expected});
      const assembled=await positions();
      await slider.press('End');
      await page.waitForFunction(() => document.getElementById('inspection-percent').textContent === '100%');
      const expanded=await positions();
      const offsets=[24,expected.rearOffset,...Array(expanded.length-2).fill(3)];
      check(expanded.every((position,index) => position.every((value,axis) =>
        Math.abs(value-assembled[index][axis]-(axis===2?offsets[index]:0))<1e-8)), `${id} incorrect housing or board attachment motion`);
      await page.evaluate(async () => {
        const THREE = await import('./vendor/three.module.min.js');
        const orientation = new THREE.Quaternion().setFromEuler(new THREE.Euler(-.88, .25, 0));
        dinMeterInspection.view.setOrientation(orientation.toArray(), {notify: false});
      });
      await screenshot(page.locator('#scene-mount canvas').first(), id, 'workbench-expanded');
      await page.locator('#reset-view').click();
      check(await slider.inputValue() === '0', `${id} reset`);
      const restored=await positions();
      check(JSON.stringify(restored) === JSON.stringify(assembled), `${id} assembly coordinates changed after reset`);
      const rect = await page.locator('#scene-mount canvas').first().boundingBox();
      const initialOrientation = await page.evaluate(() => dinMeterInspection.view.getOrientation());
      await page.mouse.move(rect.x + 12, rect.y + 12);
      await page.mouse.down();
      await page.mouse.move(rect.x + 75, rect.y + 55, {steps: 8});
      await page.mouse.up();
      const rotated = await page.evaluate(() => dinMeterInspection.view.getOrientation());
      check(JSON.stringify(initialOrientation) !== JSON.stringify(rotated), `${id} assembled model did not orbit`);
      report.products[id].workbench = {expanded, restored};
    }
    check(errors.length === 0, errors.join('; '));
    return report;
  } finally {
    await context.unroute(scenePattern);
    page.off('pageerror', error);
  }
}
