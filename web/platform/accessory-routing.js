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
