import {mountFirmwareHub} from './m5burner-ui.js';
import {createLilygoSparkClient,matchSparkDevice,sparkFirmware,sparkHistoryFirmware,LILYGO_SPARK_HOME} from './lilygo-spark.js';
import {extractFirmwareZip,firmwareArchiveCacheKey,MAX_FIRMWARE_ZIP_BYTES} from './firmware-archive.js';
import {firmwareCache} from './firmware-cache.js';
import {sha256} from './runtime.js';
export function mountLilygoSpark(options) {
  if(!options.mount)return null;
  const recovery=document.createElement('div'),note=document.createElement('p'),link=document.createElement('a'),label=document.createElement('label'),input=document.createElement('input'),message=document.createElement('p');
  recovery.hidden=true;note.className=message.className='helper';message.setAttribute('role','status');
  note.textContent='若原站不允许浏览器直接下载，可下载原始 ZIP 后在这里导入，无需手动解压。';
  link.textContent='下载原始 ZIP ↗';link.className='text-link';link.target='_blank';link.rel='noopener noreferrer';
  label.textContent='导入已下载的 ZIP / BIN';label.className='field-label';input.type='file';input.accept='.zip,.bin,application/zip,application/octet-stream';input.setAttribute('aria-label','导入当前 Spark 版本的 ZIP 或 BIN');label.append(input);recovery.append(note,link,label,message);
  let hub,locked=false,reading=false,lastSource='';
  const refresh=()=>{const source=hub?.current()?.source,identity=JSON.stringify(source);recovery.hidden=!source?.archive;if(source?.archive)link.href=source.url;input.disabled=locked||reading;if(identity!==lastSource){lastSource=identity;message.textContent='';input.value='';}};
  hub=mountFirmwareHub({...options,onChange(){refresh();options.onChange?.();},client:options.client||createLilygoSparkClient(),provider:{name:'LILYGO Spark',prefix:'spark',detailsLabel:'查看项目来源 ↗',home:LILYGO_SPARK_HOME,match:matchSparkDevice,firmware:sparkFirmware,history:sparkHistoryFirmware}});
  options.mount.append(recovery);
  input.addEventListener('change',async()=>{
    const source=hub.current()?.source,file=input.files[0],identity=JSON.stringify(source);if(!source?.archive||!file||reading)return;
    reading=true;refresh();message.textContent='正在解压并校验所选版本…';
    try {
      if(file.size<1||file.size>MAX_FIRMWARE_ZIP_BYTES)throw Error('请选择不超过 64 MiB 的 ZIP 或 BIN。');
      let data=new Uint8Array(await file.arrayBuffer());
      if(/\.zip$/i.test(file.name)||data[0]===0x50&&data[1]===0x4b)data=await extractFirmwareZip(data,source);
      if(await sha256(data)!==source.sha256)throw Error('文件与当前所选版本的 SHA-256 不一致。');
      if(identity!==JSON.stringify(hub.current()?.source))return;
      await firmwareCache.putVerified(firmwareArchiveCacheKey(source),source.sha256,data);
      if(identity===JSON.stringify(hub.current()?.source))message.textContent='已校验并缓存当前版本。点击“下载并尝试运行”即可继续；芯片和启动布局仍会自动检查。';
    }catch(error){if(identity===JSON.stringify(hub.current()?.source))message.textContent=error.message;}
    finally{reading=false;refresh();}
  });
  return {...hub,setDisabled(value){locked=value;hub.setDisabled(value);refresh();},setProduct(value){hub.setProduct(value);refresh();}};
}
