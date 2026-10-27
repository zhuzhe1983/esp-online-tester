// Exact audited products only. A U006 is four-wire parallel copper, not a mux.
export const UNIT_BUTTON_ID='m5stack-unit-button-u027';
export const UNIT_RGB_ID='m5stack-unit-rgb-u003';
export const UNIT_ANGLE_ID='m5stack-unit-angle-u005';
export const UNIT_DLIGHT_ID='m5stack-unit-dlight-u136';
export const UNIT_ENV4_ID='m5stack-unit-env4-u001d';
export const UNIT_HUB_ID='m5stack-unit-hub-u006';
export const HUB_PORTS=['port-1','port-2','port-3'];
export const unitButtonControl=id=>`accessory:${id}:button`;
const supportedHost=product=>product?.id==='m5stack-stopwatch'&&product.runtime?.board==='stopwatch';
const direct=instance=>instance?.wiring?.mode==='physical'&&instance.wiring.interfaceId==='grove'&&instance.wiring.viaAccessoryId===undefined&&instance.wiring.portId===undefined;
export function unitHubRoute(product,instance,definition) {
  return supportedHost(product)&&definition?.id===UNIT_HUB_ID&&definition.model?.id==='passive-hub-u006'&&direct(instance)?{model:'passive-hub-u006',port:'grove',hub:true}:null;
}
function hostRoute(product,instance,scene,catalog) {
  if(!supportedHost(product))return null;
  if(direct(instance))return {};
  const wiring=instance?.wiring;
  if(wiring?.mode!=='physical'||wiring.interfaceId!==undefined||!HUB_PORTS.includes(wiring.portId)||!wiring.viaAccessoryId||wiring.viaAccessoryId===instance.id)return null;
  const parent=scene?.accessories?.find(item=>item.id===wiring.viaAccessoryId),definition=catalog?.accessories?.find(item=>item.id===parent?.catalogId);
  return unitHubRoute(product,parent,definition)?{viaAccessoryId:parent.id,portId:wiring.portId}:null;
}
function leafRoute(product,instance,definition,scene,catalog,id,model,fields) {
  if(definition?.id!==id||definition.model?.id!==model)return null;
  const through=hostRoute(product,instance,scene,catalog);
  return through?{model,port:'grove',...fields,...through}:null;
}
export const unitButtonRoute=(p,i,d,s,c)=>leafRoute(p,i,d,s,c,UNIT_BUTTON_ID,'gpio-button-u027',{pin:11,control:'button'});
export const unitRgbRoute=(p,i,d,s,c)=>leafRoute(p,i,d,s,c,UNIT_RGB_ID,'sk6812-u003',{pin:10,output:'grove-u003'});
export const unitAngleRoute=(p,i,d,s,c)=>leafRoute(p,i,d,s,c,UNIT_ANGLE_ID,'potentiometer-u005-10k-curve',{pin:11,control:'knob'});
export const unitEnv4Route=(p,i,d,s,c)=>leafRoute(p,i,d,s,c,UNIT_ENV4_ID,'sht40-bmp280-u001d',{pins:[10,11],addresses:[0x44,0x76],control:'environment'});
export const unitDlightRoute=(p,i,d,s,c)=>leafRoute(p,i,d,s,c,UNIT_DLIGHT_ID,'bh1750fvi-u136',{pins:[10,11],addresses:[0x23],control:'illuminance'});
export const compatibleI2cRoutes=(a,b)=>Array.isArray(a.pins)&&Array.isArray(b.pins)&&a.pins.length===2&&a.pins.every((pin,index)=>pin===b.pins[index])&&Array.isArray(a.addresses)&&Array.isArray(b.addresses)&&!a.addresses.some(address=>b.addresses.includes(address));
export const physicalAccessoryRoute=(p,i,d,s,c)=>unitHubRoute(p,i,d)||unitButtonRoute(p,i,d,s,c)||unitRgbRoute(p,i,d,s,c)||unitAngleRoute(p,i,d,s,c)||unitEnv4Route(p,i,d,s,c)||unitDlightRoute(p,i,d,s,c);
export function routedAccessories(scene,product,catalog) {
  return (scene?.accessories||[]).flatMap(instance=>{
    const definition=catalog?.accessories.find(item=>item.id===instance.catalogId),route=physicalAccessoryRoute(product,instance,definition,scene,catalog);
    return route?[{accessoryId:instance.id,...route}]:[];
  });
}
export const routedUnitButtons=(s,p,c)=>routedAccessories(s,p,c).filter(route=>route.control==='button');
export const accessoryPoweredPath=route=>route.connected!==false&&route.effectiveConnected!==false;
