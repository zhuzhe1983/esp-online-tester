import {buildRunIssueUrl,redactIssueText} from './firmware-run-ui.js';
import {burnerProjectUrl} from './firmware-download-access.js';

export function expectedCancellation(error) {
  return ['FETCH_ABORTED','FLASH_CANCELLED','RECOVERY_CANCELLED','SESSION_REPLACED'].includes(error?.code)||error?.name==='AbortError';
}
let active=null;
export function showErrorDialog({error,message,context={},kind='operation'}={}) {
  if(expectedCancellation(error))return;
  const key=`${error?.code||''}:${message}`;
  if(active?.key===key)return;
  const node=(tag,text,className)=>{const el=document.createElement(tag);if(text)el.textContent=text;if(className)el.className=className;return el;};
  const previousFocus=active?.previousFocus||document.activeElement;
  if(active){active.dialog.remove();active=null;}
  const dialog=node('dialog',null,'firmware-run-confirm workbench-error');dialog.id='workbench-error';dialog.setAttribute('aria-labelledby','workbench-error-title');dialog.setAttribute('aria-describedby','workbench-error-message');
  const title=node('h2',error?.code==='FIRMWARE_AUTH_REQUIRED'?'固件下载需要验证':kind==='startup'?'设备启动失败':kind==='page'?'页面发生错误':'操作未完成');title.id='workbench-error-title';
  const detail=node('p',message||'发生未知错误，请查看错误详情。');detail.id='workbench-error-message';
  const contextLabel=node('p',context.product?.name,'helper');
  const diagnostics=node('details',null,'error-diagnostics'),summary=node('summary','错误详情');
  const code=node('pre',redactIssueText(`${error?.code||'UI_ERROR'}\n${error?.message||message||''}`).slice(0,3000));code.setAttribute('data-i18n-skip','');diagnostics.append(summary,code);
  const note=node('p','报告会打开 GitHub 草稿，附带设备、固件和经脱敏的日志；请检查后自行提交。','helper');
  const actions=node('div',null,'dialog-actions'),close=node('button','返回工作台','button'),report=node('a','报告问题 ↗','button primary');close.type='button';
  report.id='report-runtime-issue';report.target='_blank';report.rel='noopener noreferrer';report.href=buildRunIssueUrl({...context,state:{...context.state,lastError:error}});
  const officialUrl=burnerProjectUrl(error?.details?.url);
  if(officialUrl&&['FIRMWARE_AUTH_REQUIRED','FIRMWARE_DOWNLOAD_RESPONSE'].includes(error?.code)){
    const official=node('a','前往 M5Burner 验证并下载 ↗','error-source-link');official.href=officialUrl;official.target='_blank';official.rel='noopener noreferrer';dialog.append(official);
  }
  actions.append(close,report);dialog.prepend(title,contextLabel,detail);dialog.append(diagnostics,note,actions);
  close.addEventListener('click',()=>dialog.close());
  dialog.addEventListener('close',()=>{
    if(active?.dialog===dialog)active=null;
    dialog.remove();if(previousFocus?.isConnected)previousFocus.focus();
  },{once:true});
  document.body.append(dialog);active={key,dialog,previousFocus};dialog.showModal();close.focus();
}
