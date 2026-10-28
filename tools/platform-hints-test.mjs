import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {productHints,firmwareHints,placeHint} from '../web/platform/hints.js';
for (const [file,group] of [['products',productHints],['firmwares',firmwareHints]]) {
 const catalog=JSON.parse(await readFile(new URL(`../web/platform/catalog/${file}.json`,import.meta.url),'utf8'));
 for(const item of catalog.items) {
  const hints=group(item), facts=hints.flatMap(hint=>hint.paragraphs);
  for(const fact of item.limitations) assert.ok(facts.includes(fact), `${item.id} dropped ${fact}`);
  assert.ok(hints.every(hint=>hint.icon&&hint.label&&hint.title));
 }
 console.log(`ok all ${file} limitation text retained in labeled hints`);
}
for(const viewport of [{width:390,height:844},{width:320,height:480},{width:1280,height:720},{left:20,top:40,width:280,height:400}]) {
 for(const anchor of [{left:0,top:0,bottom:25},{left:viewport.width-20,top:viewport.height-30,bottom:viewport.height-10},{left:60,top:200,bottom:230}]) {
  const p=placeHint(anchor,{width:326,height:540},viewport);
  assert.ok(p.left>=(viewport.left||0)+12);assert.ok(p.left+p.width<=(viewport.left||0)+viewport.width-12);
  assert.ok(p.top>=(viewport.top||0)+12);assert.ok(p.top+Math.min(540,p.maxHeight)<=(viewport.top||0)+viewport.height-12);
 }
}
console.log('ok tooltip placement stays inside desktop, mobile and zoomed viewports');
const unavailable={runtime:{status:'unavailable'},limitations:['Hardware support has not been implemented.']};
assert.equal(productHints(unavailable)[0].label,'资料与配置');
assert.equal(productHints({...unavailable,model:{type:'manufacturer-cad'}})[0].label,'仅外观预览');
console.log('ok metadata-only products do not claim an existing visual model');
