// Remote release metadata only. Firmware payloads are never stored by this watchlist.
const ISSUE_BASE = 'https://github.com/zhuzhe1983/esp-online-tester/issues/new';
const MAX_METADATA_BYTES = 2 * 1024 * 1024;
const safeUrl = value => {
  const url = new URL(value);
  if (url.protocol !== 'https:' || url.username || url.password) throw new Error('Watchlist URLs must use HTTPS without credentials.');
  return url.href;
};

export function buildFirmwareIssueUrl({ entryId = 'custom-firmware', url = '', reason = 'Firmware source unavailable', status, productId = '' } = {}) {
  let publicUrl = '';
  try { const value = new URL(url); value.username = ''; value.password = ''; value.search = ''; value.hash = ''; publicUrl = value.href; } catch { /* Do not put arbitrary input in an issue URL. */ }
  const issue = new URL(ISSUE_BASE);
  issue.searchParams.set('title', `[Firmware source] ${String(entryId).slice(0, 100)}`);
  issue.searchParams.set('body', `Firmware: ${entryId}\nProduct: ${productId || 'unspecified'}\nSource: ${publicUrl}\nObserved: ${new Date().toISOString()}\nHTTP status: ${status ?? 'unknown'}\nProblem: ${String(reason).slice(0, 500)}\n\nPlease review the upstream link and compatibility metadata. URL query parameters were removed for privacy.\n`);
  return issue.href;
}

export function validateWatchlist(document) {
  if (document?.schemaVersion !== '1' || !Array.isArray(document.categories) || !Array.isArray(document.entries)) throw new Error('Invalid firmware watchlist schema.');
  if (document.categories.length > 50 || document.entries.length > 200) throw new Error('Watchlist exceeds its metadata limits.');
  const categories = new Set(document.categories.map(item => item.id)), ids = new Set();
  for (const entry of document.entries) {
    if (!/^[a-z0-9-]+$/.test(entry.id) || ids.has(entry.id) || !categories.has(entry.category)) throw new Error('Invalid watchlist ID or category.');
    ids.add(entry.id);
    if (!/^[\w.-]+\/[\w.-]+$/.test(entry.upstreamRepo)) throw new Error('Invalid upstream repository.');
    safeUrl(entry.releaseUrl); safeUrl(entry.metadataUrl);
    if (entry.browserInstallUrl) safeUrl(entry.browserInstallUrl);
    if (entry.browserAuditUrl) safeUrl(entry.browserAuditUrl);
    if(entry.metadataProvider==='m5burner') {
      if(entry.metadataUrl!=='https://m5burner-api.m5stack.com/api/firmware'||!/^\w{16,64}$/.test(entry.firmwareId))throw new Error('Invalid official M5Burner source.');
    } else if(entry.metadataProvider==='cloudzao') {
      if(!/^[1-9][0-9]{0,8}$/.test(entry.hardwareModelId||'')||!/^\w{28,64}$/.test(entry.firmwareId||'')||!['esp32s3','esp32s31'].includes(entry.targetChip)||entry.metadataUrl!==`https://admin.metalio.world/xiaozhi/api/firmwares?page=1&limit=1&hardwareModelId=${entry.hardwareModelId}&sourceType=official`)throw new Error('Invalid official CloudZao source.');
    } else if(entry.metadataProvider && entry.metadataProvider!=='github' || entry.metadataUrl!==`https://api.github.com/repos/${entry.upstreamRepo}/releases/latest`)throw new Error('Invalid GitHub metadata source.');
    if (!['reference-only', 'product-verified'].includes(entry.compatibility?.status)) throw new Error('Declare a compatibility status.');
    if (!Array.isArray(entry.compatibility.productIds) || !Array.isArray(entry.assets) || entry.assets.length > 8) throw new Error('Invalid watchlist compatibility or assets.');
    for (const asset of entry.assets) {
      safeUrl(asset.url); if (asset.upstreamUrl) safeUrl(asset.upstreamUrl); if (asset.sourceMetadataUrl) safeUrl(asset.sourceMetadataUrl);
      if (asset.sha256 && !/^[a-f0-9]{64}$/i.test(asset.sha256)) throw new Error('Invalid artifact hash.');
      if (asset.sourcePolicy === 'mutable-pinned' && !asset.sha256) throw new Error('Mutable official download endpoints require a fixed artifact hash.');
    }
    if(entry.releaseChannel!==undefined&&!['stable','rc','prerelease'].includes(entry.releaseChannel))throw new Error('Invalid release channel.');
    if(entry.releaseHistoryLimit!==undefined&&(!Number.isInteger(entry.releaseHistoryLimit)||entry.releaseHistoryLimit<1||entry.releaseHistoryLimit>30))throw new Error('Invalid release history limit.');
    if(entry.assetMetadata!==undefined&&(entry.assetMetadata!=='meshtastic-targets'||entry.upstreamRepo!=='meshtastic/firmware'))throw new Error('Invalid supplementary asset metadata source.');
    if(entry.catalogTargets!==undefined){
      if(!Array.isArray(entry.catalogTargets)||entry.catalogTargets.length>40)throw new Error('Invalid catalog targets.');
      const targets=new Set();
      for(const target of entry.catalogTargets){
        if(entry.assetMetadata==='meshtastic-targets'&&(!/^[a-z0-9-]{1,80}$/.test(target.boardTarget||'')||!['esp32c6','esp32s3'].includes(target.targetChip)||target.format!=='merged'||target.sourceType!=='upstream-release'))throw new Error('Invalid Meshtastic board identity.');
        if(!/^[a-z0-9][a-z0-9:-]{0,79}$/.test(target.firmwareId)||targets.has(target.firmwareId))throw new Error('Invalid or duplicate firmware target.');
        targets.add(target.firmwareId);
        if(!Array.isArray(target.productIds)||!target.productIds.length||new Set(target.productIds).size!==target.productIds.length||target.productIds.some(id=>!entry.compatibility.productIds.includes(id)))throw new Error('Target products must be explicitly declared by this watch entry.');
        if(!['stable','rc','prerelease'].includes(target.channel)||!['merged','app','manifest'].includes(target.format)||!['upstream-release','manufacturer-distribution'].includes(target.sourceType))throw new Error('Target needs a channel, format and source identity.');
        if(typeof target.assetNamePattern!=='string'||target.assetNamePattern.length>240||!target.assetNamePattern.startsWith('^')||!target.assetNamePattern.endsWith('$'))throw new Error('Asset selector must be a bounded, anchored pattern.');
        new RegExp(target.assetNamePattern);
        if(!['source','manifest-app'].includes(target.updateMode)||target.format==='app'&&target.updateMode!=='manifest-app')throw new Error('App-only releases need an explicit app manifest.');
        if(target.updateMode==='manifest-app'&&(target.format!=='app'||!/^\.\/firmware-plans\/[a-z0-9-]+\.json$/.test(target.manifestUrl||'')))throw new Error('Invalid app manifest target.');
        if(typeof target.nameTemplate!=='string'||!target.nameTemplate.includes('{version}'))throw new Error('Target needs a versioned display name.');
        if(target.downloadUrl)safeUrl(target.downloadUrl);
        if(!target.basis?.url||!target.basis?.note)throw new Error('Target needs a source mapping basis.');safeUrl(target.basis.url);
      }
    }
    if (!Number.isFinite(Date.parse(entry.checkedAt))) throw new Error('Watchlist needs a check timestamp.');
  }
  return document;
}

async function metadataJson(url, { fetchImpl = globalThis.fetch, signal } = {}) {
  const response = await fetchImpl(url, { cache: 'no-store', credentials: 'omit', mode: 'cors', signal });
  if (!response.ok) throw Object.assign(new Error(`Release metadata returned HTTP ${response.status}.`), { code: 'WATCH_METADATA_HTTP', status: response.status });
  if (Number(response.headers.get('content-length')) > MAX_METADATA_BYTES) throw new Error('Release metadata exceeds 2 MiB.');
  const reader = response.body?.getReader();
  if (!reader) throw new Error('Streaming fetch is required for bounded metadata reads.');
  const chunks = []; let length = 0;
  try {
    while (true) {
      const { done, value } = await reader.read(); if (done) break;
      length += value.length;
      if (length > MAX_METADATA_BYTES) throw new Error('Release metadata exceeds 2 MiB.');
      chunks.push(value);
    }
  } finally { await reader.cancel().catch(() => {}); }
  const bytes = new Uint8Array(length); let at = 0;
  for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
  return JSON.parse(new TextDecoder().decode(bytes));
}

export async function loadWatchlist({ fetchImpl = globalThis.fetch, signal } = {}) {
  return validateWatchlist(await metadataJson(new URL('./catalog/firmware-watchlist.json', import.meta.url).href, { fetchImpl, signal }));
}

export async function probeFirmwareUrl(value, { fetchImpl = globalThis.fetch, signal } = {}) {
  const url = safeUrl(value);
  try {
    let response = await fetchImpl(url, { method: 'HEAD', cache: 'no-store', credentials: 'omit', mode: 'cors', redirect: 'follow', signal });
    // Some origins disallow HEAD. Read zero body bytes even when Range is ignored.
    if ([405, 501].includes(response.status)) {
      await response.body?.cancel();
      response = await fetchImpl(url, { method: 'GET', headers: { Range: 'bytes=0-0' }, cache: 'no-store', credentials: 'omit', mode: 'cors', redirect: 'follow', signal });
    }
    const result = { url, status: response.status, available: response.ok, state: response.ok ? 'reachable' : [404, 410].includes(response.status) ? 'missing' : 'http-error', contentLength: Number(response.headers.get('content-length')) || null, bytesRead: 0 };
    await response.body?.cancel();
    return result;
  } catch (error) { return { url, state: signal?.aborted ? 'cancelled' : 'network-or-cors', available: null, status: null, bytesRead: 0, message: error.message }; }
}

const normalVersion=value=>String(value||'').replace(/^v/i,'').toLowerCase();
const prerelease=release=>release.prerelease===true||/(?:^|[.\d-])(?:rc|pre|alpha|beta|nightly)(?:[.\d-]|$)/i.test(release.tag_name||'');
function matchesChannel(release,channel){
  if(release.draft)return false;
  if(channel==='rc')return /(?:^|[.\d-])rc(?:[.\d-]|$)/i.test(release.tag_name||'');
  return channel==='prerelease'?prerelease(release):!prerelease(release);
}
function newerDeviceRelease(latest,currentVersion,releases){
  if(!latest?.tag_name||normalVersion(latest.tag_name)===normalVersion(currentVersion))return false;
  const a=normalVersion(latest.tag_name).match(/^(\d+)\.(\d+)(?:\.(\d+))?/),b=normalVersion(currentVersion).match(/^(\d+)\.(\d+)(?:\.(\d+))?/);
  if(a&&b)for(let i=1;i<=3;i++){const difference=Number(a[i]||0)-Number(b[i]||0);if(difference)return difference>0;}
  const previous=releases.find(release=>normalVersion(release.tag_name)===normalVersion(currentVersion));
  if(previous?.published_at&&latest.published_at)return Date.parse(latest.published_at)>Date.parse(previous.published_at);
  return true;
}
// The optional selection scope keeps project-wide checks backward compatible.
// Device checks never borrow another device's latest release or source probe.
export function selectWatchRelease(entry,saved,{firmwareId,productId,currentVersion,source}={}){
  const scoped=Boolean(firmwareId||productId);
  const targets=(entry.catalogTargets||[]).filter(target=>(!firmwareId||target.firmwareId===firmwareId)&&(!productId||target.productIds.includes(productId)));
  const target=targets.length===1?targets[0]:null;
  const assignment=(entry.deviceVersions||[]).find(item=>(!firmwareId||item.firmwareId===firmwareId)&&(!productId||item.productIds.includes(productId)));
  const previousVersion=currentVersion||assignment?.version||entry.version;
  if(!scoped)return {previousVersion,latest:saved?.release||{tag_name:entry.version,html_url:entry.releaseUrl,assets:[]},probeAssets:entry.assets,scope:'project',reason:null};
  const result={previousVersion,latest:null,probeAssets:[],scope:'device',reason:null};
  if(!target){result.reason=targets.length>1?'ambiguous-device-target':'no-device-target';return result;}
  const pattern=new RegExp(target.assetNamePattern),releases=(saved?.releases?.length?saved.releases:[saved?.release]).filter(Boolean).filter(release=>matchesChannel(release,target.channel)).sort((a,b)=>Date.parse(b.published_at||0)-Date.parse(a.published_at||0));
  for(const release of releases){
    const assets=(release.assets||[]).filter(asset=>asset.name.length<=240&&pattern.test(asset.name));
    if(assets.length>1){result.reason='ambiguous-device-assets';break;}
    if(assets.length===1){result.latest={...release,assets};break;}
  }
  if(!result.latest&&!result.reason)result.reason='no-matching-device-asset';
  result.updateAvailable=newerDeviceRelease(result.latest,previousVersion,releases);
  const sourceUrl=source?.url||(target.format==='manifest'&&source?.manifestUrl?.startsWith('https:')?source.manifestUrl:null)||target.downloadUrl;
  if(sourceUrl){
    const listed=entry.assets.find(asset=>asset.url===sourceUrl&&(!source?.sha256||target.updateMode==='manifest-app'||asset.sha256===source.sha256));
    result.probeAssets=[listed||{url:safeUrl(sourceUrl),name:new URL(sourceUrl).pathname.split('/').pop(),format:target.format}];
  }else{
    const candidates=entry.assets.filter(asset=>pattern.test(asset.name));
    // Opaque/manifests with several versions need the actual selected source;
    // probing every project asset would report failures for unrelated devices.
    if(candidates.length===1)result.probeAssets=candidates;
    else if(candidates.length>1)result.probeReason='selected-artifact-ambiguous';
  }
  return result;
}

export async function checkWatchEntry(entry, { fetchImpl = globalThis.fetch, timeoutMs = 20000, signal, firmwareId, productId, currentVersion, source } = {}) {
  const controller = new AbortController(), abort = () => controller.abort();
  signal?.addEventListener('abort', abort, { once: true }); if (signal?.aborted) abort();
  const timer = setTimeout(abort, timeoutMs), checkedAt = new Date().toISOString();
  const selection={firmwareId,productId,currentVersion,source};
  const previousVersion=currentVersion||(entry.deviceVersions||[]).find(item=>firmwareId&&item.firmwareId===firmwareId)?.version||entry.version;
  try {
    // Visitors read the static snapshot; never consume anonymous API quotas.
    const snapshot = await metadataJson(new URL('./catalog/firmware-release-snapshot.json', import.meta.url).href, { fetchImpl, signal: controller.signal });
    if (snapshot?.schemaVersion !== '1' || !Array.isArray(snapshot.entries)) throw new Error('Invalid release metadata snapshot.');
    const saved = snapshot.entries.find(item => item.id === entry.id && item.upstreamRepo === entry.upstreamRepo);
    const selected=selectWatchRelease(entry,saved,selection),latest=selected.latest;
    const scopeMessages={'no-device-target':'该固件尚无明确的设备版本规则，保留已收录版本。','ambiguous-device-target':'该设备有多条固件规则，需要明确选择固件。','no-matching-device-asset':'同步历史中未找到此设备对应的发布资产，保留已收录版本。','ambiguous-device-assets':'此设备的发布资产不唯一，暂不判定新版本。'};
    const metadata = {source:'site-snapshot',scope:selected.scope,checkedAt:saved?.checkedAt||null,
      stale:!latest||!Number.isFinite(Date.parse(saved?.checkedAt))||Date.now()-Date.parse(saved.checkedAt)>48*3600000,
      refreshError:saved?.error?.message||scopeMessages[selected.reason]||(!saved?.release?'尚无最近发布信息；保留已收录版本。':null),
      ...(selected.probeReason?{probeReason:selected.probeReason}:{}),...(selected.reason?{matchReason:selected.reason}:{})};
    const candidates=Array.isArray(latest?.assets)?latest.assets:[];
    const assets=await Promise.all(selected.probeAssets.map(async asset=>({...await probeFirmwareUrl(asset.url,{fetchImpl,signal:controller.signal}),name:asset.name,format:asset.format,sourcePolicy:asset.sourcePolicy})));
    const problems=assets.filter(asset=>asset.available!==true),missing=assets.find(asset=>asset.state==='missing');
    return {id:entry.id,firmwareId:firmwareId||null,productId:productId||null,checkedAt,previousVersion:selected.previousVersion,latestVersion:latest?.tag_name||null,
      updateAvailable:selected.scope==='device'?selected.updateAvailable===true:!!latest?.tag_name&&latest.tag_name!==entry.version,metadata,
      releaseUrl:safeUrl(latest?.html_url||entry.releaseUrl),upstreamAssets:candidates.slice(0,400).map(asset=>({name:asset.name,url:safeUrl(asset.browser_download_url),bytes:asset.size,sha256:asset.digest?.startsWith('sha256:')?asset.digest.slice(7):null})),
      assets,state:problems.length||metadata.stale||metadata.refreshError||metadata.probeReason?'attention':'ok',issueUrl:missing?buildFirmwareIssueUrl({entryId:firmwareId||entry.id,productId,url:missing.url,status:missing.status,reason:'Remote firmware link returned HTTP 404/410.'}):null};
  } catch (error) {
    return {id:entry.id,firmwareId:firmwareId||null,productId:productId||null,checkedAt,state:'attention',previousVersion,latestVersion:previousVersion,updateAvailable:false,metadata:{source:'site-snapshot',scope:firmwareId||productId?'device':'project',checkedAt:null,stale:true,refreshError:'站点发布信息暂不可用；保留已收录版本，可查看上游发布页。'},error:{code:error.code||'WATCH_METADATA_FETCH',message:error.message,status:error.status||null},assets:[],issueUrl:null};
  } finally { clearTimeout(timer); signal?.removeEventListener('abort', abort); }
}
