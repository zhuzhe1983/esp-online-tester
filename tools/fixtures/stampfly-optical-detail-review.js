async (page) => {
  const origin = new URL(page.url()).origin;
  const receipts = [];
  await page.setViewportSize({width:800,height:800});
  for (const sku of ['m5stack-k138','m5stack-k138-v11']) {
    await page.goto(`${origin}/tools/fixtures/stampfly-bottom-model-review.html?sku=${sku}`);
    await page.waitForFunction(() => document.body.dataset.ready === 'true');
    const initial = await page.evaluate(async () => {
      const hashes = {};
      for (const name of ['model.json','geometry.bin','pcb-back.webp','optical-flow-back.webp','optical-flow-package.webp','optical-flow-lens.webp','optical-flow-socket.webp']) {
        const response=await fetch(`/web/platform/assets/${review.sku}/${name}`,{cache:'no-store'}); if(!response.ok)throw Error(`Missing ${name}`);
        hashes[name]=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',await response.arrayBuffer()))).map(x=>x.toString(16).padStart(2,'0')).join('');
      }
      return {hashes,materials:review.materials(),positions:review.positions(),bounds:review.bounds(),inspectionError:review.model.group.userData.inspectionError||null,views:review.views};
    });
    if(initial.inspectionError)throw Error(initial.inspectionError);
    const prefix=`output/playwright/optical-detail/${sku}`;
    for(const view of ['back','bottomDetail','sideDetail','obliqueDetail','connectorDetail']) {
      await page.evaluate(v=>review.show(v),view);await page.locator('canvas').screenshot({path:`${prefix}/${view}.png`});
    }
    const appearances={gold:{shell:{color:'#d4af37',opacity:1,finish:'metal'}},purple:{shell:{color:'#8240bf',opacity:.35,finish:'clear-resin'}}};
    const materials={original:initial.materials};
    for(const [name,value]of Object.entries(appearances)) {
      materials[name]=await page.evaluate(value=>{review.show('obliqueDetail');return review.appearance(value);},value);
      await page.locator('canvas').screenshot({path:`${prefix}/${name}.png`});
      for(const key of Object.keys(initial.materials).filter(x=>x!=='frame'))if(JSON.stringify(materials[name][key])!==JSON.stringify(initial.materials[key]))throw Error(`Material isolation: ${sku}/${name}/${key}`);
    }
    await page.reload(); await page.waitForFunction(() => document.body.dataset.ready === 'true');
    const expanded=await page.evaluate(()=>{review.inspection(true);review.show('unknown');return {positions:review.positions(),hidden:Object.fromEntries(review.meta.presentation.inspectionGroups[0].hideParts.map(n=>[n,!review.model.parts[n].visible]))};});
    if(!Object.values(expanded.hidden).every(Boolean))throw Error('Inspection must hide all fixed wires');
    await page.locator('canvas').screenshot({path:`${prefix}/inspection.png`});
    const restored=await page.evaluate(()=>{review.inspection(false);review.show('obliqueDetail');return {positions:review.positions(),visible:Object.fromEntries(review.meta.presentation.inspectionGroups[0].hideParts.map(n=>[n,review.model.parts[n].visible])),materials:review.materials()};});
    if(JSON.stringify(restored.positions)!==JSON.stringify(initial.positions)||!Object.values(restored.visible).every(Boolean))throw Error('Inspection restore changed transform or visibility');
    await page.locator('canvas').screenshot({path:`${prefix}/restored.png`});
    receipts.push({productId:sku,initial,materials,inspection:{expanded,restored,passed:true},scope:'Actual production model loader. No firmware/hardware acceptance.'});
  }
  return receipts;
}
