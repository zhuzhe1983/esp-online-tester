const collator = new Intl.Collator('zh-CN', {numeric:true,sensitivity:'base'});
export const productOptionLabel = product => product.runtime?.board ? product.name : `${product.name} × 尚未适配`;
export const manufacturerNames = products => [...new Set(products.map(item => item.manufacturer))].sort(collator.compare);
export const productsForManufacturer = (products, manufacturer) => products.filter(item => item.manufacturer === manufacturer).sort((a,b) => collator.compare(a.name.replace(/^M5Stack /,''),b.name.replace(/^M5Stack /,'')) || collator.compare(a.id,b.id));
export function searchProducts(products, query) {
  const words = String(query).normalize('NFKC').toLowerCase().trim().split(/\s+/).filter(Boolean);
  return products.filter(item => { const text = [item.name,item.manufacturer,item.sku,item.id,item.module,item.chip].filter(Boolean).join(' ').normalize('NFKC').toLowerCase(); return words.every(word => text.includes(word)); }).sort((a,b) => collator.compare(a.name,b.name));
}
export const accessoryBrand = item => item.manufacturer || 'DIY';
export const accessoryInterface = item => item.connector?.type || '未标注';
export const accessoryBrands = items => [...new Set(items.map(accessoryBrand))].sort(collator.compare);
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
