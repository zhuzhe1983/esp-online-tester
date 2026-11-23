import assert from 'node:assert/strict';
import {createLilygoSparkClient,LILYGO_SPARK_MANIFEST,matchSparkDevice,sparkFirmware,sparkHistoryFirmware} from '../web/platform/lilygo-spark.js';
import {createFirmwareHistory} from '../web/platform/firmware-history.js';
const hash='a'.repeat(64),otherHash='b'.repeat(64),pid='lilygo-t-display-s3';
const url='https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Display-S3/main/firmware/firmware_no_touch.bin';
const product={id:pid,manufacturer:'LILYGO',name:'LILYGO T-Display-S3 · non-touch'};
const devices=[{deviceId:'t-display-s3',deviceName:'T-Display S3'},{deviceId:'t-display-s3-pro',deviceName:'T-Display S3 Pro'}];
assert.equal(matchSparkDevice(product,devices),devices[0]);
assert.equal(matchSparkDevice({...product,id:'lilygo-t-display-s3-pro'},devices),null);
assert.equal(matchSparkDevice({...product,manufacturer:'M5Stack'},devices),null);
assert.equal(matchSparkDevice(product,[...devices,devices[0]]),null);
const version={version:'2023',filename:'firmware_no_touch.bin',download_url:url,sha256:hash,size:1048576};
const project={id:'factory-no-touch',name:'Non-touch demo',supported_product_ids:['t-display-s3'],versions:[version]};
const manifest={schema_version:2,generated_at:'2026-06-26T08:16:10Z',product_list:[null,{products:[{product_id:'t-display-s3',name:'T-Display S3'},{product_id:'t-display',name:'T-Display'}]}],firmwares:[
  project,
  {...project,id:'wrong-board',versions:[{...version,download_url:url.replace('/T-Display-S3/','/T-Display-S3-AMOLED/')}]},
  {...project,id:'wrong-version',versions:[{...version,supported_product_ids:['t-display-s3-pro']}]},
  {...project,id:'bad-credentials',versions:[{...version,download_url:url.replace('https://','https://user:password@')}]},
  {...project,id:'bad-sha',name:'Missing hash',versions:[{...version,sha256:'abc'}]},
  {...project,id:'small-asset',name:'Small asset',versions:[{...version,size:6640}]},
  {...project,id:'archive',versions:[{...version,download_url:url+'.zip'}]},
  {...project,id:'broken-url',versions:[{...version,download_url:url.replace('firmware_no_touch','%E0%A4%A')}]},
  {id:'unigeek',name:'UniGeek',supported_product_ids:['t-display'],versions:[{version:'1.7.2',filename:'unigeek-t_display.bin',oss_url:'https://lilygo.oss-accelerate.aliyuncs.com/fw.zip',sha256:hash}]},null,
]};
let calls=0;
const client=createLilygoSparkClient(async(request,options)=>{calls++;assert.equal(request,LILYGO_SPARK_MANIFEST);assert.equal(options.mode,'cors');assert.equal(options.credentials,'omit');assert.equal(options.headers.Authorization,undefined);return Response.json(manifest);});
assert.equal((await client.devices()).length,2);
const found=await client.search('t-display-s3');assert.deepEqual(found.rows.map(row=>row.firmwareId),['factory-no-touch','bad-sha','small-asset']);
assert.equal(calls,1,'Search and version selection use metadata only, never a BIN request');
const versions=await client.versions(project.id,'t-display-s3');assert.equal(versions.length,1);
const selected=sparkFirmware(found.rows[0],versions[0],'t-display-s3');assert.deepEqual(selected.source,{url,sha256:hash});assert.equal(selected.simulationVerified,false);
assert.equal((await client.versions('bad-sha','t-display-s3'))[0].source,null,'A truncated digest cannot authorize a mutable BIN URL');
assert.equal((await client.versions('small-asset','t-display-s3'))[0].source,null,'Known small bogus firmware metadata remains reference-only');
assert.equal((await client.versions('unigeek','t-display'))[0].source,null,'ZIP payload SHA is not a ZIP-container SHA');
assert.equal((await client.search('t-display-s3','noN-touch')).total,1);assert.equal((await client.search('t-display-s3','absent')).total,0);
assert.equal((await client.search('t-display-s3','',2)).rows.length,0);
assert.match(client.describe(),/2026-06-26/);
await assert.rejects(()=>client.search('t-display-s3-pro'),/型号无效/);
await assert.rejects(()=>client.search('t-display-s3','',0),/页码无效/);
const entry={provider:'lilygo-spark',firmwareId:project.id,deviceId:'t-display-s3',url,sha256:hash,kind:'merged',productId:pid,name:selected.name,version:selected.version,lastUsedAt:1};
assert.equal(sparkHistoryFirmware(entry).source.sha256,hash);
for(const bad of [{...entry,deviceId:'t-display'},{...entry,productId:'lilygo-t-display-s3-pro'},{...entry,url:url.replace('/T-Display-S3/','/T-Display-S3-Pro/')},{...entry,sha256:''}])assert.equal(sparkHistoryFirmware(bad),null);
const storage={value:'',getItem(){return this.value;},setItem(key,value){this.value=value;}};
const history=createFirmwareHistory(()=>storage);history.remember(entry);
assert.equal(createFirmwareHistory(()=>storage).list(pid)[0].provider,'lilygo-spark');
history.remember({...entry,sha256:otherHash,lastUsedAt:2});assert.equal(history.list(pid).length,2,'A changed content digest is a distinct Spark artifact');
assert.equal(history.list('another-device').length,0);
await assert.rejects(()=>createLilygoSparkClient(async()=>new Response('',{status:429})).devices(),/过于频繁/);
await assert.rejects(()=>createLilygoSparkClient(async()=>Response.json({schema_version:1})).devices(),/格式有变/);
await assert.rejects(()=>createLilygoSparkClient(async()=>new Response(null,{status:204})).devices(),/空目录/);
await assert.rejects(()=>createLilygoSparkClient(async()=>new Response(' '.repeat(2*1024*1024+1))).devices(),/元信息过大/);
await assert.rejects(()=>createLilygoSparkClient(async()=>{throw new TypeError('CORS');}).devices(),/无法读取/);
const controller=new AbortController();controller.abort();await assert.rejects(()=>client.devices(controller.signal),{name:'AbortError'});
const deckProduct={id:'lilygo-t-deck-plus-1',manufacturer:'LILYGO',providers:{'lilygo-spark':{productId:'t-deck-plus-1'}},family:{id:'t-deck-series',name:'T-Deck 系列'}};
const deckManifest={schema_version:2,product_list:[{id:'t-deck-series',name:'T-Deck Series',products:[{product_id:'t-deck',name:'T-Deck'},{product_id:'t-deck-plus-1',name:'T-Deck Plus'},{product_id:'t-deck-pro',name:'T-Deck Pro'}]}],firmwares:[
  {id:'plus',name:'Plus firmware',supported_product_ids:['t-deck-plus-1'],versions:[{...version,download_url:'https://example.com/plus.bin'},{...version,supported_product_ids:['t-deck-pro'],sha256:otherHash}]},
  {id:'pro',name:'Pro firmware',supported_product_ids:['t-deck-pro'],versions:[version]},
]};
let deckCalls=0;const deckClient=createLilygoSparkClient(async()=>{deckCalls++;return Response.json(deckManifest);});
const deckDevices=await deckClient.devices();
assert.equal(matchSparkDevice(deckProduct,deckDevices).deviceId,'t-deck-plus-1','Use upstream product ID, including its non-obvious -1 suffix');
assert.equal(matchSparkDevice({...deckProduct,providers:{'lilygo-spark':{productId:'t-deck-series'}}},deckDevices),null,'A family is not a firmware target');
assert.equal(matchSparkDevice({...product,providers:{'lilygo-spark':{productId:'t-deck-plus-1'}}},deckDevices),null,'A conflicting identity must not override an audited hardware target');
assert.deepEqual(deckDevices.map(item=>item.familyId),Array(3).fill('t-deck-series'));
const plusRows=await deckClient.search('t-deck-plus-1');assert.deepEqual(plusRows.rows.map(row=>row.firmwareId),['plus']);
assert.equal(plusRows.rows[0].versions.length,1,'Version target overrides prevent sibling-series leakage');
const plusFirmware=sparkFirmware(plusRows.rows[0],plusRows.rows[0].versions[0],'t-deck-plus-1');
assert.equal(plusFirmware.status,'reference');assert.equal(plusFirmware.source,null,'Unreviewed product identity alone never authorizes a binary');
assert.match(plusFirmware.sourceNote,/硬件版次/);assert.equal(deckCalls,1,'Expanded product browsing remains metadata-only');
await assert.rejects(()=>deckClient.search('t-deck-series'),/型号无效/);
const duplicateClient=createLilygoSparkClient(async()=>Response.json({...deckManifest,product_list:[...deckManifest.product_list,...deckManifest.product_list]}));
await assert.rejects(()=>duplicateClient.search('t-deck-plus-1'),/型号无效/);
console.log('ok Spark exact target contracts, version overrides, metadata-only CORS reads, raw BIN/hash boundary, ZIP references, search, cancellation and scoped history');

// A fixed ZIP describes one exact payload; its SHA is not the container digest.
const bruceHash='c6e5c5df56ec30bbbfc826075f3deccf1f809343a6bec7bdf09e9a00fc74146d',bruceFile='Bruce-lilygo-t-embed-cc1101.bin';
const bruceUrl=`https://lilygo.oss-accelerate.aliyuncs.com/firmware/${bruceHash.slice(0,16)}_${bruceFile}.zip`;
const bruce={id:'bruce-v1-10-3',name:'Bruce v1.10.3',supported_product_ids:['t-embed-cc1101','t-embed-cc1101-plus'],versions:[{version:'1.10.3',filename:bruceFile,sha256:bruceHash,oss_url:bruceUrl,size:4203216}]};
const sendHash='f6c96c4e9b89cc5dd05a835c43316d6f7c89733da9b077fece222a7f55bce73a',sendUrl='https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Embed-CC1101/master/firmware/examples/cc1101_send_irq.bin';
const sparkArchiveManifest={schema_version:2,product_list:[{product_id:'t-embed-cc1101',name:'T-Embed CC1101'}],firmwares:[bruce,{id:'cc1101-send-irq',name:'cc1101 send irq',supported_product_ids:['t-embed-cc1101'],versions:[{filename:'cc1101_send_irq.bin',download_url:sendUrl,sha256:sendHash}]}]};
const archives=createLilygoSparkClient(async()=>Response.json(sparkArchiveManifest));const bruceVersion=(await archives.versions(bruce.id,'t-embed-cc1101'))[0];
assert.deepEqual(bruceVersion.source,{url:bruceUrl,sha256:bruceHash,archive:{format:'zip',entry:bruceFile}});
const archivedEntry={...entry,productId:'lilygo-t-embed-cc1101',deviceId:'t-embed-cc1101',firmwareId:bruce.id,...bruceVersion.source};
assert.deepEqual(sparkHistoryFirmware(archivedEntry).source,bruceVersion.source);history.remember(archivedEntry);assert.deepEqual(history.list(archivedEntry.productId)[0].archive,bruceVersion.source.archive);
for(const change of [{url:bruceUrl.replace('lilygo.oss-accelerate.aliyuncs.com','evil.example')},{sha256:hash},{archive:{format:'zip',entry:'other-board.bin'}},{deviceId:'t-embed-cc1101-plus'}])assert.equal(sparkHistoryFirmware({...archivedEntry,...change}),null);
const restoredSend=(await archives.versions('cc1101-send-irq','t-embed-cc1101'))[0].source;assert.equal(restoredSend.sha256,sendHash);assert.equal(restoredSend.url,sendUrl.replace('/master/','/68ef3042980e51711c0318efd6cc944999e688db/'));
console.log('ok Spark ZIP identity/history and same-hash pinned recovery of deleted upstream paths');

// Spark indexes both CameraPlus-S3 revisions. Only the audited V1.2 LVGL image
// may run on this profile. Its old Spark path was moved upstream.
const camera='t-camera-plus-s3',cameraProduct='lilygo-t-camera-plus-s3';
const cameraHash='25b77114190ffef9b1c71532eb919fbea4e113076e9f837c0803835ad52acb0a';
const cameraUrl='https://raw.githubusercontent.com/Xinyuan-LilyGO/T-CameraPlus-S3/arduino-esp32-libs_V2.0.14/firmware/[T-CameraPlus-S3_V1.2][Lvgl_UI]_firmware_202504081446.bin';
const cameraProject={id:'camera-lvgl',name:'Camera LVGL',source:'tree',supported_product_ids:[camera,'t-deck-plus-1'],
  latest_sha256:otherHash,versions:[{version:'other PCB',sha256:otherHash,download_url:cameraUrl},{version:'V1.2',sha256:cameraHash,download_url:cameraUrl}]};
const cameraManifest={...deckManifest,product_list:[...deckManifest.product_list,{product_id:camera,name:'CameraPlus S3'}],firmwares:[
  {...cameraProject,id:'community-copy',source:'third_party'},
  {...cameraProject,id:'other-owner',versions:[{...cameraProject.versions[1],download_url:cameraUrl.replace('Xinyuan-LilyGO','somebody')}]},
  cameraProject,
]};
const cameras=createLilygoSparkClient(async()=>Response.json(cameraManifest));
const cameraRows=(await cameras.search(camera)).rows;
assert.equal(cameraRows[0].firmwareId,'camera-lvgl','Prioritize the confirmed image over unverified index tags');
assert.equal(cameraRows[0].currentVersionId,cameraHash,'Select the matching hardware over a newer unreviewed image');
const [otherCamera,matchingCamera]=cameraRows[0].versions;
assert.equal(otherCamera.source,null);assert.equal(otherCamera.detailsUrl,'https://github.com/Xinyuan-LilyGO/T-CameraPlus-S3');
const cameraFirmware=sparkFirmware(cameraRows[0],matchingCamera,camera);
assert.equal(cameraFirmware.downloadUrl,cameraFirmware.source.url);assert.match(cameraFirmware.source.url,/\/d2d6d36194569706c4a48b54d5af7b7d81f6592c\//);
assert.equal(cameraFirmware.simulationVerified,false);assert.equal(cameraFirmware.source.bootMode,'rom');
assert.equal(cameraRows[1].versions[1].source,null);assert.match(cameraRows[1].versions[1].sourceNote,/第三方/);assert.equal(cameraRows[2].versions[0].source,null);
assert.equal((await cameras.search('t-deck-plus-1')).rows.find(row=>row.firmwareId==='camera-lvgl').versions[1].source,null,'A shared tag cannot transfer a known digest across boards');
const cameraEntry={...entry,productId:cameraProduct,deviceId:camera,firmwareId:cameraProject.id,...cameraFirmware.source};
assert.deepEqual(cameras.history(cameraEntry).source,cameraFirmware.source);
for(const change of [{productId:pid},{url:cameraUrl},{sha256:otherHash},{archive:{format:'zip',entry:'old.bin'}}])assert.equal(cameras.history({...cameraEntry,...change}),null);

const localProduct={...deckProduct,providers:{'lilygo-spark':{productId:'t-deck-plus-1'}}};
const fixed={url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Deck/'+('1'.repeat(40))+'/firmware/board.bin',sha256:otherHash,bootMode:'app-direct'};
const catalogItem={productIds:[localProduct.id],status:'trial',role:'factory',publisher:'LILYGO',source:fixed};
const deckVersion={...cameraProject.versions[0],download_url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Deck/main/firmware/board.bin'};
const localZip={...deckVersion,download_url:undefined,filename:'board.bin',oss_url:`https://lilygo.oss-accelerate.aliyuncs.com/firmware/${otherHash.slice(0,16)}_board.bin.zip`};
const localManifest={...cameraManifest,firmwares:[
  {...cameraProject,versions:[deckVersion,cameraProject.versions[1]]},
  {...cameraProject,id:'official-zip',author:{name:'LILYGO'},source_code_url:'https://github.com/Xinyuan-LilyGO/T-Deck',versions:[localZip]},
  {...cameraProject,id:'other-vendor-board',versions:[{...deckVersion,download_url:cameraUrl}]},
]};
const withCatalog=(items,products=[localProduct])=>createLilygoSparkClient(async()=>Response.json(localManifest),{products,firmwares:items});
const localClient=withCatalog([catalogItem]);
const localVersions=await localClient.versions(cameraProject.id,'t-deck-plus-1');
assert.deepEqual((await localClient.versions('official-zip','t-deck-plus-1'))[0].source,fixed,'Use the verified same-byte BIN when Spark publishes an OSS ZIP');
assert.equal((await localClient.versions('other-vendor-board','t-deck-plus-1'))[0].source,null,'Different manufacturer repositories retain distinct identities');
assert.deepEqual(localVersions[0].source,fixed);assert.equal(localVersions[1].source,null);
const localEntry={...entry,firmwareId:cameraProject.id,productId:localProduct.id,deviceId:'t-deck-plus-1',...fixed};
assert.deepEqual(localClient.history(localEntry).source,fixed);
for(const client of [
  withCatalog([{...catalogItem,publisher:'Other'}]),withCatalog([{...catalogItem,status:'reference'}]),
  withCatalog([{...catalogItem,source:{...fixed,url:fixed.url.replace('1'.repeat(40),'main')}}]),
  withCatalog([catalogItem],[localProduct,{...localProduct,id:'duplicate-upstream-id'}]),
]) {
  assert.equal((await client.versions(cameraProject.id,'t-deck-plus-1'))[0].source,null);
  assert.equal(client.history(localEntry),null);
}
console.log('ok audited Spark image priority and direct downloads, manufacturer sources, PCB and publisher boundaries, exact catalog digest matching and history');
