// A failed module dependency prevents app.js from installing its own handlers.
// Keep this entry point dependency-free so loading failures still have an exit.
try {
  await import('./app.js');
} catch(cause) {
  const message='页面脚本加载失败，请刷新页面重试。';
  const error={code:'PAGE_LOAD_FAILED',message:cause?.message||message};
  try {
    const {showErrorDialog}=await import('./error-ui.js');
    showErrorDialog({error,message,kind:'page'});
  } catch {
    // If the dialog's modules also failed, basic native HTML still works.
    const dialog=document.createElement('dialog');dialog.id='workbench-error';dialog.className='firmware-run-confirm workbench-error';dialog.setAttribute('aria-labelledby','workbench-error-title');
    const heading=document.createElement('h2');heading.id='workbench-error-title';heading.textContent='页面发生错误';
    const detail=document.createElement('p');detail.textContent=message;
    const actions=document.createElement('div');actions.className='dialog-actions';
    const close=document.createElement('button');close.className='button';close.textContent='关闭';close.addEventListener('click',()=>dialog.close());
    const report=document.createElement('a');report.id='report-runtime-issue';report.className='button primary';report.textContent='报告问题 ↗';report.target='_blank';report.rel='noopener noreferrer';
    const url=new URL('https://github.com/zhuzhe1983/esp-online-tester/issues/new');url.searchParams.set('title','[页面加载] PAGE_LOAD_FAILED');url.searchParams.set('body','页面脚本或依赖未能加载。请补充浏览器版本、操作步骤和网络情况。');report.href=url.href;
    actions.append(close,report);dialog.append(heading,detail,actions);document.body.append(dialog);dialog.addEventListener('close',()=>dialog.remove(),{once:true});dialog.showModal();close.focus();
  }
}
