import {currentLocale} from './i18n.js';
// Observations from the board model only; a rendered frame is not hardware validation.
const text = (value, limit) => typeof value === 'string' && value.length <= limit;
const integer = (value, max) => Number.isSafeInteger(value) && value >= 0 && value <= max;
export function parseHardwareDiagnostics(value) {
  if(value?.board==='corematrix') {
    const s=value.state,m=s?.matrix,b=s?.buzzer;
    if(value.schemaVersion!==1||!s||!m||!b||!['controllerPowered','grove5vEnabled','sdPresent'].every(key=>typeof s[key]==='boolean')||
      !['powered','enabled'].every(key=>typeof m[key]==='boolean')||!integer(m.pwmSteps,16)||!integer(m.frames,Number.MAX_SAFE_INTEGER)||!integer(m.bytesWritten,Number.MAX_SAFE_INTEGER)||
      typeof b.supplyEnabled!=='boolean'||!integer(b.frequencyHz,0xffff_ffff)||!integer(b.duty4095,4095))return null;
    return {schemaVersion:1,board:'corematrix',state:{controllerPowered:s.controllerPowered,
      matrix:{powered:m.powered,enabled:m.enabled,pwmSteps:m.pwmSteps,frames:m.frames,bytesWritten:m.bytesWritten},
      buzzer:{frequencyHz:b.frequencyHz,duty4095:b.duty4095,supplyEnabled:b.supplyEnabled},grove5vEnabled:s.grove5vEnabled,sdPresent:s.sdPresent}};
  }
  const state=value?.state, display=state?.display;
  if(value?.schemaVersion!==1 || value.board!=='stopwatch' || !state || !display ||
    !['powerLedEnabled','downloadLocked'].every(key=>typeof state[key]==='boolean') ||
    !integer(state.pm1ConfiguredHz,0xffff_ffff) || !(state.pm1LastBusHz===null || integer(state.pm1LastBusHz,0xffff_ffff)) ||
    !['sleepOut','displayOn','visible'].every(key=>typeof display[key]==='boolean') ||
    !(display.brightnessControlEnabled===null || typeof display.brightnessControlEnabled==='boolean') || !integer(display.brightness,255) ||
    !Array.isArray(value.findings) || value.findings.length>16 || !Array.isArray(value.limits) || value.limits.length>8 ||
    !value.limits.every(item=>text(item,512)))return null;
  const codes=new Set();
  for(const finding of value.findings) {
    if(!finding || !text(finding.code,64) || !/^[A-Z][A-Z0-9_]*$/.test(finding.code) || codes.has(finding.code) ||
      !['warning','info'].includes(finding.severity) || !integer(finding.count,Number.MAX_SAFE_INTEGER) ||
      !text(finding.summary,200) || !text(finding.detail,1536))return null;
    codes.add(finding.code);
  }
  // Whitelist + copies prevent subscribers and callers from mutating retained state.
  return {schemaVersion:1,board:'stopwatch',state:{powerLedEnabled:state.powerLedEnabled,downloadLocked:state.downloadLocked,
    pm1ConfiguredHz:state.pm1ConfiguredHz,pm1LastBusHz:state.pm1LastBusHz,display:{sleepOut:display.sleepOut,displayOn:display.displayOn,
      brightness:display.brightness,brightnessControlEnabled:display.brightnessControlEnabled,visible:display.visible}},
    findings:value.findings.map(({code,severity,count,summary,detail})=>({code,severity,count,summary,detail})),limits:[...value.limits]};
}

const labels={
  PM1_I2C_CLOCK_MISMATCH:['电源芯片与主机的 I²C 速率不匹配','先使用电源芯片支持的速率通信；需要提速时，核对两端配置及初始化顺序。'],
  PM1_DOWNLOAD_LOCK:['固件设置了下载锁','核对 M5PM1 的 DL_LOCK 设置，它会影响实机长按进入下载模式。'],
  PM1_DOWNLOAD_UNLOCK_UNVERIFIED:['固件尝试清除下载锁，解锁效果尚未验证','寄存器写入不能证明实机已经恢复长按下载；请核对电源芯片的锁定与解锁行为。'],
  PM1_POWER_LED_DISABLED:['固件关闭了电源指示灯','关闭指示灯可以是正常设计，不代表设备已经断电。'],
  CO5300_PIXELS_WHILE_HIDDEN:['显存有像素，但屏幕当前不可见','提前写入显存可以是正常操作。请核对供电、复位、退出休眠和显示开启命令，不能仅凭像素写入判断实机已亮屏。'],
  CO5300_PIXEL_FORMAT:['检测到 RGB565 以外的像素格式','核对像素格式设置与写入数据是否一致；当前模型仅覆盖 RGB565 的像素输出。'],
  CO5300_BRIGHTNESS_CONTROL:['屏幕亮度控制需要核对','核对亮度控制和亮度设置。参考资料中的默认值有差异，不能据此断言实机一定黑屏。'],
};

export function mountHardwareDiagnostics({container,runtime}) {
  const doc=container.ownerDocument;
  const element=(tag,className,content)=>{const node=doc.createElement(tag);if(className)node.className=className;if(content!==undefined)node.textContent=content;return node;};
  container.classList.add('hardware-diagnostics');container.hidden=true;
  const details=element('details'),summary=element('summary'),title=element('strong','', '硬件检查');
  const status=element('span','hardware-check-status'),hint=element('span','hardware-check-hint'),body=element('div','hardware-check-body');
  status.setAttribute('role','status');summary.append(title,status,hint);details.append(summary,body);container.append(details);
  let revision=0,last='',currentBoard=null;
  function update(snapshot,board) {
    currentBoard=board;const supported=['stopwatch','corematrix'].includes(board);container.hidden=!supported;
    title.textContent=board==='corematrix'?'硬件状态':'硬件检查';
    const signature=JSON.stringify([board,snapshot]);if(signature===last)return;last=signature;
    if(!supported || !snapshot) {
      details.open=false;container.classList.remove('has-warning');status.textContent='等待固件运行';hint.textContent='';
      body.replaceChildren(element('p','hardware-check-limit','仅覆盖已建模检查，仍需实机验证。'));return;
    }
    if(board==='corematrix') {
      const {matrix,buzzer,...state}=snapshot.state,rows=element('dl','hardware-check-state');
      container.classList.remove('has-warning');status.textContent=state.controllerPowered?'运行供电已开启':'主控已断电';hint.textContent='';
      for(const [name,value] of [['LED 点阵供电',matrix.powered?'已开启':'已关闭'],['LED 点阵显示',matrix.enabled?'已开启':'休眠'],
        ['点阵亮度',`${matrix.pwmSteps} / 16`],['点阵更新次数',String(matrix.frames)],
        ['蜂鸣器供电',buzzer.supplyEnabled?'已开启':'已关闭'],['蜂鸣器频率',`${buzzer.frequencyHz} Hz`],['PWM 占空比',`${(100*buzzer.duty4095/4095).toFixed(1)}%`],
        ['Grove 5V',state.grove5vEnabled?'已开启':'已关闭'],['microSD',state.sdPresent?'已插入':'未插入']])rows.append(element('dt','',name),element('dd','',value));
      body.replaceChildren(rows,element('p','hardware-check-limit','状态来自固件控制的模拟器件。'));return;
    }
    const warnings=snapshot.findings.filter(item=>item.severity==='warning');container.classList.toggle('has-warning',warnings.length>0);
    const statusText=warnings.length?`${warnings.length} 项需检查`:'当前未发现已覆盖的问题',hintText=warnings.length?(labels[warnings[0].code]?.[0]||warnings[0].summary):'';
    if(status.textContent!==statusText)status.textContent=statusText;
    if(hint.textContent!==hintText)hint.textContent=hintText;
    const state=snapshot.state,display=state.display,rows=element('dl','hardware-check-state');
    const hz=value=>value===null?'未知':`${(value/1000).toLocaleString(currentLocale(),{maximumFractionDigits:2})} kHz`;
    for(const [name,value] of [['电源灯',state.powerLedEnabled?'已启用':'已关闭'],['下载锁',state.downloadLocked?'已锁定':'未锁定'],
      ['I²C 主机 / 电源芯片',`${hz(state.pm1LastBusHz)} / ${hz(state.pm1ConfiguredHz)}`],
      ['屏幕状态',display.visible?'可见':'不可见'],['退出休眠 / 显示开启',`${display.sleepOut?'是':'否'} / ${display.displayOn?'是':'否'}`],
      ['亮度 / 亮度控制',`${display.brightness} / ${display.brightnessControlEnabled===null?'未明确配置':display.brightnessControlEnabled?'已启用':'未启用'}`]]) {
      rows.append(element('dt','',name),element('dd','',value));
    }
    const findings=element('ul','hardware-check-findings');
    for(const finding of [...warnings,...snapshot.findings.filter(item=>item.severity!=='warning')]) {
      const item=element('li',`hardware-check-finding ${finding.severity}`),label=labels[finding.code];
      item.append(element('strong','',label?.[0]||finding.summary));
      if(label)item.append(element('p','',label[1]));
      item.append(element('p','hardware-check-evidence',finding.detail),element('code','',`${finding.code} · ${finding.count} 次`));findings.append(item);
    }
    const limits=element('div','hardware-check-limits');limits.append(element('p','hardware-check-limit','仅覆盖已建模检查，仍需实机验证。'));
    for(const limit of snapshot.limits)limits.append(element('p','hardware-check-evidence',limit));
    body.replaceChildren(rows,findings,limits);
  }
  const unsubscribe=runtime.subscribe(event=>{
    if(event.type==='state'){revision++;update(event.state.hardwareDiagnostics,event.state.product?.runtime?.board);}
    else if(event.type==='hardware-diagnostics'){revision++;update(event.hardwareDiagnostics,event.hardwareDiagnostics?.board||currentBoard);}
  });
  const initial=revision;void runtime.observe().then(state=>{if(initial===revision)update(state.hardwareDiagnostics,state.product?.runtime?.board);});
  return {dispose(){revision++;unsubscribe();container.replaceChildren();}};
}
