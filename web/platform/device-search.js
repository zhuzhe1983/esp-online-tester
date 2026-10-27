import {searchProducts,searchAccessories,accessoryBrand,accessoryInterface} from './catalog-selection.js';

export function mountDeviceSearch({button, products, onSelect, onError}) {
  return mountCatalogSearch({button,getItems:()=>products,onSelect,onError,kind:'device',label:'设备',search:searchProducts,describe:item=>[item.manufacturer,item.sku,item.chip?.toUpperCase()].filter(Boolean).join(' · ')});
}
export function mountAccessorySearch({button, getItems, onSelect, onError}) {
  return mountCatalogSearch({button,getItems,onSelect,onError,kind:'accessory',label:'配件',search:searchAccessories,describe:item=>[accessoryBrand(item),item.sku,accessoryInterface(item)].filter(Boolean).join(' · ')});
}
function mountCatalogSearch({button, getItems, onSelect, onError, kind, label, search, describe}) {
  if (!button) return;
  const node = (tag, text) => { const item = document.createElement(tag); if (text) item.textContent=text; return item; };
  const dialog = node('dialog'); dialog.id=`${kind}-search-dialog`; dialog.className='device-search-dialog'; dialog.setAttribute('aria-labelledby',`${kind}-search-title`);
  const heading=node('div');heading.className='dialog-heading';
  const title=node('h2',`搜索所有${label}`);title.id=`${kind}-search-title`;
  const close=node('button','×');close.type='button';close.className='icon-button';close.setAttribute('aria-label',`关闭${label}搜索`);heading.append(title,close);
  const input=node('input');input.type='search';input.placeholder=kind==='accessory'?'品牌、型号、SKU 或接口':'品牌、型号或 SKU';input.setAttribute('aria-label',`跨品牌搜索${label}`);
  const count=node('p');count.className='helper';count.setAttribute('role','status');
  const results=node('div');results.className='device-search-results';
  dialog.append(heading,input,count,results);document.body.append(dialog);
  function render() {
    const matches=search(getItems(),input.value);count.textContent=`${matches.length} 款${label}`;results.replaceChildren();
    for(const product of matches){
      const row=node('button');row.type='button';row.className='device-search-result';row.dataset.productId=product.id;
      row.append(node('strong',product.name),node('small',describe(product)));
      row.addEventListener('click',()=>{dialog.close();Promise.resolve(onSelect(product.id)).catch(onError);});results.append(row);
    }
    if(!matches.length)results.append(node('p','没有匹配的型号，试试其他名称或 SKU。'));
  }
  const open=()=>{input.value='';render();dialog.showModal();input.focus();};button.addEventListener('click',open);
  close.addEventListener('click',()=>dialog.close());input.addEventListener('input',render);
  input.addEventListener('keydown',event=>{if(event.key==='ArrowDown'){event.preventDefault();results.querySelector('button')?.focus();}else if(event.key==='Enter'&&results.querySelectorAll('button').length===1){event.preventDefault();results.querySelector('button').click();}});
  results.addEventListener('keydown',event=>{const rows=[...results.querySelectorAll('button')],at=rows.indexOf(document.activeElement);if(['ArrowDown','ArrowUp'].includes(event.key)){event.preventDefault();rows[Math.max(0,Math.min(rows.length-1,at+(event.key==='ArrowDown'?1:-1)))]?.focus();}});
  return ()=>{button.removeEventListener('click',open);dialog.remove();};
}
