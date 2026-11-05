import {currentLocale} from './i18n.js';
import {createFlashBatch} from './flash-batch.js';
import {prepareBurnerSource} from './m5burner-download-ui.js';
import {burnerProjectUrl} from './firmware-download-access.js';
import {createFlashConfigurationUI} from './flash-configuration-ui.js';
import {createM5BurnerClient,matchBurnerDevice,parseBurnerDownload,burnerDownloadUrl} from './m5burner.js';

const node=(tag,className,text)=>{const value=document.createElement(tag);if(className)value.className=className;if(text!==undefined)value.textContent=text;return value;};
const button=(label,className='button')=>{const value=node('button',className,label);value.type='button';return value;};
const activeItem=status=>['connecting','validating','writing','resetting','closing','cancelling','preparing','choosing'].includes(status);
const statusLabels={queued:'等待烧录',ready:'等待烧录',connecting:'连接中',validating:'检查设备',writing:'正在写入',resetting:'正在复位',closing:'关闭连接',cancelling:'正在停止',done:'写入已校验',error:'烧录失败',cancelled:'已取消',disconnected:'设备已拔出'};
const modeHints={single:'写入一台设备，完成后保留结果。',parallel:'先逐个添加设备，再一起开始。所有已选设备同时写入，平台不限制数量；实际速度取决于浏览器、USB 带宽和供电。',sequential:'保留本批固件，写完一台后换插下一台。点击“选择下一台”授权并继续，无需重新选择固件。'};

// A batch owns its source snapshot and its chosen ports. Hiding the dialog does
// not dispose that session; the workbench entry remains its visible status.
export function openFlashStation({product,source,scene,catalog,onChange,onClose,onNewBatch,createBatch=createFlashBatch,prepareSource=prepareBurnerSource,burnerClient=createM5BurnerClient()}) {
  const dialog=node('dialog','firmware-run-confirm flash-station');dialog.setAttribute('aria-labelledby','flash-station-title');
  const heading=node('div','flash-station-heading'),headingText=node('div');headingText.append(node('span','eyebrow','USB / 实体设备'));
  const title=node('h2','','烧录到设备');title.id='flash-station-title';headingText.append(title);
  const close=button('关闭','button quiet');close.setAttribute('aria-label','关闭烧录窗口');heading.append(headingText,close);
  const identity=node('div','flash-station-identity');identity.append(node('strong','',product.name));
  const sourceName=source.file?.name||(source.files?`本地固件 · ${source.files.length} 个文件`:source.url||source.manifestUrl||'所选固件');
  const fingerprint=node('p','helper flash-fingerprint');fingerprint.hidden=true;
  const origin=node('p','flash-source',sourceName),fixed=node('p','helper');
  const receiptDetails=node('details','flash-receipt'),receiptSummary=node('summary','','固件校验与写入范围'),receiptBody=node('div','flash-receipt-body');receiptDetails.append(receiptSummary,receiptBody);receiptDetails.hidden=true;identity.append(origin,fingerprint,fixed,receiptDetails);
  const modeGroup=node('fieldset','flash-mode-group'),legend=node('legend','','烧录方式');modeGroup.append(legend);
  const radios=new Map();
  for(const [value,label] of [['single','单台烧录'],['parallel','多台并行'],['sequential','逐台连续']]) {
    const option=node('label','flash-mode-option'),input=node('input');input.type='radio';input.name='flash-mode';input.value=value;input.checked=value==='single';
    option.append(input,node('span','',label));modeGroup.append(option);radios.set(value,input);
    input.addEventListener('change',()=>{mode=value;render();});
  }
  const modeHint=node('p','helper flash-mode-hint');
  const status=node('p','flash-station-status');status.setAttribute('role','status');
  const errorBox=node('div','flash-station-error');errorBox.setAttribute('role','alert');errorBox.hidden=true;
  const errorText=node('p'),errorLink=node('a','text-link','打开官方页面验证并下载 ↗');errorLink.target='_blank';errorLink.rel='noopener noreferrer';errorLink.hidden=true;errorBox.append(errorText,errorLink);
  const recovery=node('div','flash-download-recovery'),retryDownload=button('重试准备固件'),refreshVersions=button('刷新版本列表'),versionSelect=node('select'),useVersion=button('使用所选版本并重新准备');
  versionSelect.setAttribute('aria-label','可用固件版本');versionSelect.hidden=useVersion.hidden=true;
  const versionHint=node('p','helper');recovery.append(retryDownload,refreshVersions,versionHint,versionSelect,useVersion);recovery.hidden=true;
  const queueHeading=node('div','flash-queue-heading'),queueTitle=node('h3','','设备队列'),add=button('添加设备');queueHeading.append(queueTitle,add);
  const browserHint=node('p','helper','浏览器一次只授权一台 USB 设备。多台设备请分别添加；这里只会写入本批次中明确选择的设备。');
  const list=node('div','flash-device-list');list.setAttribute('aria-label','烧录设备队列');
  const empty=node('p','flash-empty','连接 USB 设备，然后点击“添加设备”。'),counts=node('p','flash-counts');
  const acknowledgement=node('label','flash-acknowledgement'),checked=node('input');checked.type='checkbox';
  acknowledgement.append(checked,document.createTextNode('我已备份本批设备，确认型号和固件正确，并接受写入会覆盖设备数据的风险。'));
  const risk=node('p','helper flash-risk','固件来自第三方发布者或由用户自行制作，请自行确认兼容性并承担使用后果。写入校验通过不等于应用已正常启动。');
  const actions=node('div','flash-station-actions'),newBatch=button('新建批次','button quiet'),stop=button('停止本批'),start=button('开始烧录','button primary');actions.append(newBatch,stop,start);
  dialog.append(heading,identity,modeGroup,modeHint,status,errorBox,recovery,queueHeading,browserHint,list,counts,acknowledgement,risk,actions);
  let state={status:'idle',items:[],counts:{},running:false,choosing:false},mode='single',disposed=false,started=false,preparing=false,prepareError=null,closeFocus=null;
  const rows=new Map();let receiptKey='',aborter=null,attempt=0,versionAbort=null,versionLoading=false,versions=[],batchPreparation=null;
  const batch=createBatch({onEvent(event){if(event.type==='state'&&!disposed){state=event.state;render();}}});
  const configurationUI=createFlashConfigurationUI({apply:profile=>batch.setConfiguration(profile),onChange:render,onError:showError});modeGroup.before(configurationUI.root);
  const busy=()=>Boolean(preparing||versionLoading||state.running||state.choosing||['preparing','stopping'].includes(state.status));
  function showError(error){
    if(disposed||['FLASH_CANCELLED','FETCH_ABORTED'].includes(error?.code)||error?.name==='NotFoundError')return;
    errorBox.hidden=false;errorText.textContent=error?.message||String(error);
    const url=burnerProjectUrl(error?.details?.url||source.url);errorLink.hidden=!(url&&['FIRMWARE_AUTH_REQUIRED','FIRMWARE_DOWNLOAD_RESPONSE','FIRMWARE_VERSION_NOT_FOUND','CAPTCHA_CANCELLED'].includes(error?.code));if(!errorLink.hidden){errorLink.href=url;errorLink.textContent=error?.code==='FIRMWARE_VERSION_NOT_FOUND'?'查看官方固件项目 ↗':'打开官方页面验证并下载 ↗';}
  }
  function action(callback){errorBox.hidden=true;try{Promise.resolve(callback()).catch(showError);}catch(error){showError(error);}}
  function updateReceipt(){
    const receipt=state.receipt;if(!receipt)return;
    const key=JSON.stringify(receipt);if(key===receiptKey)return;receiptKey=key;receiptDetails.hidden=false;receiptBody.replaceChildren();
    const artifacts=receipt.artifacts||[];fingerprint.hidden=false;fingerprint.textContent=artifacts.length===1&&artifacts[0].sha256?`已固定 · SHA-256 ${artifacts[0].sha256.slice(0,16)}…`:`已固定 · ${artifacts.length} 个镜像已校验`;
    for(const artifact of receipt.artifacts||[]) {
      const item=node('div','flash-artifact');const offset=artifact.address??artifact.offset??0;
      item.append(node('p','',`0x${Number(offset).toString(16)} · ${(artifact.bytes??artifact.size??artifact.byteLength??0).toLocaleString(currentLocale())} 字节`));
      if(artifact.sha256)item.append(node('code','',`SHA-256 ${artifact.sha256}`));receiptBody.append(item);
    }
    if(!receiptBody.childNodes.length)receiptBody.append(node('p','helper','固件内容与芯片、分区布局已校验。'));
  }
  function deviceRow(item){
    let row=rows.get(item.id);if(row)return row;
    const root=node('article','flash-device'),header=node('div','flash-device-heading'),label=node('strong'),badge=node('span','flash-device-badge');header.append(label,badge);
    const configuration=node('p','helper flash-device-configuration'),message=node('p','flash-device-message'),progress=node('progress','flash-progress');progress.max=1;progress.value=0;
    const controls=node('div','flash-device-actions'),cancel=button('取消','button quiet tiny'),retry=button('重试此设备','button quiet tiny'),replace=button('更换设备','button quiet tiny');
    cancel.addEventListener('click',()=>action(()=>batch.cancel(item.id)));retry.addEventListener('click',()=>action(()=>{const result=batch.retry(item.id);started=true;render();return result;}));
    replace.addEventListener('click',()=>action(()=>batch.replaceDevice(item.id)));controls.append(cancel,retry,replace);
    const details=node('details','flash-device-log'),summary=node('summary','','查看日志'),log=node('pre');details.append(summary,log);root.append(header,configuration,message,progress,controls,details);
    row={root,label,badge,configuration,message,progress,cancel,retry,replace,details,log};rows.set(item.id,row);return row;
  }
  function render(){
    if(disposed)return;
    const items=state.items||[],isBusy=busy();
    configurationUI.update(state);
    fixed.textContent=state.receipt?'本批原始固件已固定；SHA-256 对应原始镜像。启用附加配置后，仅修改相应配置区域。切换工作台不会改变本批次。':'当前仅固定了所选来源；下载并校验完成后才会固定固件内容。';
    recovery.hidden=!!state.receipt||(!prepareError&&state.status!=='stopped');
    retryDownload.disabled=isBusy;refreshVersions.disabled=isBusy;useVersion.disabled=isBusy||!versions.some(value=>value.versionId===versionSelect.value);versionSelect.disabled=isBusy;
    refreshVersions.hidden=prepareError?.code!=='FIRMWARE_VERSION_NOT_FOUND'||!parseBurnerDownload(source.url);
    refreshVersions.textContent=versionLoading?'正在读取版本…':'刷新版本列表';
    modeHint.textContent=mode==='single'&&!started&&items.some(item=>item.status==='cancelled')?'已取消排队。勾选确认后可重试此设备，也可点击“更换设备”重新选择。':modeHints[mode];
    for(const input of radios.values())input.disabled=started||isBusy||items.length>1&&input.value==='single';
    add.textContent=mode==='sequential'&&started?'选择下一台':'添加设备';
    add.disabled=configurationUI.editing()||!state.receipt||preparing||state.status==='preparing'||state.status==='stopping'||state.choosing||Boolean(prepareError)||mode==='single'&&items.length>=1||started&&mode!=='sequential'||started&&!state.running;
    const labels={idle:'正在准备固件…',preparing:'正在下载并校验固件…',ready:started?'设备已更换，点击“烧录此设备”继续。':'固件已准备好，选择设备并勾选确认后即可开始。',running:mode==='sequential'?'连续烧录已开启。完成后换插设备，再点击“选择下一台”。':'正在烧录，请保持 USB 连接。',stopping:'正在停止本批次，等待各设备关闭连接…',stopped:'本批已停止，可查看结果或新建批次。',done:'本批次已完成。',error:'准备固件失败，可新建批次重试。'};
    status.textContent=preparing?(state.status==='stopped'?'正在取消准备，等待当前校验结束…':'正在下载并校验固件…'):prepareError?'固件未准备好，尚未连接或写入设备。':state.choosing?'请在浏览器中选择一台设备。':labels[state.status]||state.status;
    close.textContent=isBusy?'收起':'关闭';close.setAttribute('aria-label',isBusy?'收起烧录窗口':'关闭烧录窗口');
    const nextRows=items.map(item=>{
      const row=deviceRow(item),isActive=activeItem(item.status),queued=['queued','ready'].includes(item.status);
      row.root.dataset.status=item.status;row.root.dataset.deviceId=item.id;row.label.textContent=item.label||item.id;
      row.badge.textContent=statusLabels[item.status]||item.status;
      row.configuration.textContent=item.configuration?`配置：${item.configuration.name} · ${item.configuration.type==='uiflow1'?'UIFlow1':'UIFlow2'}`:'配置：不写入附加配置';
      row.message.textContent=item.message||(!item.connected?'设备未连接':'已选择设备');
      row.progress.setAttribute('aria-label',`${item.label||item.id} 写入进度`);row.progress.value=Math.max(0,Math.min(1,item.progress||0));row.progress.hidden=!isActive&&item.status!=='done';
      row.cancel.hidden=!isActive&&!queued;row.cancel.disabled=item.status==='cancelling'||state.choosing;row.cancel.textContent=queued?'取消排队':'取消';
      row.retry.textContent=item.status==='ready'?'烧录此设备':'重试此设备';
      row.retry.hidden=!['error','cancelled'].includes(item.status)&&!(started&&item.status==='ready');row.retry.disabled=configurationUI.editing()||state.choosing||state.status==='stopping'||isBusy&&!state.running||!checked.checked||item.connected===false;
      row.replace.hidden=!['queued','ready','error','cancelled'].includes(item.status);row.replace.disabled=configurationUI.editing()||state.choosing||state.status==='stopping'||preparing;
      const logs=Array.isArray(item.logs)?item.logs.map(entry=>typeof entry==='string'?entry:entry.message||'').join('\n'):String(item.logs||'');
      if(row.log.textContent!==logs)row.log.textContent=logs;row.details.hidden=!logs;return row.root;
    });
    if(!items.length)nextRows.push(empty);
    if(nextRows.length!==list.children.length||nextRows.some((row,index)=>row!==list.children[index]))list.replaceChildren(...nextRows);
    const total=state.counts||{};counts.textContent=`已选择 ${total.total??items.length} 台 · 成功 ${total.done||0} · 失败 ${total.error||0} · 等待 ${total.queued||0}${total.cancelled?` · 已取消 ${total.cancelled}`:''}`;
    acknowledgement.hidden=started;checked.disabled=started;start.hidden=started;start.disabled=!state.receipt||configurationUI.editing()||!checked.checked||isBusy||Boolean(prepareError)||(mode!=='sequential'&&!items.some(item=>['queued','ready'].includes(item.status)&&item.connected!==false));
    start.textContent=mode==='sequential'?'开启连续烧录':mode==='parallel'?`开始烧录 ${items.filter(item=>['queued','ready'].includes(item.status)).length} 台`:'开始烧录';
    stop.hidden=!preparing&&!versionLoading&&!state.running&&state.status!=='stopping';stop.disabled=state.status==='stopping'||preparing&&state.status==='stopped';stop.textContent=preparing||versionLoading?'取消准备':'停止本批';
    newBatch.disabled=isBusy;updateReceipt();
    onChange?.({...state,busy:isBusy,mode,started,visible:dialog.open});
  }
  add.addEventListener('click',()=>action(()=>batch.addDevice()));
  checked.addEventListener('change',render);
  start.addEventListener('click',()=>{
    if(start.disabled)return;
    action(()=>{const result=batch.start({mode:mode==='sequential'?'sequential':'parallel',...(mode==='single'?{concurrency:1}:{})});started=true;render();return result;});
  });
  stop.addEventListener('click',()=>action(async()=>{if(preparing||versionLoading){++attempt;aborter?.abort();versionAbort?.abort();}const pending=batchPreparation;await batch.stop();await pending?.catch(()=>{});preparing=false;versionLoading=false;render();}));
  retryDownload.addEventListener('click',()=>{if(!busy())void prepare();});
  refreshVersions.addEventListener('click',async()=>{
    const ids=parseBurnerDownload(source.url);if(busy()||!ids)return;
    const controller=new AbortController();versionAbort=controller;versionLoading=true;versionHint.textContent='正在读取此项目适用于当前设备的公开版本…';render();
    try {
      const device=matchBurnerDevice(product,await burnerClient.devices(controller.signal));
      if(disposed||controller.signal.aborted)return;
      if(!device)throw Error('无法确认官方设备对应关系，请在官方项目页面检查版本。');
      const values=await burnerClient.versions(ids.firmwareId,device.deviceId,controller.signal);
      if(disposed||controller.signal.aborted)return;
      versions=values.filter(value=>value.versionId!==ids.versionId);
      versionSelect.replaceChildren(...versions.map(value=>new Option(`${value.versionName} · ${value.publishTime?.slice(0,10)||'日期未知'} · ID ${value.versionId}`,value.versionId)));
      versionSelect.value='';versionSelect.prepend(new Option('请选择版本','',true,true));versionSelect.hidden=useVersion.hidden=!versions.length;
      versionHint.textContent=versions.length?'选择后会重新下载和校验。版本名称相同也可能是不同的固件内容；整批烧录仍固定为同一份文件。':'此项目暂无其他适用于当前设备的公开版本。';
    }catch(error){if(!disposed&&!controller.signal.aborted)versionHint.textContent=error.message;}
    finally{if(versionAbort===controller){versionLoading=false;render();}}
  });
  versionSelect.addEventListener('change',render);
  useVersion.addEventListener('click',()=>{
    const ids=parseBurnerDownload(source.url),version=versions.find(value=>value.versionId===versionSelect.value);
    if(busy()||!ids||!version)return;
    // Explicit choice starts a new preparation, never retarget a pinned batch.
    source={url:burnerDownloadUrl(ids.firmwareId,version.versionId),...(source.bootMode?{bootMode:source.bootMode}:{})};
    origin.textContent=source.url;versions=[];versionSelect.hidden=useVersion.hidden=true;versionHint.textContent='';void prepare();
  });
  newBatch.addEventListener('click',()=>{if(!busy())onNewBatch?.();});
  close.addEventListener('click',()=>dialog.close());
  dialog.addEventListener('close',()=>{if(disposed)return;onClose?.();onChange?.({...state,busy:busy(),mode,started,visible:false});closeFocus?.focus?.();});
  const beforeUnload=event=>{if(busy()){event.preventDefault();event.returnValue='';}};window.addEventListener('beforeunload',beforeUnload);
  document.body.append(dialog);
  function show(){if(disposed||dialog.open)return;closeFocus=document.activeElement;dialog.showModal();render();close.focus();}
  show();
  async function prepare(){
    const controller=new AbortController(),revision=++attempt;aborter=controller;preparing=true;prepareError=null;errorBox.hidden=true;state={...state,status:'preparing'};render();
    let pending;
    try{const prepared=await prepareSource(source,{signal:controller.signal});if(disposed||revision!==attempt||controller.signal.aborted)return;pending=batch.prepare({scene,catalog,source:prepared});batchPreparation=pending;await pending;}
    catch(error){if(disposed||revision!==attempt)return;prepareError=error;state={...state,status:controller.signal.aborted?'stopped':'error'};showError(error);}
    finally{if(batchPreparation===pending)batchPreparation=null;if(revision===attempt){preparing=false;render();}}
  }
  void prepare();
  return {
    show,
    observe:()=>({...state,busy:busy(),mode,started,visible:dialog.open}),
    async dispose(){if(disposed)return;disposed=true;++attempt;aborter?.abort();versionAbort?.abort();configurationUI.dispose();window.removeEventListener('beforeunload',beforeUnload);if(dialog.open)dialog.close();dialog.remove();await batch.dispose();},
  };
}
