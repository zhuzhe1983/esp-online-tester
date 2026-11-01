import {instructionTargets,readImagePoints} from './m5burner-image-points.js';
import {M5BURNER_API,parseBurnerDownload} from './m5burner.js';

const fail=(code,message)=>Object.assign(new Error(message),{code});
const tokenValid=value=>typeof value==='string'&&/^[A-Za-z0-9_-]{16,256}$/.test(value);
export function captchaPoint(event,rect,challenge) {
  if(!(rect.width>0&&rect.height>0))throw fail('CAPTCHA_INPUT','验证码图片尚未显示。');
  return {x:Math.max(0,Math.min(challenge.width,Math.round((event.clientX-rect.left)/rect.width*challenge.width))),y:Math.max(0,Math.min(challenge.height,Math.round((event.clientY-rect.top)/rect.height*challenge.height)))};
}
export function createBurnerCaptchaClient({fetchImpl=globalThis.fetch}={}) {
  async function post(path,payload,signal) {
    const controller=new AbortController(),abort=()=>controller.abort(),timer=setTimeout(abort,20000);
    signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
    let reader;
    try {
      const response=await fetchImpl(`${M5BURNER_API}/firmware-burn/captcha${path}`,{method:'POST',mode:'cors',credentials:'omit',cache:'no-store',headers:{'Content-Type':'application/json'},body:JSON.stringify(payload),signal:controller.signal});
      const max=2*1024*1024;let data;
      if(Number(response.headers.get('content-length'))>max)throw fail('CAPTCHA_RESPONSE','验证码响应过大。');
      reader=response.body?.getReader();
      if(reader){const chunks=[];let size=0;for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>max)throw fail('CAPTCHA_RESPONSE','验证码响应过大。');chunks.push(value);}const bytes=new Uint8Array(size);let at=0;for(const chunk of chunks){bytes.set(chunk,at);at+=chunk.length;}data=JSON.parse(new TextDecoder().decode(bytes));}
      else {const text=await response.text();if(text.length>max)throw fail('CAPTCHA_RESPONSE','验证码响应过大。');data=JSON.parse(text);}
      if(controller.signal.aborted)throw fail('FETCH_ABORTED','验证码请求已取消或超时。');
      if(response.status===429)throw fail('CAPTCHA_RATE_LIMIT','请求过于频繁，请稍后再试。');
      // Do not echo provider messages, tokens or request IDs into diagnostics.
      if(!response.ok||data?.code!==200)throw fail('CAPTCHA_REJECTED','验证未通过或已过期，请换一张图片后重试。');
      if(!tokenValid(data?.data?.captchaToken))throw fail('CAPTCHA_RESPONSE','验证码响应格式有变，请前往官方页面。');
      return data.data;
    } catch(error) {
      if(controller.signal.aborted)throw fail('FETCH_ABORTED','验证码请求已取消或超时。');
      if(typeof error.code==='string'&&error.code.startsWith('CAPTCHA_'))throw error;
      throw fail('CAPTCHA_UNAVAILABLE','无法读取验证码，请检查网络或前往官方页面。');
    } finally {await reader?.cancel().catch(()=>{});reader?.releaseLock();clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  }
  return {
    async issue(url,signal) {
      const ids=parseBurnerDownload(url);if(!ids)throw fail('INVALID_SOURCE','请选择 M5Burner 的固定版本下载地址。');
      const c=await post('',ids,signal);
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
export function requestBurnerVerification(url,{signal,client=createBurnerCaptchaClient()}={}) {
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
    let challenge=null,clicks=[],deadline=0,working=false,settled=false,position={x:0,y:0};const controller=new AbortController();
    const finish=(error,token)=>{if(settled)return;settled=true;controller.abort();clearInterval(timer);signal?.removeEventListener('abort',abort);dialog.close();dialog.remove();challenge=null;clicks=[];img.removeAttribute('src');error?reject(error):resolve(token);};
    const abort=()=>finish(fail('FETCH_ABORTED','已取消下载验证。'));
    function render() {
      const seconds=Math.max(0,Math.ceil((deadline-Date.now())/1000));submit.disabled=working||!challenge||!seconds;clear.disabled=working||!clicks.length;refresh.disabled=working;
      status.textContent=challenge?`${clicks.length}/${challenge.targetCount} · ${seconds?`剩余 ${seconds} 秒`:'验证码已过期，请换一张'}`:'';
      markers.replaceChildren(...clicks.map((p,i)=>{const mark=node('span',String(i+1),'burner-captcha-mark');mark.style.left=`${p.x/challenge.width*100}%`;mark.style.top=`${p.y/challenge.height*100}%`;return mark;}));
    }
    async function issue() {
      if(working||settled)return;working=true;challenge=null;clicks=[];surface.hidden=true;cursor.hidden=true;feedback.textContent='';instruction.textContent='正在获取验证码…';render();
      try{const c=await client.issue(url,controller.signal);if(settled)return;challenge=c;deadline=Date.now()+c.expiresInSeconds*1000;position={x:c.width/2,y:c.height/2};img.src=c.captchaImage;surface.style.aspectRatio=`${c.width}/${c.height}`;surface.hidden=false;
        const targets=instructionTargets(c.instruction);instruction.replaceChildren();
        if(targets.length){const words=node('ol',null,'burner-captcha-targets');words.setAttribute('data-i18n-skip','');for(const [i,text] of targets.entries()){const item=node('li');item.append(node('span',String(i+1)),node('strong',text));words.append(item);}instruction.append(node('p','请按顺序点击：'),words);}else instruction.textContent=c.instruction;
      }
      catch(e){if(!settled){feedback.textContent=e.message;instruction.textContent='验证码暂时不可用';}}
      finally{working=false;if(!settled)render();}
    }
    function select(p){if(!challenge||working||deadline<=Date.now()||clicks.length>=challenge.targetCount)return;clicks.push(p);render();}
    surface.addEventListener('click',event=>{if(challenge)select(captchaPoint(event,surface.getBoundingClientRect(),challenge));});
    surface.addEventListener('keydown',event=>{if(!challenge||working)return;if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Enter',' '].includes(event.key)){event.preventDefault();const step=event.shiftKey?10:2;if(event.key==='ArrowLeft')position.x-=step;if(event.key==='ArrowRight')position.x+=step;if(event.key==='ArrowUp')position.y-=step;if(event.key==='ArrowDown')position.y+=step;position.x=Math.round(Math.max(0,Math.min(challenge.width,position.x)));position.y=Math.round(Math.max(0,Math.min(challenge.height,position.y)));cursor.hidden=false;cursor.style.left=`${position.x/challenge.width*100}%`;cursor.style.top=`${position.y/challenge.height*100}%`;if(event.key==='Enter'||event.key===' ')select({...position});}});
    clear.onclick=()=>{clicks=[];render();};refresh.onclick=issue;cancel.onclick=abort;dialog.addEventListener('cancel',event=>{event.preventDefault();abort();});dialog.addEventListener('close',()=>{if(!settled)abort();});
    async function verify(fromImage=false) {
      if(submit.disabled||settled)return;
      if(!fromImage&&clicks.length!==challenge.targetCount){feedback.textContent='请按顺序点选图片中的全部文字。';return;}
      working=true;feedback.textContent='正在验证…';render();
      try {
        if(fromImage){
          let result;
          try{result=await readImagePoints(challenge,{signal:controller.signal,onProgress:text=>{if(!settled)feedback.textContent=text;}});}
          catch{if(!settled)feedback.textContent='暂未完成，请按提示点击图片后继续。';return;}
          if(settled)return;
          if(deadline<=Date.now()){feedback.textContent='验证码已过期，请换一张';return;}
          if(!result.complete||result.points.length!==challenge.targetCount){feedback.textContent='暂未完成，请按提示点击图片后继续。';return;}
          clicks=result.points;feedback.textContent='正在验证…';render();
        }
        const token=await client.verify(challenge,clicks,controller.signal);finish(null,token);
      }catch(e){if(!settled){challenge=null;clicks=[];surface.hidden=true;feedback.textContent=e.message;}}
      finally{working=false;if(!settled)render();}
    }
    submit.onclick=event=>{if(event.detail<2)void verify();};
    submit.ondblclick=event=>{event.preventDefault();void verify(true);};
    const timer=setInterval(render,1000);signal?.addEventListener('abort',abort,{once:true});document.body.append(dialog);dialog.showModal();cancel.focus();if(signal?.aborted)abort();else void issue();
  });
}
