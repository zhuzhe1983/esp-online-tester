// The same SD controls are mounted in the workbench and the mini page.
export function createSdCardUi({mount, getApi, blocked, releaseInputs, onError}) {
  const node = (tag, text) => { const value = document.createElement(tag); if (text) value.textContent = text; return value; };
  const root = node('section'); root.className = 'sd-card-details'; root.hidden = true;
  const heading = node('h3', '虚拟 TF 卡'), status = node('p'); status.className = 'helper'; status.setAttribute('role','status');
  const note = node('p', '默认插入 64 MiB FAT32 卡。数据按设备和固件分别保存在当前浏览器。导入磁盘镜像或清空会停止模拟器，完成后可再次启动。'); note.className = 'helper';
  const controls = node('div'); controls.className = 'session-tool-row';
  const makeButton = (text, action) => { const value = node('button', text); value.type = 'button'; value.className = 'button quiet tiny'; value.dataset.action = action; return value; };
  const importButton = makeButton('导入 TF 卡镜像', 'import-sd-card'), exportButton = makeButton('导出 TF 卡镜像', 'export-sd-card'), clearButton = makeButton('清空为 FAT32', 'clear-sd-card');
  const file = node('input'); file.type = 'file'; file.accept = '.img,.bin,.ima'; file.hidden = true;
  const confirmation = node('div'); confirmation.hidden = true;
  const confirm = makeButton('确认清空 TF 卡', 'confirm-clear-sd-card'), cancel = makeButton('取消', 'cancel-clear-sd-card');
  confirmation.append(node('p', '将删除当前设备与固件的虚拟 TF 卡内容，并创建空白 FAT32 卡。'), confirm, cancel);
  controls.append(importButton, exportButton, clearButton, file); root.append(heading, status, note, controls, confirmation); mount.append(root);
  let state = null, busy = false, active = true;
  const render = () => {
    root.hidden = !state?.supported;
    const labels = {unloaded:'加载固件后自动插卡',loading:'正在恢复 TF 卡…',saving:'正在保存 TF 卡…',saved:'TF 卡已保存',clean:state?.restored?'已恢复 TF 卡':'TF 卡已就绪','memory-only':'浏览器持久化不可用，TF 卡仅保留在本页',conflict:'其他页面已修改 TF 卡，请重新加载固件',error:'TF 卡保存失败，请勿关闭页面'};
    status.textContent = busy ? '正在处理 TF 卡…' : `${labels[state?.status] || state?.status || '尚未插卡'}${state?.bytes ? ` · ${(state.bytes / 1024 / 1024).toFixed(0)} MiB · ${state.format === 'fat32' ? 'FAT32' : '导入镜像'}` : ''}`;
    const disabled = busy || blocked() || !state?.present || ['loading','saving','conflict'].includes(state?.status);
    for (const control of [importButton,exportButton,clearButton,confirm]) control.disabled = disabled;
    cancel.disabled = busy;
  };
  async function operation(run) {
    if (busy) return; busy = true; render();
    try { await releaseInputs(); await run(); state = (await getApi().observe()).sdCard || null; confirmation.hidden = true; }
    catch (error) { onError(error); }
    finally { busy = false; if (active) render(); }
  }
  importButton.addEventListener('click', () => file.click());
  file.addEventListener('change', () => {
    const selected = file.files?.[0]; file.value = ''; if (!selected) return;
    void operation(async () => {
      if (selected.size > 128 * 1024 * 1024 || selected.size === 0) throw Error('请选择不超过 128 MiB 的完整 TF 卡磁盘镜像。');
      await getApi().attachSdCard({bytes: new Uint8Array(await selected.arrayBuffer())});
    });
  });
  exportButton.addEventListener('click', () => { void operation(async () => {
    const result = await getApi().exportSdCard(), url = URL.createObjectURL(new Blob([result.bytes], {type:'application/octet-stream'}));
    const anchor = node('a'); anchor.href = url; anchor.download = result.filename || 'tf-card.img'; document.body.append(anchor); anchor.click(); anchor.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }); });
  clearButton.addEventListener('click', () => { confirmation.hidden = false; });
  cancel.addEventListener('click', () => { confirmation.hidden = true; });
  confirm.addEventListener('click', () => { void operation(() => getApi().clearSdCard()); });
  const unsubscribe = getApi()?.subscribe(event => {
    if (event.type === 'sd-card') { state = event.sdCard; render(); }
    else if (event.type === 'state') { state = event.state.sdCard || null; confirmation.hidden = true; render(); }
  });
  Promise.resolve(getApi()?.observe()).then(value => { if (active) { state = value?.sdCard || null; render(); } }).catch(onError);
  return {refresh:render,dispose(){ active=false; unsubscribe?.();root.remove(); }};
}
