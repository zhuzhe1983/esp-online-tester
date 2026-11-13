import {SD_CAPACITIES,MAX_SD_BYTES} from './sd-card-image.js';

// Shared controls; private card bytes never enter a scene/share URL.
export function createSdCardUi({mount, getApi, blocked, releaseInputs, onError}) {
  if(!document.querySelector('link[data-tf-settings]')){const sheet=document.createElement('link');sheet.rel='stylesheet';sheet.href=new URL('./sd-card-ui.css',import.meta.url).href;sheet.dataset.tfSettings='';document.head.append(sheet);}
  const node=(tag,text)=>{const value=document.createElement(tag);if(text)value.textContent=text;return value;};
  const root=node('section');root.className='sd-card-details';root.hidden=true;
  const heading=node('h3','虚拟 TF 卡'),status=node('p');status.className='helper';status.setAttribute('role','status');
  const note=node('p');note.className='helper';
  const controls=node('div');controls.className='sd-card-actions';
  const makeButton=(text,action)=>{const value=node('button',text);value.type='button';value.className='button quiet tiny';value.dataset.action=action;return value;};
  const importButton=makeButton('上传 IMG 镜像','import-sd-card'),exportButton=makeButton('导出 IMG 镜像','export-sd-card'),formatButton=makeButton('创建空卡…','clear-sd-card');
  const file=node('input');file.type='file';file.accept='.img,.bin,.ima';file.hidden=true;file.setAttribute('aria-label','上传 TF 卡磁盘镜像');
  const fields=node('div');fields.className='sd-card-format-fields';
  const formatLabel=node('label','空卡格式'),format=node('select');format.setAttribute('aria-label','TF 卡空卡格式');
  for(const [id,label]of [['fat32','FAT32（默认）'],['exfat','exFAT']])format.append(new Option(label,id));formatLabel.append(format);
  const sizeLabel=node('label','空卡容量'),size=node('select');size.setAttribute('aria-label','TF 卡空卡容量');for(const mb of SD_CAPACITIES)size.append(new Option(`${mb} MiB`,String(mb)));sizeLabel.append(size);
  fields.append(formatLabel,sizeLabel);
  const formatNote=node('p','格式选择只在创建空卡时生效。exFAT 需要固件自身支持；上传镜像保留原格式，不会自动转换。');formatNote.className='helper';
  const confirmation=node('div');confirmation.className='sd-card-confirm';confirmation.hidden=true;
  const confirmNote=node('p'),confirm=makeButton('确认创建空卡','confirm-clear-sd-card'),cancel=makeButton('取消','cancel-clear-sd-card');
  confirmation.append(confirmNote,confirm,cancel);
  controls.append(importButton,exportButton,file);root.append(heading,status,note,controls,fields,formatNote,formatButton,confirmation);mount.append(root);
  let state=null,busy=false,active=true;
  const render=()=>{
    root.hidden=!(state?.slot||state?.supported);
    const labels={unloaded:'正在准备 TF 卡',loading:'正在恢复 TF 卡…',prepared:'TF 卡已准备',saving:'正在保存 TF 卡…',saved:'TF 卡已保存',clean:state?.restored?'已恢复 TF 卡':'TF 卡已就绪','memory-only':'TF 卡仅保留在本页，浏览器持久化不可用',conflict:'其他页面已修改 TF 卡，请重新选择设备或加载固件',error:'TF 卡保存失败，请勿关闭页面'};
    const formatName={fat32:'FAT32',exfat:'exFAT',image:'原始镜像'}[state?.format]||'原始镜像';
    status.textContent=busy?'正在处理 TF 卡…':`${labels[state?.status]||state?.status||'尚未插卡'}${state?.bytes?` · ${state.bytes/1024/1024} MiB · ${formatName}`:''}${state?.present&&!state?.supported?' · 尚未接入固件':''}`;
    note.textContent=state?.mounted?'卡数据按当前设备与固件保存在此浏览器。上传镜像或创建空卡会替换当前卡并暂停设备；原有 Flash 配置保留。':state?.supported?'默认已准备 64 MiB FAT32 卡，载入固件后自动插入。可先上传或创建卡；已有固件保存的卡数据优先保留。':'此设备有 TF 卡槽，已默认准备空卡，也可保存自己的镜像。当前板型尚未模拟 TF 传输，固件暂时不能读写这张卡。';
    const disabled=busy||blocked()||!state?.present||['loading','saving','conflict'].includes(state?.status);
    for(const control of [importButton,exportButton,formatButton,confirm,format,size])control.disabled=disabled;
    cancel.disabled=busy;
  };
  async function operation(run){
    if(busy)return;busy=true;render();
    try{await releaseInputs();await run();state=(await getApi().observe()).sdCard||null;confirmation.hidden=true;}
    catch(error){onError(error);}
    finally{busy=false;if(active)render();}
  }
  importButton.addEventListener('click',()=>file.click());
  file.addEventListener('change',()=>{
    const selected=file.files?.[0];file.value='';if(!selected)return;
    void operation(async()=>{if(selected.size>MAX_SD_BYTES||selected.size<1024*1024||selected.size%(512*1024))throw Error('请选择 1–128 MiB、大小为 512 KiB 整数倍的完整 TF 卡 IMG 镜像。');await getApi().attachSdCard({bytes:new Uint8Array(await selected.arrayBuffer())});});
  });
  exportButton.addEventListener('click',()=>{void operation(async()=>{
    const result=await getApi().exportSdCard(),url=URL.createObjectURL(new Blob([result.bytes],{type:'application/octet-stream'}));
    const anchor=node('a');anchor.href=url;anchor.download=result.filename||'tf-card.img';document.body.append(anchor);anchor.click();anchor.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);
  });});
  formatButton.addEventListener('click',()=>{confirmNote.textContent=`将用 ${size.value} MiB ${format.value==='exfat'?'exFAT':'FAT32'} 空卡替换${state?.mounted?'当前设备与固件的卡':'此设备准备的卡'}，原卡文件会被清空。`;confirmation.hidden=false;});
  for(const field of [format,size])field.addEventListener('change',()=>{confirmation.hidden=true;});
  cancel.addEventListener('click',()=>{confirmation.hidden=true;});
  confirm.addEventListener('click',()=>{void operation(()=>getApi().attachSdCard({format:format.value,byteLength:Number(size.value)*1024*1024}));});
  const unsubscribe=getApi()?.subscribe(event=>{if(event.type==='sd-card'){state=event.sdCard;render();}else if(event.type==='state'){state=event.state.sdCard||null;confirmation.hidden=true;render();}});
  Promise.resolve(getApi()?.observe()).then(value=>{if(active){state=value?.sdCard||null;render();}}).catch(onError);
  return{refresh:render,dispose(){active=false;unsubscribe?.();root.remove();}};
}
