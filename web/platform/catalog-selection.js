const collator = new Intl.Collator('zh-CN', {numeric:true,sensitivity:'base'});
const manufacturerCollator = new Intl.Collator('en', {numeric:true,sensitivity:'base'});
export function manufacturerLabel(name) {
  const value = String(name || '').trim();
  const parts = value.match(/^(.+?)\s*[（(]([^（）()]+)[）)]$/);
  if (!parts) return value;
  const [,first,second] = parts.map(part=>part.trim());
  if (/\p{Script=Han}/u.test(first) && /^[A-Za-z]/.test(second) && !/\p{Script=Han}/u.test(second)) return `${second}（${first}）`;
  if (/^[A-Za-z]/.test(first) && /\p{Script=Han}/u.test(second)) return `${first}（${second}）`;
  return value;
}
const compareManufacturers = (a,b) => {
  const left=manufacturerLabel(a),right=manufacturerLabel(b);
  return Number(!/^[A-Za-z0-9]/.test(left))-Number(!/^[A-Za-z0-9]/.test(right)) || manufacturerCollator.compare(left,right);
};
export const productOptionLabel = product => product.runtime?.board ? product.name : `${product.name} × 尚未适配`;
export const manufacturerNames = products => [...new Set(products.map(item => item.manufacturer))].sort(compareManufacturers);
export const productsForManufacturer = (products, manufacturer, familyId = '') => products.filter(item => item.manufacturer === manufacturer && (!familyId || item.family?.id === familyId)).sort((a,b) => collator.compare(a.name.replace(/^M5Stack /,''),b.name.replace(/^M5Stack /,'')) || collator.compare(a.id,b.id));
export function productFamilies(products, manufacturer) {
  const families = new Map();
  for (const item of products) {
    if (item.manufacturer !== manufacturer || !item.family?.id || !item.family?.name) continue;
    const family = families.get(item.family.id) || {id:item.family.id,name:item.family.name,count:0};
    family.count++; families.set(family.id,family);
  }
  return [...families.values()].sort((a,b)=>compareManufacturers(a.name,b.name) || collator.compare(a.id,b.id));
}
export function productFamilyGroups(products) {
  const groups = new Map();
  for (const item of products) {
    const id = item.family?.id || '';
    if (!groups.has(id)) groups.set(id,{id,name:item.family?.name || '未分类',products:[]});
    groups.get(id).products.push(item);
  }
  return [...groups.values()].sort((a,b)=>Number(!a.id)-Number(!b.id) || compareManufacturers(a.name,b.name));
}
export function productIdentityLabel(product) {
  const revision = product.hardwareRevision;
  const knownRevision = revision && !/^(?:unspecified|unknown)(?:\b|;)|^(?:未注明|未知)(?:$|[;；、，])/i.test(revision);
  return [manufacturerLabel(product.manufacturer),product.family?.name,product.sku,knownRevision ? `硬件：${revision}` : null].filter(Boolean).join(' / ');
}
export function searchProducts(products, query) {
  const words = String(query).normalize('NFKC').toLowerCase().trim().split(/\s+/).filter(Boolean);
  return products.filter(item => { const text = [item.name,item.manufacturer,item.family?.name,item.family?.id,item.sku,item.id,item.module,item.chip,item.hardwareRevision,item.providers?.['lilygo-spark']?.productId,...(Array.isArray(item.aliases)?item.aliases:[])].filter(Boolean).join(' ').normalize('NFKC').toLowerCase(); return words.every(word => text.includes(word)); }).sort((a,b) => collator.compare(a.name,b.name));
}
export const accessoryBrand = item => item.manufacturer || 'DIY';
export const accessoryInterface = item => item.connector?.type || '未标注';
export const accessoryBrands = items => [...new Set(items.map(accessoryBrand))].sort(compareManufacturers);
export const accessoryInterfaces = items => [...new Set(items.map(accessoryInterface))].sort(collator.compare);
export function filterAccessories(items, {brand='', connector=''} = {}) {
  return items.filter(item => (!brand || accessoryBrand(item) === brand) && (!connector || accessoryInterface(item) === connector)).sort((a,b) => collator.compare(a.name,b.name));
}
export function searchAccessories(items, query) {
  return searchProducts(items.map(item => ({...item, manufacturer:accessoryBrand(item), module:[item.controller,accessoryInterface(item),item.connector?.protocol].filter(Boolean).join(' ')})),query);
}
export function firmwareGroups(firmwares, productId, defaultId) {
  const groups = new Map();
  const entries = firmwares.filter(firmware => firmware.productIds.includes(productId)).sort((a,b) => Number(b.role === 'factory')-Number(a.role === 'factory') || Number(b.id === defaultId)-Number(a.id === defaultId));
  for (const item of entries) {
    const label = item.publisherGroup || (item.publisher === 'M5Stack' || item.role === 'factory' ? '官方' : item.publisher || '社区');
    if (!groups.has(label)) groups.set(label,[]);
    groups.get(label).push(item);
  }
  return [...groups].sort(([a],[b]) => a === '官方' ? -1 : b === '官方' ? 1 : collator.compare(a,b));
}
export const firstFirmware = (firmwares, product) => firmwareGroups(firmwares,product.id,product.defaultFirmwareId)[0]?.[1][0] || null;
