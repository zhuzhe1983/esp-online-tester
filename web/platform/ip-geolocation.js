// Optional user-initiated lookup only. No scene, firmware, coordinates or IP are sent in the URL.
// Provider policy checked 2026-10-27: HTTPS/CORS, no key, 1,000/day shared per requesting domain.
export const IP_LOCATION_PROVIDER = Object.freeze({
  id: 'ipwho.is', name: 'IPWHOIS', endpoint: 'https://ipwho.is/?fields=success,message,latitude,longitude,city,region,country',
  documentationUrl: 'https://ipwhois.io/documentation', checkedAt: '2026-10-27',
});
const fail = (code, message) => Object.assign(new Error(message), {code});
const cleanLabel = value => typeof value === 'string' ? value.replace(/[\u0000-\u001f\u007f]/g, '').trim().slice(0, 128) : '';
export function parseIpLocation(data, fetchedAt = new Date().toISOString()) {
  if (!data || data.success !== true) throw fail('IP_LOCATION_UNAVAILABLE', 'IP 定位服务暂时不可用，请手动填写 GPS 数值。');
  if (typeof data.latitude !== 'number' || !Number.isFinite(data.latitude) || Math.abs(data.latitude) > 90 || typeof data.longitude !== 'number' || !Number.isFinite(data.longitude) || Math.abs(data.longitude) > 180) throw fail('IP_LOCATION_INVALID', '定位服务没有返回有效经纬度，请手动填写。');
  if (!Number.isFinite(Date.parse(fetchedAt))) throw fail('IP_LOCATION_INVALID', '定位获取时间无效。');
  const positionSource = {kind: 'ip-estimate', provider: IP_LOCATION_PROVIDER.id, fetchedAt};
  for (const key of ['city', 'region', 'country']) { const label = cleanLabel(data[key]); if (label) positionSource[key] = label; }
  return {latitude: data.latitude, longitude: data.longitude, positionSource};
}
export async function lookupIpLocation({fetchImpl = globalThis.fetch, signal, timeoutMs = 10000, now = () => new Date().toISOString()} = {}) {
  const controller = new AbortController(); let timedOut = false;
  const abort = () => controller.abort(); signal?.addEventListener('abort', abort, {once: true}); if (signal?.aborted) abort();
  const timer = setTimeout(() => { timedOut = true; controller.abort(); }, timeoutMs);
  try {
    const response = await fetchImpl(IP_LOCATION_PROVIDER.endpoint, {method: 'GET', mode: 'cors', credentials: 'omit', cache: 'no-store', referrerPolicy: 'no-referrer', signal: controller.signal});
    if (response.status === 429) throw fail('IP_LOCATION_RATE_LIMIT', 'IP 定位服务已达到调用限额，请稍后重试或手动填写。');
    if (!response.ok) throw fail('IP_LOCATION_UNAVAILABLE', 'IP 定位服务暂时不可用，请手动填写 GPS 数值。');
    const reader = response.body?.getReader(); let bytes;
    if (reader) {
      const chunks = []; let count = 0;
      const cancel = () => { void reader.cancel().catch(() => {}); }; controller.signal.addEventListener('abort', cancel, {once: true});
      try {
        while (true) { const {done, value} = await reader.read(); if (controller.signal.aborted) throw fail('IP_LOCATION_CANCELLED', '已取消 IP 定位。'); if (done) break; count += value.length; if (count > 32768) throw fail('IP_LOCATION_INVALID', '定位服务返回的数据过大，请手动填写。'); chunks.push(value); }
        bytes = new Uint8Array(count); let offset = 0; for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
      } catch (error) { await reader.cancel().catch(() => {}); throw error; }
      finally { controller.signal.removeEventListener('abort', cancel); reader.releaseLock(); }
    } else { bytes = new Uint8Array(await response.arrayBuffer()); if (bytes.length > 32768) throw fail('IP_LOCATION_INVALID', '定位服务返回的数据过大，请手动填写。'); }
    if (controller.signal.aborted) throw fail('IP_LOCATION_CANCELLED', '已取消 IP 定位。');
    let data; try { data = JSON.parse(new TextDecoder().decode(bytes)); } catch { throw fail('IP_LOCATION_INVALID', '定位服务返回的数据无法读取，请手动填写。'); }
    return parseIpLocation(data, now());
  } catch (error) {
    if (timedOut) throw fail('IP_LOCATION_TIMEOUT', 'IP 定位请求超时，请重试或手动填写。');
    if (controller.signal.aborted) throw fail('IP_LOCATION_CANCELLED', '已取消 IP 定位。');
    if (error.code) throw error;
    throw fail('IP_LOCATION_UNAVAILABLE', '无法访问 IP 定位服务（网络或 CORS 限制），可手动填写 GPS 数值。');
  } finally { clearTimeout(timer); signal?.removeEventListener('abort', abort); }
}
