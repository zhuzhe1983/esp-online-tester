// ESP-IDF v5.5.1 esp_app_format/include/esp_app_desc.h: version at +16,
// project_name at +48; descriptor follows the image and first segment headers.
// https://github.com/espressif/esp-idf/blob/v5.5.1/components/esp_app_format/include/esp_app_desc.h
const clean = value => typeof value === 'string' ? value.replace(/[\u0000-\u001f\u007f]/g,'').trim().slice(0,120) : '';
function description(data, offset) {
  if (offset + 288 > data.length || data[offset] !== 0xe9 || data[offset+1] < 1 || data[offset+1] > 16) return null;
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const length = view.getUint32(offset+28,true), at = offset+32;
  if (length < 256 || at+length > data.length || view.getUint32(at,true) !== 0xabcd5432) return null;
  const string = start => {
    const bytes = data.subarray(start,start+32), end = bytes.indexOf(0);
    try { return clean(new TextDecoder('utf-8',{fatal:true}).decode(bytes.subarray(0,end < 0 ? 32 : end))); } catch { return ''; }
  };
  const name = string(at+48), version = string(at+16);
  return name || version ? {name,version} : null;
}
export function firmwareMetadata(files, manifest) {
  const found = new Map();
  for (const file of files) {
    if (!['app','flash'].includes(file.kind)) continue;
    const step = file.kind === 'app' ? file.data.length : 0x1000;
    if (!step) continue;
    for (let offset=0; offset<file.data.length; offset+=step) {
      const item = description(file.data,offset); if (item) found.set(JSON.stringify(item),item);
    }
  }
  // Multiple OTA versions do not establish which app will boot. Do not guess.
  if (found.size === 1) return {...found.values().next().value, source:'esp-app-description'};
  if (found.size > 1) return {name:'',version:'',source:'multiple-app-images'};
  return {name:clean(manifest?.name), version:clean(manifest?.version), source:'manifest'};
}
