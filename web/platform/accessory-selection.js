import {accessoryBrand,accessoryBrands,accessoryInterface,filterAccessories} from './catalog-selection.js';

export const ALL_ACCESSORY_INTERFACES = '*';
const SIGNAL_ONLY_INTERFACES = new Set(['GPIO','I2C','I²C','SPI','QSPI','SDMMC','UART','I2S','I²S','CAN','RS485','USB']);

// Mechanical connector types only. Internal buses and unverified ports never
// become attachment choices; equal connector names do not imply pin compatibility.
export function productAccessoryInterfaces(product) {
  const types=new Map();
  for(const port of product?.interfaces||[]) {
    if(port.external!==true||typeof port.type!=='string'||!port.type.trim())continue;
    const type=port.type.trim();
    if(SIGNAL_ONLY_INTERFACES.has(type.toUpperCase()))continue;
    if(!types.has(type))types.set(type,{type,names:[]});
    const names=types.get(type).names;
    if(port.name&&port.name!==type&&!names.includes(port.name))names.push(port.name);
  }
  return [...types.values()].map(item=>({...item,label:item.names.length===1&&item.names[0].startsWith(item.type)?item.names[0]:item.names.length?`${item.type} · ${item.names.join(' / ')}`:item.type}));
}

export function accessoryChoicesForProduct(product,items,preferred={}) {
  const interfaces=productAccessoryInterfaces(product);
  const supported=type=>interfaces.some(item=>item.type===type);
  const connector=preferred.browseAll||preferred.connector===ALL_ACCESSORY_INTERFACES?ALL_ACCESSORY_INTERFACES:
    supported(preferred.connector)?preferred.connector:
    interfaces.find(port=>items.some(item=>accessoryInterface(item)===port.type))?.type||interfaces[0]?.type||'';
  const matching=connector===ALL_ACCESSORY_INTERFACES?items:connector?items.filter(item=>accessoryInterface(item)===connector):[];
  const brands=accessoryBrands(matching),brand=brands.includes(preferred.brand)?preferred.brand:brands.includes(product?.manufacturer)?product.manufacturer:brands[0]||'';
  const choices=filterAccessories(matching,{brand}),id=choices.some(item=>item.id===preferred.id)?preferred.id:choices[0]?.id||'';
  const notice=connector===ALL_ACCESSORY_INTERFACES?'正在浏览全部配件，未按当前设备接口筛选。':
    !interfaces.length?'外部接口资料待核对；可选择“所有接口”浏览配件。':
    !matching.length?'此接口暂无收录配件。':'按当前设备的外部接口筛选；连接方式在加入后设置。';
  return {interfaces,connector,brands,brand,choices,id,notice};
}

export function accessorySearchPreference(product,item) {
  const connector=accessoryInterface(item);
  return {brand:accessoryBrand(item),connector,id:item.id,browseAll:!productAccessoryInterfaces(product).some(port=>port.type===connector)};
}
