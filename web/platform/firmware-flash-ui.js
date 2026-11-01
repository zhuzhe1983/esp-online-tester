// Every physical write starts with a fresh confirmation. Its final click must
// call requestPort synchronously so Web Serial retains the user gesture.
export function openFlashConfirmation({product, source, onConfirm, onClose}) {
  const node=(tag,text)=>{const value=document.createElement(tag);if(text)value.textContent=text;return value;};
  const dialog=node('dialog');dialog.className='firmware-run-confirm firmware-flash-confirm';dialog.setAttribute('aria-labelledby','firmware-flash-confirm-title');
  const title=node('h2','确认刷写实体设备');title.id='firmware-flash-confirm-title';
  const identity=node('p',product.name);identity.className='flash-target';
  const origin=node('p',source.file?.name || (source.files ? `本地固件 · ${source.files.length} 个文件` : source.url || source.manifestUrl));origin.className='helper flash-source';
  const layout=node('p','完整 merged 镜像按合并镜像写入；分区固件按烧录清单中的实际偏移逐段写入。缺少启动文件或分区信息时会停止并提示补齐。');layout.className='helper';
  const disclaimer=node('p','固件均来自第三方发布者或由用户自行制作，本平台不保证其兼容性、稳定性或安全性。刷写可能覆盖设备数据，或导致设备无法正常启动。');
  const responsibility=node('p','请在刷机前自行备份设备数据，并自行承担使用和刷写固件的风险及后果。');
  const acknowledgement=node('label');acknowledgement.className='flash-acknowledgement';
  const checked=node('input');checked.type='checkbox';
  acknowledgement.append(checked,document.createTextNode('我已自行备份，并了解和接受上述风险与后果'));
  const actions=node('div');actions.className='dialog-actions';
  const cancel=node('button','取消');cancel.type='button';cancel.className='button';
  const proceed=node('button','确认并选择设备');proceed.type='button';proceed.className='button primary';proceed.disabled=true;
  checked.addEventListener('change',()=>{proceed.disabled=!checked.checked;});
  cancel.addEventListener('click',()=>dialog.close());
  let submitted=false;
  proceed.addEventListener('click',()=>{
    if(!checked.checked||submitted)return;
    submitted=true;proceed.disabled=true;dialog.close();onConfirm();
  });
  dialog.addEventListener('close',()=>{dialog.remove();onClose?.();},{once:true});
  actions.append(cancel,proceed);dialog.append(title,identity,origin,layout,disclaimer,responsibility,acknowledgement,actions);
  document.body.append(dialog);dialog.showModal();cancel.focus();
  return {close:()=>dialog.close()};
}
