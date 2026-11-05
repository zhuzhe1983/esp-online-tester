// Firmware discovery is independent of scene identity, physical ports and board models.
const idPattern=/^[a-z0-9][a-z0-9:-]{0,79}$/;
export const firmwareTargetId=product=>product?.firmwareTargetId??product?.id;
export function validateFirmwareTargets(products) {
  const byId=new Map(products.map(product=>[product.id,product]));
  const invalid=message=>{throw Object.assign(new Error(message),{code:'CATALOG_SCHEMA'});};
  for(const product of products) {
    if(product.firmwareTargetId===undefined)continue;
    const id=product.firmwareTargetId,target=byId.get(id);
    if(typeof id!=='string'||!idPattern.test(id)||!target)invalid(`Invalid firmware target for ${product.id}.`);
    if(firmwareTargetId(target)!==target.id)invalid(`Firmware targets cannot form chains or cycles: ${product.id}.`);
    if(target.id!==product.id&&(!product.chip||product.chip!==target.chip))invalid(`Firmware target chip differs from ${product.id}.`);
  }
}
export const firmwareTargetProduct=(product,products)=>products.find(item=>item.id===firmwareTargetId(product))||null;

export function sharedFirmware(firmware,targetId) {
  if(!firmware)return null;
  if(firmware.sharedTargetId===targetId)return firmware;
  const result={...firmware,sharedTargetId:targetId,simulationVerified:false,hardwareValidated:false,
    status:firmware.source?.url||firmware.source?.manifestUrl?'trial':'reference',
    limitations:['来自共用固件目录，当前型号尚未验证；不作为本产品的出厂固件。',...(firmware.limitations||[]).map(note=>`原适配型号说明：${note}`)]};
  for(const key of ['role','validation','validationEvidence','verification','compatibility','artifactHistory'])delete result[key];
  return result;
}
export function firmwareForProduct(firmware,product) {
  if(!firmware||!product)return null;
  if(firmware.productIds?.includes(product.id))return firmware;
  const targetId=firmwareTargetId(product);
  return targetId!==product.id&&firmware.productIds?.includes(targetId)?sharedFirmware(firmware,targetId):null;
}
export const firmwaresForProduct=(firmwares,product)=>firmwares.map(firmware=>firmwareForProduct(firmware,product)).filter(Boolean).sort((a,b)=>Number(!!a.sharedTargetId)-Number(!!b.sharedTargetId));
