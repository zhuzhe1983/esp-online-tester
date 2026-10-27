// Confirmation belongs to the human UI, not to the agent/runtime execution API.
export function confirmFirmwareRun({product, firmware}) {
  if (!firmware || firmware.simulationVerified === true) return Promise.resolve(true);
  const node = (tag, text) => { const value=document.createElement(tag);if(text)value.textContent=text;return value; };
  const dialog=node('dialog');dialog.className='firmware-run-confirm';dialog.setAttribute('aria-labelledby','firmware-run-confirm-title');
  const title=node('h2','尚未确认完整适配');title.id='firmware-run-confirm-title';
  const identity=node('p',`${product.name} · ${firmware.name}`);identity.className='helper';
  const detail=node('p','此固件尚未完成适配验证。可以继续运行，但可能无法启动，或有部分功能不可用。遇到问题可点击“报告问题”，附上运行日志。');
  const limitations=(firmware.limitations||[]).filter(text=>/实验直启|app-only|跳过 ROM/.test(text));
  const actions=node('div');actions.className='dialog-actions';
  const cancel=node('button','取消');cancel.type='button';cancel.className='button';
  const proceed=node('button','继续运行');proceed.type='button';proceed.className='button primary';
  actions.append(cancel,proceed);dialog.append(title,identity,detail,...limitations.map(text=>{const p=node('p',text);p.className='helper';return p;}),actions);document.body.append(dialog);
  return new Promise(resolve=>{
    let accepted=false;
    cancel.addEventListener('click',()=>dialog.close());
    proceed.addEventListener('click',()=>{accepted=true;dialog.close();});
    dialog.addEventListener('close',()=>{dialog.remove();resolve(accepted);},{once:true});
    dialog.showModal();cancel.focus();
  });
}

function publicSource(value) {
  try { const url=new URL(value);if(!['https:','http:'].includes(url.protocol))return '[local source]';url.username='';url.password='';url.search='';url.hash='';return url.href.slice(0,500); } catch { return ''; }
}
export function redactIssueText(value) {
  return String(value||'')
    .replace(/https?:\/\/[^\s<>"']+/gi,publicSource)
    .replace(/\/(?:Users|home)\/[^\s"'<>]+/g,'[local path redacted]')
    .replace(/\b(?:[0-9a-f]{2}:){5}[0-9a-f]{2}\b/gi,'[MAC redacted]')
    .replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g,'[IP redacted]')
    .replace(/\b[\w.+-]+@[\w.-]+\.[a-z]{2,}\b/gi,'[email redacted]')
    .replace(/\b(?:password|passwd|token|api[-_]?key|authorization|secret|ssid)\b\s*[:=]\s*[^\r\n]+/gi,'[credential redacted]')
    .replace(/\bBearer\s+\S+/gi,'Bearer [redacted]')
    .replace(/\bgh[pousr]_[A-Za-z0-9]+\b/g,'[token redacted]')
    .replace(/```/g,"'''");
}
export function buildRunIssueUrl({product, firmware, source, state = {}}) {
  const issue=new URL('https://github.com/zhuzhe1983/esp-online-tester/issues/new');
  const title=redactIssueText(`[固件运行] ${product?.name||product?.id||'设备'} · ${firmware?.name||'自定义固件'}`).slice(0,150);
  issue.searchParams.set('title',title);
  let errors=redactIssueText(state.lastError?`${state.lastError.code||''}: ${state.lastError.message||''}`:'未捕获运行错误').slice(0,700);
  let logs=redactIssueText(Object.entries(state.serialByPort||{}).map(([port,text])=>`[${port}]\n${String(text).slice(-1600)}`).join('\n')||state.serial||'暂无串口日志').slice(-2400);
  let identity=redactIssueText([
    `设备：${product?.name||''} (${product?.id||''})`,
    `厂商 / 型号：${product?.manufacturer||''} / ${product?.sku||''}`,
    `芯片 / 板级模型：${product?.chip||''} / ${product?.runtime?.board||'尚未完成适配'}`,
    `固件：${firmware?.name||'自定义固件'} (${firmware?.id||''})`,
    `版本：${firmware?.revision||'未标注'}`,
    `固件来源：${source?.local||state.firmware?.source==='local'?'本地文件（不包含文件内容）':publicSource(source?.url||source?.manifestUrl)}`,
    `SHA-256：${source?.sha256||state.firmware?.manifestHash||state.firmware?.artifacts?.find(a=>['flash','app'].includes(a.kind))?.sha256||'未加载或未提供'}`,
    `状态 / 执行模式：${state.status||'未启动'} / ${state.firmware?.executionMode||'未启动'}`,
    `平台 API：${state.version||'未知'}`,
  ].join('\n')).slice(0,1800);
  const body=()=>`## 问题描述\n请补充：操作步骤、预期结果、实际现象。\n\n## 设备与固件\n${identity}\n\n## 报错\n${errors}\n\n## 最近日志（已截断并移除常见凭据和设备标识）\n\n\`\`\`text\n${logs}\n\`\`\`\n`;
  issue.searchParams.set('body',body());
  while(issue.href.length>7500&&logs.length){logs=logs.slice(Math.max(1,Math.ceil(logs.length*.2)));issue.searchParams.set('body',body());}
  // Non-ASCII names and error messages can also exhaust the encoded URL budget.
  while(issue.href.length>7500&&(identity.length>400||errors.length>80)){
    if(identity.length>400)identity=identity.slice(0,Math.max(400,Math.floor(identity.length*.8)));
    else errors=errors.slice(0,Math.max(80,Math.floor(errors.length*.8)));
    issue.searchParams.set('body',body());
  }
  return issue.href;
}
