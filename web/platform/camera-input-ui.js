// Explicit still-image sensor input. It never supplies a device framebuffer.
export function mountCameraInput({container,runtime,onError=()=>{}}){
  const panel=document.createElement('details'),summary=document.createElement('summary'),note=document.createElement('p'),input=document.createElement('input'),state=document.createElement('p');
  panel.hidden=true;input.disabled=true;panel.className='camera-input-controls';summary.textContent='摄像头图源';note.textContent='选择本地 BMP、PPM 或 baseline JPEG，由客体固件配置传感器后采集。图片仅在本次运行中使用，不进入分享或录制。';input.type='file';input.accept='.bmp,.ppm,.jpg,.jpeg';input.setAttribute('aria-label','选择摄像头图源');panel.append(summary,note,input,state);container.append(panel);let disposed=false,epoch=0;
  const refresh=async()=>{const request=++epoch;try{const cap=await runtime.capabilities(),snapshot=await runtime.observe();if(disposed||request!==epoch)return;panel.hidden=!cap.camera?.hostPicture;input.disabled=!['ready','running','paused'].includes(snapshot.status);input.accept=cap.camera?.formats?.includes('baseline-jpeg')?'.bmp,.ppm,.jpg,.jpeg':'.bmp,.ppm';}catch(e){if(!disposed)onError(e);}};
  input.addEventListener('change',async()=>{const file=input.files?.[0];if(!file)return;input.disabled=true;try{if(file.size>8*1024*1024)throw new Error('图源不能超过 8 MiB。');const result=await runtime.setCameraPicture({bytes:await file.arrayBuffer()});if(!disposed)state.textContent=`已设置 ${result.bytes} 字节图源；是否采集以客体输出为准。`;}catch(e){if(!disposed){state.textContent=e.message;onError(e);}}finally{input.value='';await refresh();}});
  const off=runtime.subscribe(event=>{if(event.type==='state')void refresh();});void refresh();
  return {dispose(){disposed=true;epoch++;off();panel.remove();}};
}
