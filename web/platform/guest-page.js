// Render the guest's actual HTTP pages in an opaque-origin sandbox. No host socket or service worker.
const fail=(code,message)=>Object.assign(new Error(message),{code});
const MAX_BODY=2*1024*1024, MAX_RESOURCES=48, MAX_RESOURCE_BYTES=4*1024*1024, MAX_REQUEST=32768;
const dec=new TextDecoder(),enc=new TextEncoder();
export function guestPageUrl(value,base) {
  let url;try{url=new URL(value,base);}catch{throw fail('GUEST_URL','Invalid firmware URL.');}
  const origin=new URL(base).origin;
  if(url.protocol!=='http:'||url.origin!==origin||url.username||url.password)throw fail('GUEST_URL','The embedded page can request only this firmware HTTP service.');
  return url;
}
export function checkGuestPagePolicy(headers={},html='') {
  const h=Object.fromEntries(Object.entries(headers).map(([k,v])=>[k.toLowerCase(),String(v)]));
  if(h['x-frame-options'])throw fail('GUEST_FRAME_POLICY','This firmware forbids this isolated embedded view (X-Frame-Options). Use an advanced network mapping.');
  const policies=[h['content-security-policy'],...html.matchAll(/<meta\b[^>]*http-equiv\s*=\s*["']?content-security-policy["']?[^>]*>/gi)].filter(Boolean).map(x=>typeof x==='string'?x:x[0]);
  // Requests are proxied and resources become data URLs: arbitrary firmware CSP cannot be
  // enforced faithfully in this origin. Refuse it instead of silently weakening any directive.
  if(policies.some(p=>p.trim()))throw fail('GUEST_FRAME_POLICY','The firmware CSP cannot be preserved by this isolated page adapter. Use an advanced network mapping.');
  return h['content-security-policy']||'';
}
function bytesOf(response) {
  const bytes=response.decodedBodyBytes??response.bodyBytes;
  if(bytes instanceof Uint8Array)return bytes;
  if(bytes instanceof ArrayBuffer)return new Uint8Array(bytes);
  if(response.decodedBodyBase64||response.bodyBase64)return Uint8Array.from(atob(response.decodedBodyBase64||response.bodyBase64),c=>c.charCodeAt(0));
  return enc.encode(response.body||'');
}
function base64(bytes) {let value='';for(let i=0;i<bytes.length;i+=8192)value+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(value);}
function headersOf(response){return Object.fromEntries(Object.entries(response.headers||{}).map(([key,value])=>[key.toLowerCase(),String(value)]));}

// Preserve strings/comments/regex literals. A virtual Location is needed because the browser's
// native Location object is not replaceable. Dynamic generated code/module imports are not rewritten.
export function rewriteGuestScript(source) {
  const tokens=[];let i=0,previous='';
  while(i<source.length){const start=i,c=source[i],n=source[i+1];
    if(/\s/.test(c)){while(i<source.length&&/\s/.test(source[i]))i++;tokens.push(source.slice(start,i));continue;}
    if(c==='/'&&n==='/'){i=source.indexOf('\n',i+2);if(i<0)i=source.length;tokens.push(source.slice(start,i));continue;}
    if(c==='/'&&n==='*'){const end=source.indexOf('*/',i+2);i=end<0?source.length:end+2;tokens.push(source.slice(start,i));continue;}
    if(c==='"'||c==="'"||c==='`'){i++;while(i<source.length){if(source[i]==='\\'){i+=2;continue;}if(source[i++]===c)break;}tokens.push(source.slice(start,i));previous='literal';continue;}
    if(c==='/'&&(!previous||/^(?:[=(:,!&|?;{}\[]|return|throw|case|=>)$/.test(previous))){i++;let bracket=false;while(i<source.length){if(source[i]==='\\'){i+=2;continue;}if(source[i]==='[')bracket=true;if(source[i]===']')bracket=false;if(source[i++]==='/'&&!bracket)break;}while(/[a-z]/i.test(source[i]||'')&&i<source.length)i++;tokens.push(source.slice(start,i));previous='literal';continue;}
    if(/[\w$]/.test(c)){i++;while(i<source.length&&/[\w$]/.test(source[i]))i++;let word=source.slice(start,i);
      if(word==='location'){
        const significant=tokens.filter(t=>!/^\s*$|^\/\//.test(t)&&!t.startsWith('/*'));
        if(previous!=='.'||['window','self','globalThis','document'].includes(significant.at(-2)))word='__espGuestLocation';
      }
      tokens.push(word);previous=word;continue;
    }
    tokens.push(c);previous=c;i++;
  }
  return tokens.join('');
}

// Templates discard document wrapper attributes. Give those wrappers inert names while
// parsing, without touching comments, strings inside scripts, or raw text elements.
export function wrapGuestDocumentTags(source) {
  let result='',index=0,raw='';
  while(index<source.length){
    if(raw){const end=source.toLowerCase().indexOf('</'+raw,index);if(end<0)return result+source.slice(index);result+=source.slice(index,end);index=end;raw='';}
    if(source.startsWith('<!--',index)){const end=source.indexOf('-->',index+4);const next=end<0?source.length:end+3;result+=source.slice(index,next);index=next;continue;}
    if(source[index]!=='<'){result+=source[index++];continue;}
    let end=index+1,quote='';for(;end<source.length;end++){const c=source[end];if(quote){if(c===quote)quote='';}else if(c==='"'||c==="'")quote=c;else if(c==='>')break;}
    const tag=source.slice(index,Math.min(end+1,source.length)),match=/^<(\/?)([a-z][\w:-]*)\b/i.exec(tag);
    if(match){const name=match[2].toLowerCase();if(['html','head','body'].includes(name))result+=tag.replace(match[2],'esp-guest-'+name);else result+=tag;if(!match[1]&&['script','style','textarea','title'].includes(name))raw=name;}else result+=tag;
    index=end+1;
  }
  return result;
}

// This function is serialized into the sandbox and has no access to workbench state.
function guestBootstrap(config) {
  const pending=new Map();let sequence=0,current=config.url;
  const target=value=>{const url=new URL(value,current);if(url.origin!==new URL(config.url).origin||url.username||url.password)throw new TypeError('Only this firmware HTTP service is available here.');return url;};
  const message=(type,value={})=>parent.postMessage({guestPage:config.token,type,...value},'*');
  const invoke=(operation,value,signal)=>new Promise((resolve,reject)=>{
    if(signal?.aborted){reject(new DOMException('Aborted','AbortError'));return;}
    const id=++sequence,timer=setTimeout(()=>{pending.delete(id);message('abort',{id});reject(new Error('Firmware request timed out.'));},95000);
    const abort=()=>{clearTimeout(timer);pending.delete(id);message('abort',{id});reject(new DOMException('Aborted','AbortError'));};
    pending.set(id,{resolve,reject,timer,signal,abort});signal?.addEventListener('abort',abort,{once:true});message(operation,{id,...value});
  });
  addEventListener('message',event=>{
    if(event.source!==parent||event.data?.guestPage!==config.token||event.data.type!=='reply')return;
    const p=pending.get(event.data.id);if(!p)return;pending.delete(event.data.id);clearTimeout(p.timer);p.signal?.removeEventListener('abort',p.abort);
    if(event.data.error)p.reject(new Error(event.data.error.message));else p.resolve(event.data.result);
  });
  const navigate=value=>{const url=target(value);if(url.pathname===new URL(current).pathname&&url.search===new URL(current).search&&url.hash){current=url.href;document.getElementById(decodeURIComponent(url.hash.slice(1)))?.scrollIntoView();return;}message('navigate',{url:url.href});};
  const location={assign:navigate,replace:navigate,reload:()=>navigate(current),toString:()=>current};
  for(const key of ['href','pathname','search','hash'])Object.defineProperty(location,key,{get:()=>new URL(current)[key],set:value=>{if(key==='href'){navigate(value);return;}const url=new URL(current);url[key]=value;navigate(url.href);}});
  for(const key of ['origin','host','hostname','port','protocol'])Object.defineProperty(location,key,{get:()=>new URL(current)[key]});
  Object.defineProperty(window,'__espGuestLocation',{get:()=>location,set:navigate});
  Object.defineProperty(document,'__espGuestLocation',{get:()=>location,set:navigate});
  const NativeRequest=Request;
  window.fetch=async(input,init={})=>{
    const isRequest=input instanceof NativeRequest,url=target(isRequest?input.url:String(input));
    const method=init.method||(isRequest?input.method:'GET'),headers=init.headers||(isRequest?input.headers:undefined);
    const originalBody=init.body!==undefined?init.body:isRequest&&!['GET','HEAD'].includes(method.toUpperCase())?await input.clone().arrayBuffer():undefined;
    const request=new NativeRequest(url.href,{...init,method,headers,body:originalBody});
    const body=['GET','HEAD'].includes(request.method)?undefined:new Uint8Array(await request.arrayBuffer());
    if(body?.byteLength>32768)throw new TypeError('Firmware request body exceeds 32 KiB.');
    const result=await invoke('http',{url:url.href,method:request.method,headers:Object.fromEntries(request.headers),body},init.signal||request.signal);
    const response=new Response([101,204,205,304].includes(result.status)?null:result.bodyBytes,{status:result.status,statusText:result.statusText||'',headers:result.headers});
    Object.defineProperty(response,'url',{value:result.url||url.href});return response;
  };
  class GuestXHR extends EventTarget {
    constructor(){super();this.readyState=0;this.status=0;this.statusText='';this.responseType='';this.responseText='';this.response=null;this.timeout=0;this.withCredentials=false;this.upload=new EventTarget();this.headers={};}
    event(name){const event=new Event(name);this.dispatchEvent(event);this['on'+name]?.call(this,event);}
    state(value){this.readyState=value;this.event('readystatechange');}
    open(method,url,async=true){if(!async)throw new Error('Synchronous firmware XHR is unsupported.');this.method=method;this.url=target(url).href;this.headers={};this.state(1);}
    setRequestHeader(key,value){this.headers[key]=this.headers[key]?this.headers[key]+', '+value:String(value);}
    getResponseHeader(key){return this.responseHeaders?.get(key)||null;}
    getAllResponseHeaders(){return this.responseHeaders?[...this.responseHeaders].map(([k,v])=>k+': '+v+'\r\n').join(''):'';}
    overrideMimeType(type){this.mime=type;}
    abort(){this.controller?.abort();}
    async send(body){this.controller=new AbortController();let timedOut=false;const timer=this.timeout>0?setTimeout(()=>{timedOut=true;this.controller.abort();},this.timeout):null;this.event('loadstart');
      try{const response=await fetch(this.url,{method:this.method,headers:this.headers,body,signal:this.controller.signal});this.status=response.status;this.statusText=response.statusText;this.responseURL=response.url;this.responseHeaders=response.headers;this.state(2);const bytes=await response.arrayBuffer();this.state(3);
        if(this.responseType==='arraybuffer')this.response=bytes;else if(this.responseType==='blob')this.response=new Blob([bytes],{type:this.mime||response.headers.get('content-type')||''});else{this.responseText=new TextDecoder().decode(bytes);this.response=this.responseType==='json'?JSON.parse(this.responseText):this.responseType==='document'?new DOMParser().parseFromString(this.responseText,'text/html'):this.responseText;}
        this.state(4);this.event('load');
      }catch(error){this.error=error;this.status=0;this.state(4);this.event(timedOut?'timeout':error.name==='AbortError'?'abort':'error');}finally{clearTimeout(timer);this.event('loadend');}
    }
  }
  for(const [key,value]of Object.entries({UNSENT:0,OPENED:1,HEADERS_RECEIVED:2,LOADING:3,DONE:4})){GuestXHR[key]=value;GuestXHR.prototype[key]=value;}
  window.XMLHttpRequest=GuestXHR;
  window.WebSocket=class{constructor(){throw new Error('Guest WebSocket is not supported by the browser page adapter. Use an advanced network mapping.');}};
  window.open=url=>{navigate(url);return null;};
  const submit=(form,submitter)=>{
    const url=target(submitter?.getAttribute('formaction')||form.getAttribute('action')||current),method=(submitter?.getAttribute('formmethod')||form.method||'GET').toUpperCase();
    if(method!=='GET'&&(submitter?.getAttribute('formenctype')||form.enctype)!=='application/x-www-form-urlencoded')throw new Error('This form encoding needs an advanced network mapping.');
    const data=new FormData(form,submitter);
    const entries=[...data];if(entries.some(([,value])=>typeof value!=='string'))throw new Error('File upload needs the advanced network mapping.');
    const body=new URLSearchParams(entries).toString();if(method==='GET'){url.search=body;message('navigate',{url:url.href});}else message('navigate',{url:url.href,method,headers:{'Content-Type':'application/x-www-form-urlencoded'},body});
  };
  document.addEventListener('click',event=>{const a=event.target.closest?.('a[href]');if(!a||event.defaultPrevented)return;event.preventDefault();try{navigate(a.getAttribute('href'));}catch(error){message('error',{message:error.message});}});
  document.addEventListener('submit',event=>{if(event.defaultPrevented)return;event.preventDefault();try{submit(event.target,event.submitter);}catch(error){message('error',{message:error.message});}});
  HTMLFormElement.prototype.submit=function(){submit(this);};
  addEventListener('error',event=>message('error',{message:event.message||'A firmware page resource failed to load.'}));
  addEventListener('unhandledrejection',event=>message('error',{message:event.reason?.message||String(event.reason)}));
  message('ready');
}

// open() returns after assigning the isolated document, not after firmware JavaScript settles.
// Supported: classic scripts, fetch, async XHR, static resources, ordinary forms and navigation.
// Cookies, module/dynamic imports, generated navigation, WebSocket and arbitrary CSP are unsupported.
export function mountGuestPage({iframe,requestHttp,guestAddress,onError=()=>{},onNavigate=()=>{}}={}) {
  if(!iframe||typeof requestHttp!=='function')throw new TypeError('Guest page requires an iframe and HTTP transport.');
  const doc=iframe.ownerDocument,win=doc.defaultView;
  let disposed=false,generation=0,session=null;
  iframe.setAttribute('sandbox','allow-scripts allow-forms');iframe.setAttribute('referrerpolicy','no-referrer');
  const stop=()=>{generation++;for(const request of session?.active||[])request.abort();session=null;};
  const address=()=>{const value=typeof guestAddress==='function'?guestAddress():guestAddress;if(typeof value!=='string'||!/^\d{1,3}(?:\.\d{1,3}){3}$/.test(value)||value.split('.').some(x=>Number(x)>255))throw fail('GUEST_ADDRESS','Wait for the firmware SoftAP address.');return value;};
  const send=(target,id,result,error)=>{if(session!==target||disposed)return;iframe.contentWindow?.postMessage({guestPage:target.token,type:'reply',id,...(error?{error:{code:error.code||'GUEST_HTTP',message:error.message}}:{result})},'*');};
  const perform=async(target,url,options={})=>{
    let method=options.method||'GET',body=options.body,bodyBase64=options.bodyBase64,headers=options.headers;
    for(let redirects=0;redirects<=5;redirects++){
      if(session!==target||disposed)throw fail('GUEST_PAGE_CLOSED','Firmware page closed.');
      const parsed=guestPageUrl(url,target.origin);
      const controller=new AbortController(),abort=()=>controller.abort();target.active.add(controller);
      if(options.signal?.aborted)abort();else options.signal?.addEventListener('abort',abort,{once:true});
      let response;try{response=await requestHttp({guestPort:target.port,path:parsed.pathname+parsed.search,method,headers,body,bodyBase64,signal:controller.signal});}
      finally{target.active.delete(controller);options.signal?.removeEventListener('abort',abort);}
      if(session!==target||disposed)throw fail('GUEST_PAGE_CLOSED','Firmware page closed.');
      const h=headersOf(response),bytes=bytesOf(response);if(bytes.length>MAX_BODY)throw fail('GUEST_PAGE_LIMIT','Firmware response exceeds 2 MiB.');
      if([301,302,303,307,308].includes(response.status)&&h.location){url=guestPageUrl(h.location,parsed.href).href;if(response.status===303||([301,302].includes(response.status)&&method==='POST')){method='GET';body=undefined;bodyBase64=undefined;headers=undefined;}continue;}
      // Body is already decoded by the virtual HTTP client.
      delete h['content-encoding'];delete h['content-length'];delete h['transfer-encoding'];
      return {...response,headers:h,bodyBytes:bytes,url:parsed.href};
    }
    throw fail('GUEST_REDIRECT','Too many firmware redirects.');
  };
  async function render(target,response) {
    const source=dec.decode(response.bodyBytes),policy=checkGuestPagePolicy(response.headers,source),template=doc.createElement('template');
    template.innerHTML=wrapGuestDocumentTags(source);
    const root=template.content;
    for(const meta of root.querySelectorAll('meta[http-equiv]'))if(meta.getAttribute('http-equiv').toLowerCase()==='content-security-policy')checkGuestPagePolicy({'content-security-policy':meta.getAttribute('content')||''});
    if(root.querySelector('script[type="module"],iframe,frame,object,embed'))throw fail('GUEST_PAGE_UNSUPPORTED','Module scripts and nested browsing contexts need an advanced network mapping.');
    let count=0,total=0;const cache=new Map();
    const asset=async(value,base,kind,depth=0,trail=[])=>{
      if(/^(?:data:|#)/i.test(value))return value;
      const url=guestPageUrl(value,base).href,key=kind+':'+url;if(trail.includes(key))throw fail('GUEST_PAGE_LIMIT','Cyclic firmware stylesheets are unsupported.');if(cache.has(key))return cache.get(key);
      if(depth>4||++count>MAX_RESOURCES)throw fail('GUEST_PAGE_LIMIT','Too many nested firmware page resources.');
      const work=(async()=>{const result=await perform(target,url);if(result.status<200||result.status>=300)throw fail('GUEST_RESOURCE',`Firmware resource returned HTTP ${result.status}: ${new URL(url).pathname}`);
        total+=result.bodyBytes.length;if(total>MAX_RESOURCE_BYTES)throw fail('GUEST_PAGE_LIMIT','Firmware page resources exceed 4 MiB.');
        let bytes=result.bodyBytes,type=result.headers['content-type']?.split(';')[0]||({script:'text/javascript',style:'text/css'}[kind]||'application/octet-stream');
        if(kind==='script')bytes=enc.encode(rewriteGuestScript(dec.decode(bytes)));
        if(kind==='style')bytes=enc.encode(await css(dec.decode(bytes),result.url,depth+1,[...trail,key]));
        return `data:${type};base64,${base64(bytes)}`;
      })();cache.set(key,work);return work;
    };
    const css=async(value,base,depth=0,trail=[])=>{
      const matches=[...value.matchAll(/(@import\s+)?url\(\s*(?:"([^"]*)"|'([^']*)'|([^\s)]*))\s*\)|@import\s+(?:"([^"]*)"|'([^']*)')/gi)];
      for(let index=matches.length-1;index>=0;index--){const m=matches[index],url=m[2]??m[3]??m[4]??m[5]??m[6],isImport=m[0].toLowerCase().startsWith('@');const replacement=await asset(url,base,isImport?'style':'asset',depth,trail);value=value.slice(0,m.index)+(isImport?`@import "${replacement}"`:`url("${replacement}")`)+value.slice(m.index+m[0].length);}
      return value;
    };
    const pageUrl=response.url;
    for(const base of root.querySelectorAll('base')){const href=base.getAttribute('href');if(href&&guestPageUrl(href,pageUrl).href!==pageUrl)throw fail('GUEST_PAGE_UNSUPPORTED','A custom base URL needs an advanced network mapping.');base.remove();}
    for(const meta of root.querySelectorAll('meta[http-equiv]'))if(meta.getAttribute('http-equiv').toLowerCase()==='refresh')throw fail('GUEST_PAGE_UNSUPPORTED','Meta refresh needs an advanced network mapping.');
    for(const node of root.querySelectorAll('*')){
      for(const attr of [...node.attributes])if(attr.name.startsWith('on'))node.setAttribute(attr.name,rewriteGuestScript(attr.value));
      if(node.hasAttribute('style'))node.setAttribute('style',await css(node.getAttribute('style'),pageUrl));
      if(node.tagName==='STYLE')node.textContent=await css(node.textContent,pageUrl);
      if(node.tagName==='SCRIPT'){
        if(node.hasAttribute('integrity'))throw fail('GUEST_PAGE_UNSUPPORTED','Integrity-protected scripts need an advanced network mapping.');
        if(node.hasAttribute('src'))node.setAttribute('src',await asset(node.getAttribute('src'),pageUrl,'script'));
        else node.textContent=rewriteGuestScript(node.textContent).replace(/<\/script/gi,'<\\/script');
      }else if(node.tagName==='LINK'&&node.hasAttribute('href'))node.setAttribute('href',await asset(node.getAttribute('href'),pageUrl,node.rel.toLowerCase()==='stylesheet'?'style':'asset'));
      else if(node.hasAttribute('src'))node.setAttribute('src',await asset(node.getAttribute('src'),pageUrl,'asset'));
      if(node.hasAttribute('poster'))node.setAttribute('poster',await asset(node.getAttribute('poster'),pageUrl,'asset'));
      if(node.hasAttribute('srcset'))throw fail('GUEST_PAGE_UNSUPPORTED','Responsive image srcset needs an advanced network mapping.');
    }
    if(session!==target||disposed)throw fail('GUEST_PAGE_CLOSED','Firmware page closed.');
    const csp="default-src 'none'; script-src 'unsafe-inline' data:; style-src 'unsafe-inline' data:; img-src data:; font-src data:; media-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'";
    const escape=value=>value.replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
    const bootstrap=`(${guestBootstrap.toString()})(${JSON.stringify({token:target.token,url:pageUrl})});`;
    const wrappers=Object.fromEntries(['html','head','body'].map(name=>[name,root.querySelector('esp-guest-'+name)]));
    const attrs=name=>[...(wrappers[name]?.attributes||[])].map(attr=>` ${attr.name}="${escape(attr.value)}"`).join('');
    const htmlAttrs=attrs('html'),headAttrs=attrs('head'),bodyAttrs=attrs('body'),headHtml=wrappers.head?.innerHTML||'';
    wrappers.head?.remove();for(const name of ['body','html'])if(wrappers[name])wrappers[name].replaceWith(...wrappers[name].childNodes);
    iframe.removeAttribute('src');
    iframe.srcdoc=`<!doctype html><html${htmlAttrs}><head${headAttrs}><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${escape(csp)}">${policy?`<meta http-equiv="Content-Security-Policy" content="${escape(policy)}">`:''}<script>${bootstrap}<\/script>${headHtml}</head><body${bodyAttrs}>${template.innerHTML}</body></html>`;
    onNavigate({url:pageUrl,path:new URL(pageUrl).pathname,guestPort:target.port,status:response.status,resources:count});
    return {url:pageUrl,status:response.status,resources:count};
  }
  async function open({path='/',guestPort=80,method='GET',headers,body}={}) {
    if(disposed)throw fail('GUEST_PAGE_CLOSED','Firmware page was disposed.');
    if(!Number.isInteger(guestPort)||guestPort<1||guestPort>65535)throw fail('GUEST_PORT','Invalid firmware HTTP port.');
    stop();const token=Array.from(crypto.getRandomValues(new Uint32Array(4)),v=>v.toString(16).padStart(8,'0')).join('');
    const target={token,port:guestPort,origin:`http://${address()}:${guestPort}/`,requests:new Map(),active:new Set(),generation};session=target;
    try{return await render(target,await perform(target,guestPageUrl(path,target.origin).href,{method,headers,body}));}catch(error){if(session===target){iframe.removeAttribute('srcdoc');onError(error);}throw error;}
  }
  const receive=event=>{
    const target=session,m=event.data;
    if(disposed||!target||event.source!==iframe.contentWindow||event.origin!=='null'||m?.guestPage!==target.token)return;
    if(m.type==='error'){onError(fail('GUEST_SCRIPT',String(m.message).slice(0,500)));return;}
    if(m.type==='abort'){target.requests.get(m.id)?.abort();target.requests.delete(m.id);return;}
    if(m.type==='navigate'){
      try{const url=guestPageUrl(m.url,target.origin);void open({path:url.pathname+url.search+url.hash,guestPort:target.port,method:m.method,headers:m.headers,body:m.body}).catch(()=>{});}catch(error){onError(error);}return;
    }
    if(m.type!=='http'||!Number.isSafeInteger(m.id))return;
    if(target.requests.size>=8){send(target,m.id,null,fail('GUEST_HTTP_BUSY','At most eight firmware requests may run at once.'));return;}
    try{
      const url=guestPageUrl(m.url,target.origin),method=String(m.method||'GET').toUpperCase();
      if(!/^[A-Z]{1,16}$/.test(method)||m.body&&(!(m.body instanceof Uint8Array)||m.body.length>MAX_REQUEST))throw fail('GUEST_HTTP_REQUEST','Invalid firmware request.');
      const controller=new AbortController();target.requests.set(m.id,controller);
      // Match requestHttp's public body contract, including binary multipart data from fetch().
      const options={method,headers:m.headers,signal:controller.signal};
      if(m.body)options.bodyBase64=base64(m.body);
      void perform(target,url.href,options).then(response=>send(target,m.id,{status:response.status,statusText:response.statusText,headers:response.headers,bodyBytes:response.bodyBytes,url:response.url}),error=>send(target,m.id,null,error)).finally(()=>target.requests.delete(m.id));
    }catch(error){send(target,m.id,null,error);}
  };
  win.addEventListener('message',receive);
  return {open,dispose(){if(disposed)return;disposed=true;stop();win.removeEventListener('message',receive);iframe.removeAttribute('src');iframe.removeAttribute('srcdoc');}};
}
