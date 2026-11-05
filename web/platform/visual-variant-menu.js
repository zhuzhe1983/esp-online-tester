import {visualVariantsForProduct} from './catalog-selection.js';

// This controller only selects renderer products; it has no runtime mutation API.
export function createVisualVariantController({products,onSelect=()=>{},onChange=()=>{}}) {
  let product=null,status='idle',mode='3d',busy=false,previewId='',disposed=false;
  const variants=()=>visualVariantsForProduct(products,product?.id);
  const disabled=()=>disposed||busy||['loading','running','disposed'].includes(status);
  const snapshot=()=>({product,variants:variants(),selectedId:previewId||product?.id||'',previewOnly:!!previewId,disabled:disabled(),visible:mode==='3d'&&variants().length>0,status});
  function reset(){if(disposed||!previewId)return;previewId='';if(product)onSelect(product,{previewOnly:false});onChange(snapshot());}
  return {
    snapshot,reset,dispose(){disposed=true;previewId='';},
    update(next){
      if(disposed)return;
      const changed=product?.id!==next.product?.id,previousStatus=status;
      product=next.product||null;status=next.status||'idle';mode=next.mode||'3d';busy=!!next.busy;
      // A new scene's base model is rendered by the application, never the old parent.
      if(changed)previewId='';
      else if(['running','loading','disposed'].includes(status)||mode!=='3d'||(status==='configured'&&previousStatus!=='configured'))reset();
      onChange(snapshot());
    },
    select(id){
      if(disabled()||mode!=='3d'||!product)return false;
      if(id===product.id){reset();return true;}
      const variant=variants().find(item=>item.id===id);if(!variant)return false;
      previewId=id;onSelect(variant,{previewOnly:true});onChange(snapshot());return true;
    },
  };
}

export function mountVisualVariantMenu({container,products,onSelect}) {
  if(!container)return null;
  const doc=container.ownerDocument,root=doc.createElement('details');root.id='visual-variant-menu';root.className='shell-settings visual-variant-menu';root.hidden=true;
  root.innerHTML='<summary>衍生模型</summary><div class="shell-settings-body"><label for="visual-variant-select">查看外观</label><select id="visual-variant-select" aria-label="衍生模型"></select><p class="helper" role="status"></p></div>';
  const summary=root.querySelector('summary'),select=root.querySelector('select'),hint=root.querySelector('p');let optionsKey='';
  const controller=createVisualVariantController({products,onSelect,onChange:state=>{
    root.hidden=!state.visible;root.dataset.previewOnly=String(state.previewOnly);
    const key=[state.product?.id,...state.variants.map(item=>item.id)].join('|');
    if(key!==optionsKey){optionsKey=key;select.replaceChildren();if(state.product){const base=doc.createElement('option');base.value=state.product.id;base.textContent=`本体 · ${state.product.name.replace(/^M5Stack /,'')}`;select.append(base);for(const product of state.variants){const option=doc.createElement('option');option.value=product.id;option.textContent=product.name.replace(/^M5Stack /,'');select.append(option);}}}
    select.value=state.selectedId;select.disabled=state.disabled;summary.setAttribute('aria-disabled',String(state.disabled));
    summary.textContent=state.previewOnly?'衍生模型 · 预览':'衍生模型';
    hint.textContent=state.disabled?(['running','loading'].includes(state.status)?'运行或加载期间使用本体模型；暂停后可查看衍生外观。':'正在处理设备操作，请稍候。'):'仅查看外观，可旋转、改色和分开查看。设备配置保持不变，运行时自动回到本体。';
    root.title=state.previewOnly?`仅预览：${state.variants.find(item=>item.id===state.selectedId)?.name}`:state.disabled?'暂停运行后可查看衍生模型':'查看相关产品的衍生外观';
    if(state.disabled||!state.visible)root.open=false;
  }});
  const onToggle=event=>{if(controller.snapshot().disabled)event.preventDefault();};
  const onSelectChange=()=>{if(!controller.select(select.value))select.value=controller.snapshot().selectedId;};
  summary.addEventListener('click',onToggle);select.addEventListener('change',onSelectChange);
  const shell=container.querySelector('#shell-settings');if(shell)shell.before(root);else container.prepend(root);
  return {...controller,dispose(){controller.dispose();summary.removeEventListener('click',onToggle);select.removeEventListener('change',onSelectChange);root.remove();}};
}
