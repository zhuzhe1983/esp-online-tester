import {validFirmwareArchive} from './firmware-archive.js';
import {createSparkReview} from './lilygo-spark-reviewed.js';
import {pinnedSparkSource} from './lilygo-spark-sources.js';
// Spark's public, bundled v2 catalog. The configured OSS mirror currently lacks
// browser CORS; use the official repository copy, not a third-party proxy.
export const LILYGO_SPARK_MANIFEST = 'https://raw.githubusercontent.com/Xinyuan-LilyGO/LILYGO-Spark/main/firmware_manifest_v2.json';
export const LILYGO_SPARK_HOME = 'https://github.com/Xinyuan-LilyGO/LILYGO-Spark';
const label=value=>typeof value==='string'?value.replace(/[\u0000-\u001f\u007f]/g,'').trim().slice(0,180):'';
const slug=value=>typeof value==='string'&&/^[a-z0-9][a-z0-9-]{0,179}$/.test(value)?value:null;
const digest=value=>typeof value==='string'&&/^[a-f0-9]{64}$/i.test(value)?value.toLowerCase():'';
export const isLilygo=product=>product?.manufacturer?.toLowerCase()==='lilygo';
// Deliberately exact: Spark's supported_product_ids includes known sibling-board
// false positives. These contracts also check the binary's repository/path.
const targets={
  'lilygo-ttgo-t-display':{deviceId:'t-display',files:/^(?:unigeek-t_display|firmware_tdisplay-esp32_src)\.bin$/i},
  'lilygo-t-display-s3':{deviceId:'t-display-s3',repo:'T-Display-S3',path:/^firmware\/(?:firmware_no_touch|tft_espi_test)\.bin$/i},
  'lilygo-t-deck':{deviceId:'t-deck',repo:'T-Deck',path:/^firmware\/(?:firmware|T-Deck_UnitTest_\d+)\.bin$/i,files:/^(?:T-Deck_UnitTest_\d+|Ultra-TDeck-v[\d.]+-merged|wadamesh-tdeck-merged|rift-[\w.-]+-tdeck-merged|Area512TDeck)\.bin$/i},
  'lilygo-t-embed-cc1101':{deviceId:'t-embed-cc1101',repo:'T-Embed-CC1101',path:/^firmware\/(?:T_Embed_CC1101_v[\w.]+|examples\/(?:cc1101_send_irq|encode_test|infrared_recv_test|infrared_send_test|nfc_test|test_display))\.bin$/i,files:/^(?:T_Embed_CC1101_v[\w.]+|unigeek-t_embed_cc1101|Bruce-lilygo-t-embed-cc1101)\.bin$/i},
  'lilygo-t-beam-v1-2':{deviceId:'t-beam',repo:'LilyGo-LoRa-Series',path:/^firmware\/hardware_test\/t-beam\/T_BEAM_SX1276_Factory_\d+\.bin$/i},
  'lilygo-t-watch-s3':{deviceId:'t-watch-s3',circuitPython:true},
};
function https(value) {try{const u=new URL(value);return u.protocol==='https:'&&!u.username&&!u.password&&!u.hash?u:null;}catch{return null;}}
function githubBinary(value) {
  const u=https(value);if(!u||u.search)return null;
  let parts;try{parts=decodeURIComponent(u.pathname).split('/').filter(Boolean);}catch{return null;}
  if(u.hostname==='raw.githubusercontent.com'&&parts.length>=4)return {owner:parts[0],repo:parts[1],path:parts.slice(3).join('/')};
  if(u.hostname==='github.com'&&parts[2]==='raw'&&parts.length>=5)return {owner:parts[0],repo:parts[1],path:parts.slice(parts[3]==='refs'?6:4).join('/')};
  return null;
}
// Spark indexes a mixture of manufacturer repositories and community releases.
function manufacturerRepo(value) {
  const u=https(value);
  if(!u||u.port||u.search||!['github.com','raw.githubusercontent.com'].includes(u.hostname))return null;
  const match=/^\/xinyuan-lilygo\/([a-z0-9_.-]+)(?:\/|$)/i.exec(u.pathname);
  return match?`https://github.com/Xinyuan-LilyGO/${match[1]}`:null;
}
function versionOrigin(project,version) {
  const community=project.source==='third_party'||Array.isArray(project.type)&&project.type.includes('third_party');
  let repo=manufacturerRepo(version.download_url);
  // Some manufacturer releases supply only the OSS ZIP and a repository.
  if(!version.download_url&&project.author?.name==='LILYGO'&&archiveSource({files:/\.bin$/i},version))repo=manufacturerRepo(project.source_code_url);
  return {repo,community};
}
function directTarget(target,value) {
  const u=https(value);if(!u||u.search||!u.pathname.toLowerCase().endsWith('.bin'))return false;
  if(target.circuitPython)return u.hostname==='downloads.circuitpython.org'&&/^\/bin\/lilygo_twatch_s3\/[a-z_]+\/adafruit-circuitpython-lilygo_twatch_s3-[\w.-]+\.bin$/i.test(u.pathname);
  const file=githubBinary(value);return !!(file&&file.owner.toLowerCase()==='xinyuan-lilygo'&&file.repo.toLowerCase()===target.repo?.toLowerCase()&&target.path?.test(file.path));
}
function archiveSource(target,version) {
  const hash=digest(version.sha256),entry=label(version.filename),url=https(version.oss_url);
  if(!hash||!target.files?.test(entry)||!url||url.hostname!=='lilygo.oss-accelerate.aliyuncs.com'||url.search||url.port)return null;
  if(url.pathname!==`/firmware/${hash.slice(0,16)}_${encodeURIComponent(entry)}.zip`)return null;
  const source={url:url.href,sha256:hash,archive:{format:'zip',entry}};
  return validFirmwareArchive(source)?source:null;
}
export function matchSparkDevice(product,devices) {
  if(!isLilygo(product))return null;
  const target=targets[product.id],declared=product.providers?.['lilygo-spark']?.productId;
  // Stable upstream identity is independent of our exact hardware/revision ID.
  // Keep old six-product scenes working, but never silently override a conflict.
  const deviceId=declared===undefined?target?.deviceId:slug(declared);
  if(!deviceId||(target&&deviceId!==target.deviceId))return null;
  const matches=devices.filter(item=>item.deviceId===deviceId);return matches.length===1?matches[0]:null;
}
function exactVersion(target,project,version) {
  const supported=version.supported_product_ids??project.supported_product_ids;
  if(!Array.isArray(supported)||!supported.includes(target.deviceId))return false;
  if(target.indexed)return true;
  if(directTarget(target,version.download_url))return true;
  // The ZIP's declared payload must still match the exact reviewed hardware.
  // Its SHA belongs to the extracted BIN, never to the ZIP container.
  return !version.download_url&&target.files?.test(label(version.filename))===true;
}
export function sparkFirmware(project,version,deviceId) {
  const source=version.source?{...version.source}:null;
  return {id:`lilygo-spark:${project.firmwareId}:${version.versionId}`,name:project.firmwareName,revision:version.versionName,version:version.versionName,
    simulationVerified:false,status:source?'trial':'reference',source,provider:'lilygo-spark',deviceId,firmwareId:project.firmwareId,versionId:version.versionId,
    detailsUrl:version.detailsUrl||project.detailsUrl||LILYGO_SPARK_HOME,downloadUrl:version.downloadUrl||null,sourceNote:version.sourceNote||'Spark 目录版本；运行时仍会核对芯片、容量与镜像布局。'};
}
const defaultReview=createSparkReview();
const reviewedNote='已核对本机型的厂商 BIN 和 Spark 版本的 SHA-256，可下载试运行。完整启动与外设表现待验证。';
function expandedVersion(result,origin,match) {
  if(!origin.community&&match&&origin.repo?.toLowerCase()===manufacturerRepo(match.source.url)?.toLowerCase()) {
    return {...result,source:match.source,downloadUrl:match.source.url,sourceNote:reviewedNote};
  }
  let sourceNote='Spark 将此版本列在当前产品下；发布方、硬件版次与镜像尚未核验。';
  if(origin.community)sourceNote='第三方项目，由 Spark 收录。当前硬件版次尚未核验，仅供查阅。';
  else if(origin.repo)sourceNote='此文件来自 LILYGO 仓库。Spark 将它列在此型号下，具体硬件版次与镜像尚未核验。';
  return {...result,source:null,sourceNote};
}
export function sparkHistoryFirmware(entry,review=defaultReview) {
  if(entry?.provider!=='lilygo-spark'||!digest(entry.sha256)||!slug(entry.firmwareId))return null;
  const target=targets[entry.productId];
  if(!target) {
    const match=review(entry.deviceId,entry.sha256);
    if(!match||entry.productId!==match.productId||entry.archive)return null;
    // A catalog refresh may prefer a newer commit containing identical bytes.
    if(entry.url!==match.source.url&&entry.url!==defaultReview(entry.deviceId,entry.sha256)?.source.url)return null;
    return sparkFirmware({firmwareId:entry.firmwareId,firmwareName:label(entry.name)||'LILYGO Spark 固件'},
      {versionId:entry.sha256,versionName:label(entry.version)||'版本未知',source:match.source,downloadUrl:match.source.url,detailsUrl:manufacturerRepo(match.source.url),sourceNote:reviewedNote},entry.deviceId);
  }
  if(entry.deviceId!==target.deviceId)return null;
  const archive=entry.archive&&validFirmwareArchive(entry)&&archiveSource(target,{filename:entry.archive.entry,oss_url:entry.url,sha256:entry.sha256});
  if(entry.archive?!archive:!directTarget(target,entry.url))return null;
  return {...sparkFirmware({firmwareId:entry.firmwareId,firmwareName:label(entry.name)||'LILYGO Spark 固件'},
    {versionId:entry.sha256,versionName:label(entry.version)||'版本未知',source:{url:entry.url,sha256:entry.sha256,...(archive?{archive:{...entry.archive}}:{}),...(['rom','app-direct'].includes(entry.bootMode)?{bootMode:entry.bootMode}:{})}},entry.deviceId)};
}
export function createLilygoSparkClient(fetchImpl=globalThis.fetch,catalog) {
  const review=catalog?createSparkReview(catalog):defaultReview;
  let cached=null,generatedAt='';
  async function manifest(signal) {
    if(signal?.aborted)signal.throwIfAborted();if(cached)return cached;
    const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),20000),abort=()=>controller.abort();signal?.addEventListener('abort',abort,{once:true});
    try {
      let response;try{response=await fetchImpl(LILYGO_SPARK_MANIFEST,{mode:'cors',credentials:'omit',headers:{Accept:'application/json'},signal:controller.signal});}
      catch(error){if(signal?.aborted)throw error;throw Error(controller.signal.aborted?'LILYGO Spark 请求超时，请重试。':'暂时无法读取 LILYGO Spark 目录，请检查网络或稍后重试。');}
      if(!response.ok)throw Error(response.status===429?'LILYGO Spark 请求过于频繁，请稍后重试。':`LILYGO Spark 返回 HTTP ${response.status}。`);
      if(!response.body)throw Error('LILYGO Spark 返回了空目录。');
      const reader=response.body.getReader(),chunks=[];let size=0;
      for(;;){const {done,value}=await reader.read();if(done)break;size+=value.length;if(size>2*1024*1024){await reader.cancel();throw Error('LILYGO Spark 元信息过大。');}chunks.push(value);}
      const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
      let data;try{data=JSON.parse(new TextDecoder().decode(bytes));}catch{throw Error('LILYGO Spark 返回了无效的 JSON。');}
      if(data?.schema_version!==2||!Array.isArray(data.product_list)||!Array.isArray(data.firmwares))throw Error('LILYGO Spark 目录格式有变。');
      generatedAt=label(data.generated_at).slice(0,10);cached=data;return data;
    }catch(error){if(controller.signal.aborted&&!signal?.aborted)throw Error('LILYGO Spark 请求超时，请重试。');throw error;}
    finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  }
  async function projects(deviceId,signal) {
    const data=await manifest(signal);
    if(!slug(deviceId)||catalogDevices(data).filter(item=>item.deviceId===deviceId).length!==1)throw Error('LILYGO Spark 型号无效。');
    const target=Object.values(targets).find(item=>item.deviceId===deviceId)||{deviceId,indexed:true};
    const rows=data.firmwares.flatMap(project=>{
      if(!project||!slug(project.id)||!label(project.name)||!Array.isArray(project.versions))return [];
      const versions=project.versions.flatMap((version,index)=>{
        if(!version||!exactVersion(target,project,version))return [];
        const hash=digest(version.sha256),origin=versionOrigin(project,version);
        const detailsUrl=https(project.source_code_url||project.homepage)?.href||origin.repo||LILYGO_SPARK_HOME;
        const result={versionId:hash||`reference-${index}`,versionName:label(version.version)||label(version.filename)||'版本未知',publishTime:label(version.published_at),detailsUrl};
        if(target.indexed)return [expandedVersion(result,origin,review(deviceId,hash))];
        const direct=directTarget(target,version.download_url),usableSize=!Number.isFinite(version.size)||version.size>=32768;
        const source=direct&&hash&&usableSize?{url:pinnedSparkSource(version.download_url,hash),sha256:hash}:!version.download_url&&usableSize?archiveSource(target,version):null;
        return [{...result,source,
          sourceNote:source?source.archive?'下载后自动解压并校验 BIN，自动识别完整启动或应用直启。':'Spark 目录版本；运行时仍会核对芯片、容量与镜像布局。':direct?'此条目缺少完整校验值或镜像信息；请在 Spark 核对后导入本地文件。':'此压缩包缺少可核验的 BIN 名称、下载地址或 SHA-256；目前仅供查阅。'}];
      });
      if(!versions.length)return [];
      const current=versions.find(v=>v.versionId===project.latest_sha256)||versions[0];
      const reviewed=target.indexed&&versions.find(v=>v.source);
      return [{firmwareId:project.id,firmwareName:label(project.name),currentVersionId:reviewed&&!current.source?reviewed.versionId:current.versionId,
        ...(reviewed?{sourceType:'OFFICIAL'}:{}),detailsUrl:current.detailsUrl,versions}];
    });
    if(target.indexed)rows.sort((a,b)=>Number(b.sourceType==='OFFICIAL')-Number(a.sourceType==='OFFICIAL'));
    return rows;
  }
  return {
    history:entry=>sparkHistoryFirmware(entry,review),
    describe:()=>`官方仓库目录${generatedAt?' '+generatedAt:''}`,
    async devices(signal) {return catalogDevices(await manifest(signal));},
    async search(deviceId,query='',page=1,signal) {
      if(!Number.isSafeInteger(page)||page<1)throw Error('LILYGO Spark 页码无效。');
      const words=label(query).toLowerCase().split(/\s+/).filter(Boolean),all=(await projects(deviceId,signal)).filter(project=>words.every(word=>[project.firmwareName,...project.versions.map(v=>v.versionName)].join(' ').toLowerCase().includes(word)));
      const rows=all.slice((page-1)*50,page*50);return {total:all.length,page,received:rows.length,rows,note:Object.values(targets).some(item=>item.deviceId===deviceId)?'仅列出已核对型号的条目。':'按 Spark 产品 ID 列出；有固定厂商镜像且校验值匹配的版本可试运行。'};
    },
    async versions(firmwareId,deviceId,signal) {const rows=await projects(deviceId,signal);return rows.find(item=>item.firmwareId===firmwareId)?.versions||[];},
  };
}
function catalogDevices(data) {
  return data.product_list.flatMap(group=>(Array.isArray(group?.products)?group.products:[group]).filter(item=>slug(item?.product_id)&&label(item.name)).map(item=>({deviceId:item.product_id,deviceName:label(item.name),...(slug(group?.id)?{familyId:group.id,familyName:label(group.name)}:{})})));
}
