// Shared keyboard and dismissal behavior for the two header menus.
export function mountHeaderMenus(doc=document) {
  const menus=[...doc.querySelectorAll('.header-dropdown')].map(root=>({root,button:root.querySelector('[data-menu-toggle]'),panel:root.querySelector('[data-menu-panel]')}));
  const close=(menu,focus=false)=>{menu.panel.hidden=true;menu.button.setAttribute('aria-expanded','false');if(focus)menu.button.focus();};
  const items=menu=>[...menu.panel.querySelectorAll('[role="menuitem"]')].filter(item=>!item.disabled);
  const open=menu=>{for(const other of menus)close(other);menu.panel.hidden=false;menu.button.setAttribute('aria-expanded','true');};
  for(const menu of menus){
    menu.button.addEventListener('click',()=>{if(menu.panel.hidden)open(menu);else close(menu);});
    menu.root.addEventListener('keydown',event=>{
      if(event.key==='Escape'&&!menu.panel.hidden){event.preventDefault();event.stopPropagation();close(menu,true);return;}
      if(!['ArrowDown','ArrowUp','Home','End'].includes(event.key))return;
      event.preventDefault();event.stopPropagation();const wasClosed=menu.panel.hidden;if(wasClosed)open(menu);
      const choices=items(menu),index=choices.indexOf(doc.activeElement);
      const next=event.key==='Home'?0:event.key==='End'?choices.length-1:event.key==='ArrowDown'?(index+1)%choices.length:index<=0?choices.length-1:index-1;
      choices[next]?.focus();
    });
    menu.panel.addEventListener('click',event=>{if(event.target.closest('[role="menuitem"]'))close(menu,true);});
  }
  doc.addEventListener('pointerdown',event=>{for(const menu of menus)if(!menu.root.contains(event.target))close(menu);});
  doc.addEventListener('focusin',event=>{for(const menu of menus)if(!menu.root.contains(event.target))close(menu);});
  doc.getElementById('import-scene-trigger')?.addEventListener('click',()=>doc.getElementById('import-scene')?.click());
}
