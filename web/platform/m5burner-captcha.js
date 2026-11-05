import {instructionTargets} from './m5burner-image-points.js';
import {M5BURNER_API,parseBurnerDownload} from './m5burner.js';

const fail=(code,message)=>Object.assign(new Error(message),{code});
const tokenValid=value=>typeof value==='string'&&/^[A-Za-z0-9_-]{16,256}$/.test(value);
export const CAPTCHA_ATTEMPT_MS=10000, CAPTCHA_RESOURCE_MS=20000;
// Bound the entire attempt, including readers that do not settle on abort.
// Promise.race also consumes late rejections; stale results cannot win.
export async function captchaAttempt(work,{signal,timeoutMs=CAPTCHA_ATTEMPT_MS}={}) {
  const controller=new AbortController();let timer,abort,limit,finished=false;
  const interrupted=new Promise((_,reject)=>{
    const stop=error=>{reject(error);controller.abort(error);};
    abort=()=>stop(fail('FETCH_ABORTED','已取消下载验证。'));
    limit=ms=>{if(finished||controller.signal.aborted)return;clearTimeout(timer);timer=setTimeout(()=>stop(fail('CAPTCHA_TIMEOUT',`本次操作超过 ${Math.ceil(ms/1000)} 秒。`)),ms);};limit(timeoutMs);
    signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
  });
  try{return await Promise.race([interrupted,Promise.resolve().then(()=>{if(controller.signal.aborted)throw controller.signal.reason;return work(controller.signal,limit);})]);}
  finally{finished=true;clearTimeout(timer);signal?.removeEventListener('abort',abort);controller.abort();}
}
export function captchaPoint(event,rect,challenge) {
  if(!(rect.width>0&&rect.height>0))throw fail('CAPTCHA_INPUT','验证码图片尚未显示。');
  return {x:Math.max(0,Math.min(challenge.width,Math.round((event.clientX-rect.left)/rect.width*challenge.width))),y:Math.max(0,Math.min(challenge.height,Math.round((event.clientY-rect.top)/rect.height*challenge.height)))};
}
export function createBurnerCaptchaClient({fetchImpl=globalThis.fetch}={}) {
  async function post(path,payload,signal,timeoutMs=CAPTCHA_ATTEMPT_MS) {
    let timedOut=false;
    const controller=new AbortController(),abort=()=>controller.abort(),timer=setTimeout(()=>{timedOut=true;abort();},timeoutMs);
    signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
    let reader;
    try {
      const response=await fetchImpl(`${M5BURNER_API}/firmware-burn/captcha${path}`,{method:'POST',mode:'cors',credentials:'omit',cache:'no-store',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload),signal:controller.signal});
      const max=2*1024*1024;let data;
      if(Number(response.headers.get('content-length'))>max)throw fail('CAPTCHA_RESPONSE','验证码响应过大。');
      reader=response.body?.getReader();
      if(reader){const chunks=[];let size=0;for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>max)throw fail('CAPTCHA_RESPONSE','验证码响应过大。');chunks.push(value);}const bytes=new Uint8Array(size);let at=0;for(const chunk of chunks){bytes.set(chunk,at);at+=chunk.length;}data=JSON.parse(new TextDecoder().decode(bytes));}
      else {const text=await response.text();if(text.length>max)throw fail('CAPTCHA_RESPONSE','验证码响应过大。');data=JSON.parse(text);}
      if(controller.signal.aborted)throw fail(timedOut?'CAPTCHA_TIMEOUT':'FETCH_ABORTED',timedOut?`验证码请求超过 ${timeoutMs/1000} 秒，请重试。`:'已取消下载验证。');
      if(response.status===429)throw fail('CAPTCHA_RATE_LIMIT','请求过于频繁，请稍后再试。');
      // Do not echo provider messages, tokens or request IDs into diagnostics.
      if(!response.ok||data?.code!==200)throw fail('CAPTCHA_REJECTED','验证未通过或已过期，请换一张图片后重试。');
      if(!tokenValid(data?.data?.captchaToken))throw fail('CAPTCHA_RESPONSE','验证码响应格式有变，请前往官方页面。');
      return data.data;
    } catch(error) {
      if(controller.signal.aborted)throw fail(timedOut?'CAPTCHA_TIMEOUT':'FETCH_ABORTED',timedOut?`验证码请求超过 ${timeoutMs/1000} 秒，请重试。`:'已取消下载验证。');
      if(typeof error.code==='string'&&error.code.startsWith('CAPTCHA_'))throw error;
      throw fail('CAPTCHA_UNAVAILABLE','无法读取验证码，请检查网络或前往官方页面。');
    } finally {await reader?.cancel().catch(()=>{});reader?.releaseLock();clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  }
  return {
    async issue(url,signal) {
      const ids=parseBurnerDownload(url);if(!ids)throw fail('INVALID_SOURCE','请选择 M5Burner 的固定版本下载地址。');
      const c=await post('',ids,signal,CAPTCHA_RESOURCE_MS);
      if(!Number.isInteger(c.width)||!Number.isInteger(c.height)||c.width<1||c.height<1||c.width>2048||c.height>2048||!Number.isInteger(c.targetCount)||c.targetCount<1||c.targetCount>12||!Number.isFinite(c.expiresInSeconds)||c.expiresInSeconds<1||c.expiresInSeconds>600||typeof c.instruction!=='string'||c.instruction.length>500||typeof c.captchaImage!=='string'||!/^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(c.captchaImage))throw fail('CAPTCHA_RESPONSE','验证码响应格式有变，请前往官方页面。');
      return c;
    },
    async verify(challenge,clicks,signal) {
      if(!tokenValid(challenge?.captchaToken)||!Array.isArray(clicks)||clicks.length!==challenge.targetCount||clicks.some(p=>!Number.isInteger(p.x)||!Number.isInteger(p.y)||p.x<0||p.y<0||p.x>challenge.width||p.y>challenge.height))throw fail('CAPTCHA_INPUT','请按提示点完全部文字。');
      return (await post('/verify',{captchaToken:challenge.captchaToken,clicks:clicks.map(({x,y})=>({x,y}))},signal)).captchaToken;
    },
  };
}

// UI only. Tokens remain in this dialog's closure and the following request.
export function requestBurnerVerification(url,{signal,client=createBurnerCaptchaClient(),attemptTimeoutMs=CAPTCHA_ATTEMPT_MS}={}) {
  if(!parseBurnerDownload(url))return Promise.reject(fail('INVALID_SOURCE','请选择 M5Burner 的固定版本下载地址。'));
  const node=(tag,text,cls)=>{const e=document.createElement(tag);if(text)e.textContent=text;if(cls)e.className=cls;return e;};
  const dialog=node('dialog',null,'firmware-run-confirm burner-captcha');dialog.setAttribute('aria-labelledby','burner-captcha-title');
  const title=node('h2','完成 M5Burner 下载验证');title.id='burner-captcha-title';
  const instruction=node('div','正在获取验证码…','burner-captcha-instruction');instruction.id='burner-captcha-instruction';
  const surface=node('div',null,'burner-captcha-image');surface.tabIndex=0;surface.setAttribute('role','group');surface.setAttribute('aria-describedby',instruction.id);surface.setAttribute('aria-label','验证码图片；方向键移动光标，Enter 选择');
  const img=node('img');img.alt='M5Burner 验证码';img.draggable=false;
  const markers=node('div'),cursor=node('span',null,'burner-captcha-cursor');cursor.hidden=true;surface.append(img,markers,cursor);surface.hidden=true;
  const status=node('p',null,'helper');status.setAttribute('role','status');status.setAttribute('aria-live','polite');
  const feedback=node('p',null,'form-error');feedback.setAttribute('role','alert');
  const official=node('a','打开官方页面 ↗','text-link');official.href=`https://burner.m5stack.com/firmware/${parseBurnerDownload(url).firmwareId}`;official.target='_blank';official.rel='noopener noreferrer';
  const actions=node('div',null,'dialog-actions');const clear=node('button','清空选择','button'),refresh=node('button','换一张','button'),cancel=node('button','取消','button'),submit=node('button','验证并继续','button primary');for(const b of [clear,refresh,cancel,submit])b.type='button';actions.append(clear,refresh,cancel,submit);dialog.append(title,instruction,surface,status,feedback,official,actions);
  return new Promise((resolve,reject)=>{
    let challenge=null,clicks=[],deadline=0,working=false,settled=false,position={x:0,y:0},generation=0,operation=null;
    const current=id=>!settled&&generation===id;
    const begin=()=>{operation?.abort();operation=new AbortController();return ++generation;};
    const finish=(error,token)=>{if(settled)return;settled=true;generation++;operation?.abort();clearInterval(timer);signal?.removeEventListener('abort',abort);dialog.close();dialog.remove();challenge=null;clicks=[];img.removeAttribute('src');error?reject(error):resolve(token);};
    const abort=()=>finish(fail('FETCH_ABORTED','已取消下载验证。'));
    function render() {
      const seconds=Math.max(0,Math.ceil((deadline-Date.now())/1000));submit.disabled=working||!challenge||!seconds||clicks.length!==challenge.targetCount;clear.disabled=working||!clicks.length;refresh.disabled=working;
      status.textContent=challenge?`${clicks.length}/${challenge.targetCount} · ${seconds?`验证码剩余 ${seconds} 秒`:'验证码已过期，请换一张'}`:'';
      markers.replaceChildren(...clicks.map((p,i)=>{const mark=node('span',String(i+1),'burner-captcha-mark');mark.style.left=`${p.x/challenge.width*100}%`;mark.style.top=`${p.y/challenge.height*100}%`;return mark;}));
    }
    async function loadChallenge(id) {
      challenge=null;clicks=[];surface.hidden=true;cursor.hidden=true;instruction.textContent='正在获取验证码…';render();
      const c=await captchaAttempt(s=>client.issue(url,s),{signal:operation.signal,timeoutMs:CAPTCHA_RESOURCE_MS});
      if(!current(id))return;
      challenge=c;deadline=Date.now()+c.expiresInSeconds*1000;position={x:c.width/2,y:c.height/2};img.src=c.captchaImage;surface.style.aspectRatio=`${c.width}/${c.height}`;surface.hidden=false;
      const targets=instructionTargets(c.instruction);instruction.replaceChildren();
      if(targets.length){const words=node('ol',null,'burner-captcha-targets');words.setAttribute('data-i18n-skip','');for(const [i,text] of targets.entries()){const item=node('li');item.append(node('span',String(i+1)),node('strong',text));words.append(item);}instruction.append(node('p','请按顺序点击：'),words);}else instruction.textContent=c.instruction;
    }
    async function issue() {
      if(working||settled)return;const id=begin();working=true;feedback.textContent='';
      try{await loadChallenge(id);}
      catch(e){if(current(id)){feedback.textContent=e.message;instruction.textContent='验证码暂时不可用';}}
      finally{if(current(id)){working=false;render();}}
    }
    function select(p){if(!challenge||working||deadline<=Date.now()||clicks.length>=challenge.targetCount)return;clicks.push(p);render();}
    surface.addEventListener('click',event=>{if(challenge&&event.detail<2)select(captchaPoint(event,surface.getBoundingClientRect(),challenge));});
    surface.addEventListener('keydown',event=>{if(!challenge||working)return;if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Enter',' '].includes(event.key)){event.preventDefault();const step=event.shiftKey?10:2;if(event.key==='ArrowLeft')position.x-=step;if(event.key==='ArrowRight')position.x+=step;if(event.key==='ArrowUp')position.y-=step;if(event.key==='ArrowDown')position.y+=step;position.x=Math.round(Math.max(0,Math.min(challenge.width,position.x)));position.y=Math.round(Math.max(0,Math.min(challenge.height,position.y)));cursor.hidden=false;cursor.style.left=`${position.x/challenge.width*100}%`;cursor.style.top=`${position.y/challenge.height*100}%`;if(event.key==='Enter'||event.key===' ')select({...position});}});
    clear.onclick=()=>{if(working)return;clicks=[];render();};refresh.onclick=event=>{if(event.detail<2)void issue();};cancel.onclick=abort;dialog.addEventListener('cancel',event=>{event.preventDefault();abort();});dialog.addEventListener('close',()=>{if(!settled)abort();});
    async function verify() {
      if(submit.disabled||working||settled)return;
      const id=begin();working=true;feedback.textContent='正在验证…';render();
      try {const token=await captchaAttempt(s=>client.verify(challenge,clicks,s),{signal:operation.signal,timeoutMs:attemptTimeoutMs});if(current(id))finish(null,token);}
      catch(e){if(current(id)){challenge=null;clicks=[];surface.hidden=true;feedback.textContent=e.message;}}
      finally{if(current(id)){working=false;render();}}
    }
    submit.onclick=event=>{if(event.detail<2)void verify();};
    const timer=setInterval(render,250);signal?.addEventListener('abort',abort,{once:true});document.body.append(dialog);dialog.showModal();cancel.focus();if(signal?.aborted)abort();else void issue();
  });
}
