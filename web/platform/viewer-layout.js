// Native fullscreen where available; an in-page viewport on browsers/embeds
// that cannot grant it. Neither path navigates away from running firmware.
export function mountViewerLayout({workspace,button,onChange=()=>{}}) {
  const doc=workspace.ownerDocument,win=doc.defaultView,stage=workspace.querySelector('#device-stage');
  const area=doc.createElement('div');area.className='viewer-area';stage.before(area);area.append(stage);
  const details=workspace.querySelector('#shell-settings'),body=details?.querySelector('.shell-settings-body');
  let editor,observer,disposed=false,active=false,native=false,pending=false,scroll=[0,0],previousOverflow='';
  const chrome=doc.createElement('button');chrome.type='button';chrome.className='viewer-chrome-toggle button';chrome.textContent='收起工具';chrome.hidden=true;chrome.setAttribute('aria-pressed','false');workspace.append(chrome);
  const notify=()=>{if(!disposed)win.requestAnimationFrame(onChange);};
  const syncEditor=()=>{
    if(!editor)return;
    editor.hidden=!details.open||details.hidden;area.classList.toggle('shell-editing',!editor.hidden);
    details.querySelector('summary')?.setAttribute('aria-expanded',String(!editor.hidden));notify();
  };
  if(body){
    editor=doc.createElement('aside');editor.id='shell-editor';editor.className='shell-editor';editor.setAttribute('aria-label','外壳与贴纸编辑');editor.hidden=true;
    const header=doc.createElement('div');header.className='shell-editor-heading';
    const title=doc.createElement('strong');title.textContent='外壳与贴纸';
    const close=doc.createElement('button');close.type='button';close.className='icon-button';close.textContent='×';close.setAttribute('aria-label','收起外壳编辑');
    close.addEventListener('click',()=>{details.open=false;details.querySelector('summary')?.focus();});
    header.append(title,close);editor.append(header,body);area.append(editor);
    details.querySelector('summary')?.setAttribute('aria-controls',editor.id);
    details.addEventListener('toggle',syncEditor);observer=new MutationObserver(syncEditor);observer.observe(details,{attributes:true,attributeFilter:['hidden']});syncEditor();
  }
  function viewport(){
    if(active){workspace.style.setProperty('--viewer-height',`${win.visualViewport?.height||win.innerHeight}px`);workspace.style.setProperty('--viewer-width',`${win.visualViewport?.width||win.innerWidth}px`);}
    notify();
  }
  function setActive(value){
    if(active===value)return;active=value;
    if(active){scroll=[win.scrollX,win.scrollY];previousOverflow=doc.documentElement.style.overflow;doc.documentElement.style.overflow='hidden';if(details)details.open=false;}
    else{doc.documentElement.style.overflow=previousOverflow;workspace.classList.remove('viewer-chrome-hidden');chrome.setAttribute('aria-pressed','false');chrome.textContent='收起工具';win.scrollTo(...scroll);button?.focus();}
    workspace.classList.toggle('viewer-immersive',active);chrome.hidden=!active;
    button?.setAttribute('aria-pressed',String(active));button?.setAttribute('aria-label',active?'退出全屏':'全屏显示模拟器');if(button)button.title=active?'退出全屏':'全屏';
    if(!active){delete workspace.dataset.fullscreenMode;workspace.style.removeProperty('--viewer-height');workspace.style.removeProperty('--viewer-width');}
    viewport();
  }
  const fullscreenElement=()=>doc.fullscreenElement||doc.webkitFullscreenElement;
  async function exit(){
    if(fullscreenElement()===workspace){const leave=doc.exitFullscreen||doc.webkitExitFullscreen;await leave?.call(doc);}
    native=false;setActive(false);
  }
  async function toggle(){
    if(pending||disposed)return;pending=true;
    try{
      if(active){await exit();return;}
      // Call synchronously from the gesture before awaiting anything else.
      const request=workspace.requestFullscreen||workspace.webkitRequestFullscreen;
      setActive(true);workspace.dataset.fullscreenMode='viewport';
      if(request){try{await request.call(workspace,{navigationUI:'hide'});native=fullscreenElement()===workspace;if(native)workspace.dataset.fullscreenMode='native';}catch{native=false;}}
      viewport();
    }finally{pending=false;}
  }
  const changed=()=>{if(fullscreenElement()===workspace){native=true;setActive(true);workspace.dataset.fullscreenMode='native';viewport();}else if(native){native=false;setActive(false);}};
  const key=event=>{if(event.key==='Escape'&&active&&!native){event.preventDefault();void exit();}};
  const toggleChrome=()=>{const hidden=workspace.classList.toggle('viewer-chrome-hidden');if(hidden&&details)details.open=false;chrome.textContent=hidden?'显示工具':'收起工具';chrome.setAttribute('aria-pressed',String(hidden));notify();};
  chrome.addEventListener('click',toggleChrome);doc.addEventListener('fullscreenchange',changed);doc.addEventListener('webkitfullscreenchange',changed);doc.addEventListener('keydown',key);win.addEventListener('resize',viewport);win.visualViewport?.addEventListener('resize',viewport);
  return {toggle,get active(){return active;},dispose(){disposed=true;observer?.disconnect();details?.removeEventListener('toggle',syncEditor);doc.removeEventListener('fullscreenchange',changed);doc.removeEventListener('webkitfullscreenchange',changed);doc.removeEventListener('keydown',key);win.removeEventListener('resize',viewport);win.visualViewport?.removeEventListener('resize',viewport);if(active)doc.documentElement.style.overflow=previousOverflow;}};
}
