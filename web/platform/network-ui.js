import {mountGuestPage} from './guest-page.js';
// One network surface for human provisioning and agent-visible runtime state.
export function mountNetworkControls({container,runtime,onError=console.error}={}) {
  if(!container)return {dispose(){}};
  const style=document.createElement('link');style.rel='stylesheet';style.href=new URL('./network-ui.css',import.meta.url).href;document.head.append(style);
  container.className='network-panel';
  container.innerHTML=`<details><summary><span class="network-dot" aria-hidden="true"></span><strong>网络</strong><span data-status>未连接网络助手</span><span class="network-summary-actions"><label><input type="checkbox" data-auto checked>自动配网</label></span></summary>
    <div class="network-body"><p class="network-description">固件 SoftAP 配网页可直接在浏览器内访问；关闭自动配网即可手动操作。访问互联网需要连接你自己部署的网络助手，本站不提供默认中继。</p>
      <div class="network-connection"><label>连接你自己的网络助手<select data-mode><option value="local-helper">本地助手 · 部署在当前电脑</option><option value="public-relay">云端助手 · 部署在自己的服务器</option></select></label>
      <div class="network-setup" data-local-help hidden><strong>1. 本地部署，指向本机</strong><ol><li>在当前电脑安装 Node.js 和 Rust，克隆项目并编译网络后端。</li><li>运行 <code>node tools/network-helper.mjs</code>，保持终端运行。</li><li>打开终端输出的 GitHub Pages 链接，或将连接地址填在下方。</li></ol><p>地址通常以 <code>ws://127.0.0.1:8787/network?token=</code> 开头。可访问互联网、电脑及局域网服务，也可将固件端口映射到本机。<a href="https://github.com/zhuzhe1983/esp-online-tester/blob/main/docs/platform/network-helper.md" target="_blank" rel="noopener noreferrer">本地安装与连接步骤 ↗</a></p></div>
      <div class="network-setup" data-cloud-help><strong>2. 云端部署，指向公网地址</strong><ol><li>准备自己的云服务器和域名，安装 Docker Compose 并克隆项目。</li><li>运行 <code>bash deploy/network-relay/start.sh 你的域名</code>。</li><li>将部署好的 <code>https://你的域名</code> 填在下方并连接。</li></ol><p>云端助手只提供互联网出口，不能访问你电脑的局域网或映射本机端口。<a href="https://github.com/zhuzhe1983/esp-online-tester/blob/main/docs/platform/public-network-relay.md" target="_blank" rel="noopener noreferrer">云端部署与检查步骤 ↗</a></p></div>
      <div class="network-connect"><label data-relay-label>云端助手 HTTPS 地址<input data-relay type="url" autocomplete="off" spellcheck="false" placeholder="https://relay.your-domain.tld"></label><label data-helper-label hidden>本地助手连接地址<input data-endpoint type="password" autocomplete="off" placeholder="粘贴终端输出的 Pages connection endpoint"></label><button class="button" data-connect>连接</button><button class="button quiet" data-offline>断开外网</button></div>
      <p class="network-helper">地址与连接凭据仅用于当前会话，不会放进场景分享链接。<a href="https://github.com/zhuzhe1983/esp-online-tester/blob/main/docs/platform/network-setup.md" target="_blank" rel="noopener noreferrer">两种方式怎么选 ↗</a></p></div>
      <div class="network-addresses" data-addresses></div><p class="network-error" data-error role="status"></p>
      <div class="network-services" data-services></div>
      <form class="network-add"><select data-interface aria-label="固件网络接口"><option value="0">STA 应用服务</option><option value="1">SoftAP 服务</option></select><select data-protocol aria-label="服务协议"><option>http</option><option>https</option><option>tcp</option><option>udp</option></select><input data-port type="number" min="1" max="65535" value="80" aria-label="固件服务端口"><button class="button" type="submit">添加服务</button></form>
      <div class="network-page" hidden><div><strong data-page-title>固件网页</strong><a class="text-link" data-external target="_blank" rel="noopener noreferrer">独立打开 ↗</a><button class="button quiet" data-close-page>关闭网页</button></div><iframe title="固件原始网络页面" sandbox="allow-scripts allow-forms" referrerpolicy="no-referrer"></iframe></div>
    </div></details>`;
  const $=selector=>container.querySelector(selector),run=action=>Promise.resolve().then(()=>{if(alive){uiError='';return action();}}).catch(error=>{if(!alive)return;uiError=error.message;$('[data-error]').textContent=uiError;onError(error);});
  let current=null,serviceKey='',alive=true,openingManual=false,manualClosed=false,displayedMapping=null,guestPage=null,uiError='';
  const openPage=async mapping=>{
    if(!alive)return;
    guestPage?.dispose();guestPage=null;
    if(mapping.browser){displayedMapping=mapping.mappingId;$('.network-page').hidden=false;$('[data-external]').hidden=true;
      guestPage=mountGuestPage({iframe:$('iframe'),requestHttp:options=>runtime.requestNetworkHttp({interface:1,...options}),guestAddress:mapping.guestAddress,onError:error=>{if(alive){uiError=error.message;$('[data-error]').textContent=uiError;}}});
      await guestPage.open({path:'/',guestPort:mapping.guestPort});return;
    }
    $('[data-external]').hidden=false;$('iframe').removeAttribute('srcdoc');$('iframe').setAttribute('sandbox','allow-scripts allow-forms allow-same-origin allow-popups');
    if(!mapping.url)throw new Error('此服务没有 HTTP 入口，请使用本机客户端连接映射端口。');
    const url=new URL(mapping.url);
    // A helper allocates a distinct origin per guest HTTP service. Never run
    // untrusted firmware HTML in the workbench's own origin.
    if(!['http:','https:'].includes(url.protocol)||url.origin===location.origin)throw new Error('固件网页必须使用独立的 HTTP 来源。');
    displayedMapping=mapping.mappingId;$('.network-page').hidden=false;$('iframe').src=url.href;$('[data-external]').href=url.href;
  };
  const render=state=>{
    if(!alive)return;current=state;
    $('[data-auto]').checked=state.autoProvision;
    const labels={'relay-unconfigured':'填写自建助手地址 · 配网页可用','waiting-helper':'等待连接本地助手',connecting:'网络助手连接中',connected:state.mode==='public-relay'?'云端助手已连接':'本地助手已连接',disconnected:'外网已断开',offline:'外网已关闭',unsupported:'当前板型不支持'};
    const local=state.mode==='local-helper';$('[data-mode]').value=state.mode||'public-relay';$('[data-helper-label]').hidden=!local;$('[data-relay-label]').hidden=local;$('[data-local-help]').hidden=!local;$('[data-cloud-help]').hidden=local;
    $('[data-status]').textContent=labels[state.connection]||state.connection;
    $('.network-dot').classList.toggle('connected',state.connection==='connected');
    const a=state.interfaces?.[0],b=state.interfaces?.[1];
    $('[data-addresses]').textContent=`虚拟路由器：${state.router.ssid} · STA：${a?.leaseReady?a.guestAddress:'未取得地址'} · SoftAP：${state.softAp?.ssid||'未发现'}${b?.leaseReady?` / ${b.guestAddress}`:''}`;
    $('[data-error]').textContent=state.lastError?.message||uiError||({checking:'正在识别固件配网页…',submitting:'正在提交虚拟路由器配置…',submitted:'固件已接受配置，等待 STA 联网。',connected:'固件已取得 STA 地址。'}[state.provisioning]||'');
    if(displayedMapping&&!state.services.some(s=>s.mapping?.mappingId===displayedMapping)){displayedMapping=null;manualClosed=true;guestPage?.dispose();guestPage=null;$('.network-page').hidden=true;$('iframe').removeAttribute('src');}
    const key=JSON.stringify(state.services);
    if(key!==serviceKey){serviceKey=key;$('[data-services]').replaceChildren();for(const item of state.services){
      const row=document.createElement('div');row.className='network-service';
      const label=document.createElement('span');label.textContent=`${item.interface?'AP':'STA'} · ${item.protocol.toUpperCase()}:${item.guestPort} · ${item.verified?'已验证':item.source==='manual'?'手工添加':'待验证'}`;row.append(label);
      const button=document.createElement('button');button.className='button tiny';button.textContent=item.mapping?(item.mapping.browser||item.mapping.url?'打开网页':`本机 ${item.mapping.hostPort}`):(['http','https'].includes(item.protocol)?'打开网页':'映射到本机');
      button.addEventListener('click',()=>run(async()=>{const mapping=item.mapping||await runtime.mapNetworkPort(item);if(mapping.browser||mapping.url)await openPage(mapping);else if(alive)$('[data-error]').textContent=`本机客户端可连接 ${mapping.host||'127.0.0.1'}:${mapping.hostPort}（${item.protocol.toUpperCase()}）`;}));row.append(button);
      if(item.mapping){const close=document.createElement('button');close.className='button tiny quiet';close.textContent='取消映射';close.addEventListener('click',()=>run(()=>runtime.unmapNetworkPort({mappingId:item.mapping.mappingId})));row.append(close);}
      $('[data-services]').append(row);
    }}
    if(!state.autoProvision&&state.interfaces?.[1]?.leaseReady&&!manualClosed&&!openingManual&&!displayedMapping) {
      openingManual=true;void run(async()=>{try{const existing=state.services.find(s=>s.id==='1:http:80');await openPage(existing?.mapping||await runtime.mapNetworkPort({interface:1,protocol:'http',guestPort:80}));}finally{openingManual=false;}});
    }
    if(!state.browserSoftAp&&state.connection!=='connected'){$('.network-page').hidden=true;$('iframe').removeAttribute('src');}
  };
  $('[data-auto]').addEventListener('click',event=>event.stopPropagation());
  $('[data-auto]').addEventListener('change',()=>run(()=>(manualClosed=false,runtime.configureNetwork({autoProvision:$('[data-auto]').checked}))));
  $('[data-connect]').addEventListener('click',()=>run(async()=>{const mode=$('[data-mode]').value,endpoint=$('[data-endpoint]').value.trim(),relayUrl=$('[data-relay]').value.trim();if(mode==='local-helper'?!endpoint&&!current?.helperConfigured:!relayUrl&&!current?.relayConfigured)throw new Error(mode==='local-helper'?'请粘贴本地助手输出的 Pages connection endpoint。':'请填写你部署的云端助手 HTTPS 地址。');await runtime.configureNetwork({enabled:true,mode,...(mode==='local-helper'&&endpoint?{endpoint}:{}),...(mode==='public-relay'&&relayUrl?{relayUrl}:{})});if(current?.connection!=='connected')await runtime.connectNetwork();}));
  $('[data-mode]').addEventListener('change',()=>run(()=>runtime.configureNetwork({mode:$('[data-mode]').value})));
  $('[data-offline]').addEventListener('click',()=>run(()=>runtime.configureNetwork({enabled:false})));
  $('.network-add').addEventListener('submit',event=>{event.preventDefault();run(()=>runtime.addNetworkService({interface:Number($('[data-interface]').value),protocol:$('[data-protocol]').value,guestPort:Number($('[data-port]').value)}));});
  $('[data-close-page]').addEventListener('click',()=>{manualClosed=true;guestPage?.dispose();guestPage=null;$('.network-page').hidden=true;$('iframe').removeAttribute('src');});
  const unsubscribe=runtime.subscribe(event=>{if(event.type==='network')render(event.network);});
  void runtime.readNetwork().then(render).catch(onError);
  return {dispose(){if(!alive)return;alive=false;unsubscribe();guestPage?.dispose();$('iframe').removeAttribute('src');container.replaceChildren();style.remove();}};
}
