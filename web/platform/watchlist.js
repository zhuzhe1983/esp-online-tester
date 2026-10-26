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
    } else if(entry.metadataProvider && entry.metadataProvider!=='github' || entry.metadataUrl!==`https://api.github.com/repos/${entry.upstreamRepo}/releases/latest`)throw new Error('Invalid GitHub metadata source.');
    if (!['reference-only', 'product-verified'].includes(entry.compatibility?.status)) throw new Error('Declare a compatibility status.');
    if (!Array.isArray(entry.compatibility.productIds) || !Array.isArray(entry.assets) || entry.assets.length > 8) throw new Error('Invalid watchlist compatibility or assets.');
    for (const asset of entry.assets) {
      safeUrl(asset.url); if (asset.upstreamUrl) safeUrl(asset.upstreamUrl); if (asset.sourceMetadataUrl) safeUrl(asset.sourceMetadataUrl);
      if (asset.sha256 && !/^[a-f0-9]{64}$/i.test(asset.sha256)) throw new Error('Invalid artifact hash.');
      if (asset.sourcePolicy === 'mutable-pinned' && !asset.sha256) throw new Error('Mutable official download endpoints require a fixed artifact hash.');
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

export async function checkWatchEntry(entry, { fetchImpl = globalThis.fetch, timeoutMs = 20000, signal } = {}) {
  const controller = new AbortController(), abort = () => controller.abort();
  signal?.addEventListener('abort', abort, { once: true }); if (signal?.aborted) abort();
  const timer = setTimeout(abort, timeoutMs), checkedAt = new Date().toISOString();
  try {
    // Visitors share anonymous GitHub API quotas (often a proxy's single IP).
    // Actions queries authenticated metadata; clients read only this static snapshot.
    const snapshot = await metadataJson(new URL('./catalog/firmware-release-snapshot.json', import.meta.url).href, { fetchImpl, signal: controller.signal });
    if (snapshot?.schemaVersion !== '1' || !Array.isArray(snapshot.entries)) throw new Error('Invalid release metadata snapshot.');
    const saved = snapshot.entries.find(item => item.id === entry.id && item.upstreamRepo === entry.upstreamRepo);
    const latest = saved?.release || {tag_name: entry.version, html_url: entry.releaseUrl, assets: []};
    const metadata = {source: 'site-snapshot', checkedAt: saved?.checkedAt || null,
      stale: !saved?.release || !Number.isFinite(Date.parse(saved.checkedAt)) || Date.now() - Date.parse(saved.checkedAt) > 48 * 3600000,
      refreshError: saved?.error?.message || (!saved?.release ? '尚无最近发布信息；保留已收录版本。' : null)};
    const candidates = Array.isArray(latest.assets) ? latest.assets : [];
    const assets = await Promise.all(entry.assets.map(async asset => ({...await probeFirmwareUrl(asset.url, { fetchImpl, signal: controller.signal }), name: asset.name, format: asset.format, sourcePolicy: asset.sourcePolicy})));
    const problems = assets.filter(asset => asset.available !== true), missing = assets.find(asset => asset.state === 'missing');
    return { id: entry.id, checkedAt, previousVersion: entry.version, latestVersion: latest.tag_name || null, updateAvailable: !!latest.tag_name && latest.tag_name !== entry.version,
      metadata,
      releaseUrl: safeUrl(latest.html_url || entry.releaseUrl), upstreamAssets: candidates.slice(0, 400).map(asset => ({ name: asset.name, url: safeUrl(asset.browser_download_url), bytes: asset.size, sha256: asset.digest?.startsWith('sha256:') ? asset.digest.slice(7) : null })),
      assets, state: problems.length || metadata.stale || metadata.refreshError ? 'attention' : 'ok', issueUrl: missing ? buildFirmwareIssueUrl({ entryId: entry.id, url: missing.url, status: missing.status, reason: 'Remote firmware link returned HTTP 404/410.' }) : null };
  } catch (error) {
    return { id: entry.id, checkedAt, state: 'attention', previousVersion: entry.version, latestVersion: entry.version, metadata: {source: 'site-snapshot', checkedAt: null, stale: true, refreshError: '站点发布信息暂不可用；保留已收录版本，可查看上游发布页。'}, error: { code: error.code || 'WATCH_METADATA_FETCH', message: error.message, status: error.status || null }, assets: [], issueUrl: null };
  } finally { clearTimeout(timer); signal?.removeEventListener('abort', abort); }
}
