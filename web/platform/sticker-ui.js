import {STICKER_LIMITS} from './sticker-store.js';
export function mountStickerControls({container,view,onError=()=>{},onMessage=()=>{}}){
  const panel=document.createElement('section');panel.className='sticker-panel';panel.setAttribute('aria-label','本地贴纸');
  panel.innerHTML=`<div class="sticker-heading"><strong>我的贴纸</strong><span class="sticker-count"></span></div>
    <label class="sticker-upload button quiet tiny">添加图片<input type="file" multiple accept="image/png,image/jpeg,image/webp" aria-label="添加多张贴纸"></label>
    <p class="helper">最多 ${STICKER_LIMITS.count} 张，仅保存在本机；分享链接不会包含图片。</p>
    <div class="sticker-list" role="group" aria-label="选择贴纸"></div>
    <div class="sticker-editor" hidden><p class="sticker-selected-name" data-i18n-skip></p><div class="sticker-actions"><button type="button" class="button quiet tiny" data-sticker="place">在模型上放置</button><button type="button" class="button quiet tiny" data-sticker="remove">删除</button></div>
      <label>大小 <output data-value="size"></output><input type="range" data-sticker="size" min="2" max="100" step="1" value="22"></label>
      <label>旋转 <output data-value="rotation"></output><input type="range" data-sticker="rotation" min="-180" max="180" step="1" value="0"></label>
      <label>横向微调 <output data-value="x"></output><input type="range" data-sticker="x" min="-100" max="100" step="1" value="0"></label>
      <label>纵向微调 <output data-value="y"></output><input type="range" data-sticker="y" min="-100" max="100" step="1" value="0"></label>
    </div><p class="helper sticker-status" role="status"></p>`;
  container.append(panel);const controller=view.stickers,query=selector=>panel.querySelector(selector),input=query('input[type=file]');let signature='';
  function render(){
    const state=controller.state();panel.dataset.saving=String(state.saving);query('[data-sticker=remove]').disabled=state.busy;input.disabled=state.busy||!state.available;query('.sticker-count').textContent=`${state.items.length} / ${STICKER_LIMITS.count}`;
    const next=JSON.stringify(state.items.map(item=>[item.id,item.thumbnail,item.name]));
    if(next!==signature){signature=next;query('.sticker-list').replaceChildren(...state.items.map(item=>{const button=document.createElement('button');button.type='button';button.className='sticker-thumb';button.dataset.id=item.id;button.setAttribute('data-i18n-skip','');button.title=item.name;button.setAttribute('aria-label',item.name);const image=document.createElement('img');image.src=item.thumbnail;image.alt='';button.append(image);button.addEventListener('click',()=>controller.select(item.id));return button;}));}
    for(const button of query('.sticker-list').children){button.disabled=state.busy;button.setAttribute('aria-pressed',String(button.dataset.id===state.selected));button.dataset.rendered=String(!!state.items.find(item=>item.id===button.dataset.id)?.rendered);}
    const selected=state.items.find(item=>item.id===state.selected);query('.sticker-editor').hidden=!selected;
    if(selected){for(const input of query('.sticker-editor').querySelectorAll('input,button'))input.disabled=state.busy;query('.sticker-selected-name').textContent=selected.name;query('[data-sticker=place]').textContent=state.placing?'取消放置':selected.anchor?'重新定位':'在模型上放置';for(const key of ['size','rotation','x','y']){const value=Math.round(selected[key]*(key==='rotation'?1:100));query(`[data-sticker=${key}]`).value=value;query(`[data-value=${key}]`).textContent=`${value}${key==='rotation'?'°':'%'}`;}}
    query('.sticker-status').textContent=[state.saving?'正在保存…':state.message,!state.persistent?'浏览器存储不可用，本次贴纸只在当前页面保留。':''].filter(Boolean).join(' ');
  }
  input.addEventListener('change',async()=>{try{if(await controller.addFiles(input.files))onMessage('拖动可转动模型；轻点外壳放置贴纸。');}catch(error){onError(error);}finally{input.value='';}});
  query('[data-sticker=place]').addEventListener('click',()=>{controller.place(!controller.placing);if(controller.placing){const details=document.getElementById('shell-settings');if(details)details.open=false;onMessage('拖动可转动模型；轻点外壳放置贴纸。');}});
  query('[data-sticker=remove]').addEventListener('click',()=>controller.remove().catch(onError));
  for(const key of ['size','rotation','x','y'])for(const type of ['input','change'])query(`[data-sticker=${key}]`).addEventListener(type,event=>{try{controller.update({[key]:Number(event.target.value)/(key==='rotation'?1:100)},{save:type==='change'});}catch(error){onError(error);}});
  const mount=document.getElementById('scene-mount');mount.addEventListener('stickerschange',render);const cancel=event=>{if(event.key==='Escape'&&controller.placing)controller.place(false);};window.addEventListener('keydown',cancel);render();return ()=>{mount.removeEventListener('stickerschange',render);window.removeEventListener('keydown',cancel);};
}
