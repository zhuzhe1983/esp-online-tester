export const isPressControl = control => ['button','matrix-key'].includes(control.type);
export function shouldCaptureMappedKey(event,product,mapping) {
  const control=product?.controls?.find(item=>item.id===mapping?.[event.code]);
  return Boolean(isPressControl(control||{}) && (!(event.altKey||event.ctrlKey||event.metaKey)||control.type==='matrix-key'));
}
// Human pointer/keyboard holds keep stable control IDs; the runtime gets the
// device's actual input protocol after the shared hold/release controller.
export function controlInput(event,product) {
  if(event.type!=='button')return event;
  const accessory=/^accessory:(.+):button$/.exec(event.id);
  if(accessory)return {type:'accessory-button',accessoryId:accessory[1],id:'button',pressed:event.pressed};
  const control=product?.controls?.find(item=>item.id===event.id);
  return control?.type==='matrix-key'?{type:'matrix-key',row:control.row,column:control.column,pressed:event.pressed}:event;
}
