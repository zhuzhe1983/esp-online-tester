async (page) => {
  // Open the task-owned review-lane-02 page before invoking through playwright-cli
  // run-code. This reads actual renderer meshes; it does not edit model assets.
  await page.waitForFunction(() => document.body.dataset.ready === 'true' && !!window.laneReview);
  const products = [];
  for (const id of ['m5stack-s007-v033', 'm5stack-s007-pin127-v033', 'm5stack-s007-pin254-v033']) {
    products.push(await page.evaluate(async id => {
      const THREE = await import('./vendor/three.module.min.js');
      await laneReview.load(id);
      const {group, meta} = laneReview.current;
      group.userData.inspection.setExpanded(false);
      group.updateMatrixWorld(true);
      const digest = async bytes => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), b => b.toString(16).padStart(2, '0')).join('');
      const base = new URL(`./assets/${id}/`, location.href);
      const metadataBytes = await (await fetch(new URL('model.json', base), {cache: 'no-store'})).arrayBuffer();
      if (JSON.stringify(JSON.parse(new TextDecoder().decode(metadataBytes))) !== JSON.stringify(meta)) throw Error('Metadata changed after renderer load');
      const binaryBytes = await (await fetch(new URL(meta.binary, base), {cache: 'no-store'})).arrayBuffer();
      const geometrySha256 = await digest(binaryBytes);
      if (geometrySha256 !== meta.artifacts[meta.binary]) throw Error('Geometry hash differs from renderer contract');
      const rules = {
        'cover-key:button': {kind: 'interaction-hotspot', reason: 'Transparent zero-opacity button hit plane, not the physical cover flexure; native shell geometry remains included.'},
        'pcb-main-unknown-front': {kind: 'unknown-annotation', reason: 'UNKNOWN text plane at PCB front plus0.015mm is provenance annotation, not PCB material; the actual PCB solid remains included.'},
        'pcb-back-photo': {kind: 'photo-overlay', reason: 'Masked photograph plane0.015mm behind the PCB reproduces appearance on an existing board; it is not separate paper or a physical laminate.'},
        'screw-socket-mark': {kind: 'appearance-overlay', reason: 'Declared dark hexagonal visual mark over the donor screw head; not a physical cap or machined socket. The complete native cover-screw remains included.'},
      };
      const declared = name => meta.derivedParts.find(part => part.id === name);
      if (declared('pcb-back-photo')?.kind !== 'label') throw Error('Unexpected PCB photo entity');
      if (!/visual mark/i.test(declared('screw-socket-mark')?.notes || '')) throw Error('Socket exclusion lacks explicit appearance-only provenance');
      const physical = new THREE.Box3(), excluded = [], included = [];
      const boxData = b => ({min: b.min.toArray(), max: b.max.toArray(), size: b.getSize(new THREE.Vector3()).toArray()});
      group.traverse(object => {
        if (!object.isMesh || !object.geometry) return;
        object.geometry.computeBoundingBox();
        const bounds = object.geometry.boundingBox.clone().applyMatrix4(object.matrixWorld);
        const meshId = object.name || `${object.parent.name}/mesh[${object.parent.children.indexOf(object)}]`;
        const rule = rules[meshId];
        if (rule) {
          if (meshId === 'cover-key:button' && object.material.opacity !== 0) throw Error('Expected invisible interaction plane');
          excluded.push({id: meshId, ...rule, boundsMm: boxData(bounds), declaration: declared(meshId)?.notes || null});
        } else {
          physical.union(bounds);
          const native = meta.parts.find(part => part.name === meshId);
          included.push({id: meshId, classification: native ? 'native-entity' : meshId === 'pcb-main/mesh[0]' ? 'derived-pcb-solid' : 'derived-entity', bounds});
        }
      });
      if (excluded.length !== Object.keys(rules).length || new Set(excluded.map(x => x.id)).size !== excluded.length) throw Error('Exclusion inventory mismatch');
      if (included.filter(x => x.classification === 'native-entity').length !== meta.parts.length) throw Error('A native physical entity was omitted');
      const physicalDerivedIds = meta.derivedParts.filter(p => !['pcb-back-photo', 'screw-socket-mark'].includes(p.id)).map(p => p.id === 'pcb-main' ? 'pcb-main/mesh[0]' : p.id);
      if (physicalDerivedIds.some(id => !included.some(x => x.id === id))) throw Error('A physical derived entity was omitted');
      const extrema = {};
      for (let axis = 0; axis < 3; axis++) {
        extrema['xyz'[axis]] = {};
        for (const edge of ['min', 'max']) extrema['xyz'[axis]][edge] = included.filter(x => Math.abs(x.bounds[edge].getComponent(axis) - physical[edge].getComponent(axis)) < 1e-6).map(x => x.id);
      }
      return {
        productId: id,
        modelSha256: await digest(metadataBytes), geometrySha256,
        fullRendererBounds: boxData(new THREE.Box3().setFromObject(group)),
        physicalAssemblyBounds: {...boxData(physical), unit: 'mm'},
        excludedObjects: excluded,
        includedNativePartCount: included.filter(x => x.classification === 'native-entity').length,
        includedPhysicalMeshIds: included.map(x => x.id),
        extremaContributors: extrema,
        independentPaperParts: {count: 0, reason: 'These three SKU declarations contain no installed independent paper part; the loose supplied pin sticker is not installed. No physical paper part is removed.'},
      };
    }, id));
  }
  return {checkedAt: new Date().toISOString(), browser: page.context().browser().version(), products};
}
