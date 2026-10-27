import {createM5BurnerClient,matchBurnerDevice,burnerFirmware,burnerHistoryFirmware} from './m5burner.js';

export function mountM5Burner({mount,onChange,client=createM5BurnerClient()}) {
  if(!mount)return null;
  const node=(tag,text)=>{const el=document.createElement(tag);if(text)el.textContent=text;return el;};
  const identity=node('p');identity.className='helper';
  const searchRow=node('div');searchRow.className='input-row burner-search-row';
  const query=node('input');query.type='search';query.maxLength=120;query.setAttribute('aria-label','搜索当前设备的 M5Burner 固件');
  const search=node('button','搜索');search.type='button';search.className='button';searchRow.append(query,search);
  const message=node('p');message.className='helper';message.setAttribute('role','status');
  const firmwareLabel=node('label','固件');firmwareLabel.className='field-label';firmwareLabel.htmlFor='burner-firmware';
  const firmwareSelect=node('select');firmwareSelect.id=firmwareLabel.htmlFor;
  const next=node('button','加载更多');next.type='button';next.className='button quiet tiny';next.hidden=true;
  const versionLabel=node('label','版本');versionLabel.className='field-label';versionLabel.htmlFor='burner-version';
  const versionSelect=node('select');versionSelect.id=versionLabel.htmlFor;
  const detail=node('a','在 M5Burner 查看 ↗');detail.className='text-link';detail.target='_blank';detail.rel='noopener noreferrer';detail.hidden=true;
  mount.append(identity,searchRow,message,firmwareLabel,firmwareSelect,next,versionLabel,versionSelect,detail);
  let product=null,device=null,rows=[],versions=[],selected=null,page=0,received=0,total=0,searchText='',timer;
  let requestController=null,versionController=null,generation=0,versionGeneration=0,locked=false,loading=false,versionLoading=false;
  const notify=()=>{onChange?.();};
  function controls() {
    query.disabled=locked;search.disabled=locked||loading;next.disabled=locked||loading;
    firmwareSelect.disabled=locked||loading||!rows.length;versionSelect.disabled=locked||versionLoading||!versions.length;
    detail.hidden=!selected;if(selected)detail.href=selected.detailsUrl;
  }
  function cancel() {clearTimeout(timer);generation++;versionGeneration++;requestController?.abort();versionController?.abort();loading=false;versionLoading=false;}
  function emptySelection() {selected=null;versions=[];versionSelect.replaceChildren(new Option('请选择固件',''));detail.hidden=true;}
  function setProduct(value) {
    if(product?.id===value?.id)return;
    cancel();product=value;device=null;rows=[];emptySelection();query.value='';page=0;next.hidden=true;
    firmwareSelect.replaceChildren(new Option('搜索当前设备的固件',''));
    identity.textContent=value?`仅搜索 ${value.name} 的固件。`:'';query.placeholder=value?`在 ${value.name.replace(/^M5Stack /,'')} 中搜索`:'搜索固件';message.textContent='';controls();
  }
  function chooseVersion() {
    const project=rows.find(item=>item.firmwareId===firmwareSelect.value),version=versions.find(item=>item.versionId===versionSelect.value);
    selected=project&&version?burnerFirmware(project,version,device?.deviceId):null;controls();notify();
  }
  async function chooseFirmware() {
    versionController?.abort();const token=++versionGeneration,controller=new AbortController();versionController=controller;
    const project=rows.find(item=>item.firmwareId===firmwareSelect.value);emptySelection();versionLoading=!!project;controls();notify();if(!project)return;
    versionSelect.replaceChildren(new Option('正在读取版本…',''));
    try {
      const values=await client.versions(project.firmwareId,device.deviceId,controller.signal);if(token!==versionGeneration)return;
      versions=values;versionSelect.replaceChildren(...values.map(item=>new Option(`${item.versionName}${item.publishTime?' · '+item.publishTime.slice(0,10):''}`,item.versionId)));
      if(values.some(item=>item.versionId===project.currentVersionId))versionSelect.value=project.currentVersionId;
      if(!values.length){versionSelect.append(new Option('没有适用于此设备的公开版本',''));message.textContent='此项目暂无适用于当前设备的公开版本。';}
      chooseVersion();
    }catch(error){if(token!==versionGeneration||controller.signal.aborted)return;versionSelect.replaceChildren(new Option('读取失败，请重选固件重试',''));message.textContent=error.message;}
    finally{if(token===versionGeneration){versionLoading=false;controls();notify();}}
  }
  async function runSearch(more=false) {
    clearTimeout(timer);requestController?.abort();versionController?.abort();versionGeneration++;versionLoading=false;
    const token=++generation,controller=new AbortController();requestController=controller;loading=true;
    if(!more){rows=[];page=0;received=0;total=0;searchText=query.value;emptySelection();firmwareSelect.replaceChildren(new Option('正在搜索…',''));next.hidden=true;notify();}
    controls();message.textContent='正在查询 M5Burner…';
    try {
      if(!device){const devices=await client.devices(controller.signal);if(token!==generation)return;device=matchBurnerDevice(product,devices);}
      if(!device){firmwareSelect.replaceChildren(new Option('没有找到对应型号',''));message.textContent='M5Burner 尚未单列此型号。可改用 HTTPS 地址或本地文件。';return;}
      identity.textContent=`M5Burner · ${device.deviceName}`;
      const result=await client.search(device.deviceId,searchText,page+1,controller.signal);if(token!==generation)return;
      page=result.page;received+=result.received;total=result.total;const previous=firmwareSelect.value;
      rows=[...new Map([...rows,...result.rows].map(item=>[item.firmwareId,item])).values()];
      firmwareSelect.replaceChildren(...rows.map(item=>new Option(`${item.sourceType==='OFFICIAL'?'官方 · ':''}${item.firmwareName}`,item.firmwareId)));
      if(more&&rows.some(item=>item.firmwareId===previous))firmwareSelect.value=previous;
      if(!rows.length)firmwareSelect.append(new Option('没有匹配的固件',''));
      next.hidden=result.received===0||received>=total;message.textContent=`找到 ${total} 个项目，已显示 ${rows.length} 个。`;
      if(!more||!selected)void chooseFirmware();
    }catch(error){if(token!==generation||controller.signal.aborted)return;message.textContent=error.message;if(!rows.length)firmwareSelect.replaceChildren(new Option('暂时无法搜索，请重试',''));}
    finally{if(token===generation){loading=false;controls();notify();}}
  }
  search.addEventListener('click',()=>{void runSearch();});next.addEventListener('click',()=>{void runSearch(true);});
  query.addEventListener('input',()=>{cancel();rows=[];emptySelection();firmwareSelect.replaceChildren(new Option('正在搜索…',''));next.hidden=true;controls();notify();timer=setTimeout(()=>{void runSearch();},350);});
  query.addEventListener('keydown',event=>{if(event.key==='Enter'){event.preventDefault();void runSearch();}});
  firmwareSelect.addEventListener('change',()=>{void chooseFirmware();});versionSelect.addEventListener('change',chooseVersion);
  return {
    setProduct,activate(){if(product&&!loading&&!versionLoading&&!selected){if(rows.length)void chooseFirmware();else void runSearch();}},deactivate(){cancel();controls();},
    current:()=>selected, setDisabled(value){locked=value;controls();},
    pin(hash,bootMode){if(selected){selected.source.sha256=hash;if(bootMode)selected.source.bootMode=bootMode;}},
    restore(entry){const firmware=burnerHistoryFirmware(entry);if(!firmware||entry.productId!==product?.id)return false;cancel();selected=firmware;device=null;rows=[];versions=[];next.hidden=true;
      firmwareSelect.replaceChildren(new Option(firmware.name,firmware.firmwareId));versionSelect.replaceChildren(new Option(firmware.revision,firmware.versionId));identity.textContent=`M5Burner · ${product.name}`;message.textContent='已恢复历史版本，可直接运行，也可重新搜索。';controls();notify();return true;},
  };
}
