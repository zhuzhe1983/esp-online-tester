import {configurationDefaults} from './flash-configuration.js';
import {browserFlashProfiles} from './flash-profiles.js';
const node=(tag,text)=>{const el=document.createElement(tag);if(text!==undefined)el.textContent=text;return el;};
const button=text=>{const el=node('button',text);el.type='button';el.className='button quiet tiny';return el;};
export function createFlashConfigurationUI({apply,onChange,onError,profiles=browserFlashProfiles()}) {
  const root=node('section');root.className='flash-configuration';root.setAttribute('aria-label','随固件写入配置');
  root.append(node('h3','随固件写入配置（可选）'));
  const label=node('label','使用配置'),select=node('select');select.setAttribute('aria-label','使用配置');label.append(select);
  const toolbar=node('div'),add=button('新建配置'),edit=button('编辑配置'),remove=button('删除配置');toolbar.className='flash-config-toolbar';toolbar.append(label,add,edit,remove);
  const hint=node('p','只适用于读取相应格式的 UIFlow 固件。关闭此选项时，不额外写入配置。');hint.className='helper';
  const summary=node('p');summary.className='helper flash-config-summary';
  const form=node('form');form.addEventListener('submit',event=>event.preventDefault());form.className='flash-config-editor';form.hidden=true;
  const fields={};
  for(const [key,title,type] of [['name','配置名称','text'],['type','固件格式','select'],['server','数据服务器','text'],['ssid','Wi-Fi 名称（SSID）','text'],['password','Wi-Fi 密码','password']]) {
    const label=node('label',title),input=node(type==='select'?'select':'input');input.setAttribute('aria-label',title);
    if(type==='select')for(const value of ['uiflow2','uiflow1']){const option=node('option',value==='uiflow2'?'UIFlow2':'UIFlow1');option.value=value;input.append(option);}
    else {input.type=type;input.autocomplete=key==='password'?'new-password':'off';input.spellcheck=false;}
    if(key==='server')input.placeholder='uiflow2.m5stack.com';label.append(input);form.append(label);fields[key]=input;
  }
  const reveal=button('显示密码');reveal.addEventListener('click',()=>{fields.password.type=fields.password.type==='password'?'text':'password';reveal.textContent=fields.password.type==='password'?'显示密码':'隐藏密码';});
  const rememberLabel=node('label'),remember=node('input');remember.type='checkbox';rememberLabel.className='flash-config-remember';rememberLabel.append(remember,document.createTextNode('保存到此浏览器（包含 Wi-Fi 密码）'));
  const note=node('p','默认仅在当前页面保留。UIFlow2 将重建网络配置，启用 Wi-Fi / DHCP 和启动菜单；其他已保存的 NVS 设置会被替换。UIFlow1 密码最多 48 字节。');note.className='helper';
  const buttons=node('div'),save=button('应用此配置'),cancel=button('取消编辑');buttons.className='flash-config-buttons';save.className='button primary tiny';buttons.append(reveal,cancel,save);form.append(rememberLabel,note,buttons);root.append(toolbar,hint,summary,form);
  let current=null,editing=null,enabled=false,canEdit=false,formats={};
  const safe=fn=>{try{fn();}catch(error){onError(error);}};
  function choices(){const values=profiles.list();select.replaceChildren();const none=node('option','不写入配置');none.value='';select.append(none);for(const value of values){const option=node('option',`${value.name} · ${value.type==='uiflow1'?'UIFlow1':'UIFlow2'}${value.persistent?'':' · 本页'}`);option.value=value.id;select.append(option);}select.value=current?.id||'';}
  function refresh(){
    for(const el of [select,add,edit,remove,...Object.values(fields),remember,reveal,save,cancel])el.disabled=!canEdit;
    select.disabled=add.disabled=!canEdit||enabled;edit.disabled=remove.disabled=!canEdit||enabled||!current;
    edit.hidden=remove.hidden=!current;summary.textContent=current?`已应用：${current.name} · ${current.server}。每台设备开始写入后固定配置；连续模式切换配置只影响后续添加的设备。`:'未启用附加配置。';
    for(const option of fields.type.options)option.disabled=!formats[option.value];
  }
  function begin(value){editing=value?.id||null;enabled=true;form.hidden=false;for(const key of Object.keys(fields))fields[key].value=value?.[key]??(key==='type'?(formats.uiflow2?'uiflow2':'uiflow1'):key==='name'?'新配置':'');if(!value)fields.server.value=configurationDefaults[fields.type.value];remember.checked=!!value?.persistent;fields.password.type='password';reveal.textContent='显示密码';refresh();onChange();fields.name.focus();}
  function end(){enabled=false;editing=null;form.hidden=true;fields.password.value='';refresh();onChange();}
  fields.type.addEventListener('change',()=>{if(Object.values(configurationDefaults).includes(fields.server.value)||!fields.server.value)fields.server.value=configurationDefaults[fields.type.value];});
  add.addEventListener('click',()=>begin(null));edit.addEventListener('click',()=>begin(current));cancel.addEventListener('click',end);
  select.addEventListener('change',()=>safe(()=>{const value=profiles.list().find(p=>p.id===select.value)||null;try{apply(value);current=value;refresh();onChange();}catch(error){select.value=current?.id||'';throw error;}}));
  save.addEventListener('click',()=>safe(()=>{
    const value=Object.fromEntries(Object.entries(fields).map(([key,input])=>[key,input.value]));
    // Validate the image layout before storing credentials or changing the choice.
    apply(value);try{current=profiles.put(value,{...(editing?{id:editing}:{}),persistent:remember.checked});}catch(error){apply(current);throw error;}choices();end();
  }));
  remove.addEventListener('click',()=>safe(()=>{profiles.remove(current.id);apply(null);current=null;choices();refresh();onChange();}));
  choices();refresh();
  return {root,editing:()=>enabled,update(state){formats=state.receipt?.configurationFormats||{};canEdit=!!state.receipt&&!state.choosing&&!state.counts?.active&&!['preparing','stopping'].includes(state.status)&&(!state.running||state.mode==='sequential');if(!Object.values(formats).some(Boolean))canEdit=false;hint.textContent=!state.receipt?'固件准备完成后，将检查是否支持随固件写入配置。':Object.values(formats).some(Boolean)?'只适用于读取相应格式的 UIFlow 固件。关闭此选项时，不额外写入配置。':'当前镜像不符合已支持的 UIFlow 配置布局，可直接烧录原始固件。';refresh();},dispose(){fields.password.value='';root.remove();}};
}
