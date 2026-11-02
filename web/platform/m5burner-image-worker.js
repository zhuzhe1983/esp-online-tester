import {captchaTextMasks,matchInstructionTargets} from './m5burner-image-points.js';

// A dedicated worker owns Tesseract's nested worker, so cancellation also stops
// initialization and CPU work. No provider token is sent to this worker tree.
self.onmessage=async({data:challenge})=>{
  let worker,image;
  const progress=message=>self.postMessage({type:'progress',message});
  try {
    progress('正在准备验证…');
    const {default:{createWorker}}=await import('./vendor/tesseract/tesseract.esm.min.js');
    const base=new URL('./vendor/tesseract/',import.meta.url).href;
    // Without errorHandler, Tesseract also throws rejected jobs from its message
    // handler. Keep failures in this attempt instead of a page-level dialog.
    worker=await createWorker('eng',1,{workerPath:base+'worker.min.js',corePath:base,langPath:base,workerBlobURL:false,cacheMethod:'none',errorHandler:()=>{}});
    image=await createImageBitmap(await (await fetch(challenge.captchaImage)).blob());
    if(image.width!==challenge.width||image.height!==challenge.height)throw Error('Image dimensions changed');
    const canvas=new OffscreenCanvas(challenge.width,challenge.height),ctx=canvas.getContext('2d',{willReadFrequently:true});ctx.drawImage(image,0,0);
    const masks=captchaTextMasks(ctx.getImageData(0,0,canvas.width,canvas.height)),candidates=[];
    for(let i=0;i<masks.length;i++){
      progress(`正在处理图片 ${i+1}/${masks.length}…`);const mask=masks[i],crop=new OffscreenCanvas(mask.width,mask.height);
      crop.getContext('2d').putImageData(new ImageData(mask.data,mask.width,mask.height),0,0);
      const bytes=new Uint8Array(await (await crop.convertToBlob({type:'image/png'})).arrayBuffer());let best={text:'',confidence:0};
      for(const mode of ['7','13']){await worker.setParameters({tessedit_pageseg_mode:mode,tessedit_char_whitelist:'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'});const {data}=await worker.recognize(bytes);if(data.confidence>best.confidence)best=data;}
      candidates.push({text:best.text.trim(),confidence:best.confidence,x:mask.x,y:mask.y});
    }
    self.postMessage({type:'result',result:matchInstructionTargets(challenge.instruction,candidates)});
  } catch {self.postMessage({type:'error'});}
  finally {image?.close();await worker?.terminate().catch(()=>{});}
};
