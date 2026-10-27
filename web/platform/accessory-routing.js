// Only this audited unit/host pair has an electrical model. Catalog metadata
// and arbitrary DIY pins cannot select or redirect a guest GPIO.
export const UNIT_BUTTON_ID = 'm5stack-unit-button-u027';
export const unitButtonControl = id => `accessory:${id}:button`;
export function unitButtonRoute(product, instance, definition) {
  const wiring=instance?.wiring;
  if(product?.id!=='m5stack-stopwatch'||product.runtime?.board!=='stopwatch'||definition?.id!==UNIT_BUTTON_ID||definition.model?.id!=='gpio-button-u027'||wiring?.mode!=='physical'||wiring.interfaceId!=='grove')return null;
  return {model:'gpio-button-u027',port:'grove',pin:11,control:'button'};
}
export function routedUnitButtons(scene,product,catalog) {
  return (scene?.accessories||[]).flatMap(instance=>{
    const definition=catalog?.accessories.find(item=>item.id===instance.catalogId),route=unitButtonRoute(product,instance,definition);
    return route?[{accessoryId:instance.id,...route}]:[];
  });
}
export const UNIT_RGB_ID = 'm5stack-unit-rgb-u003';
export function unitRgbRoute(product,instance,definition) {
  if(product?.id!=='m5stack-stopwatch'||product.runtime?.board!=='stopwatch'||definition?.id!==UNIT_RGB_ID||definition.model?.id!=='sk6812-u003'||instance?.wiring?.mode!=='physical'||instance.wiring.interfaceId!=='grove')return null;
  return {model:'sk6812-u003',port:'grove',pin:10,output:'grove-u003'};
}
export const physicalAccessoryRoute=(product,instance,definition)=>unitButtonRoute(product,instance,definition)||unitRgbRoute(product,instance,definition)||unitAngleRoute(product,instance,definition);
export function routedAccessories(scene,product,catalog) {
  return (scene?.accessories||[]).flatMap(instance=>{
    const definition=catalog?.accessories.find(item=>item.id===instance.catalogId),route=physicalAccessoryRoute(product,instance,definition);
    return route?[{accessoryId:instance.id,...route}]:[];
  });
}

export const UNIT_ANGLE_ID='m5stack-unit-angle-u005';
export function unitAngleRoute(product,instance,definition) {
  if(product?.id!=='m5stack-stopwatch'||product.runtime?.board!=='stopwatch'||definition?.id!==UNIT_ANGLE_ID||definition.model?.id!=='potentiometer-u005-10k-curve'||instance?.wiring?.mode!=='physical'||instance.wiring.interfaceId!=='grove')return null;
  return {model:'potentiometer-u005-10k-curve',port:'grove',pin:11,control:'knob'};
}
