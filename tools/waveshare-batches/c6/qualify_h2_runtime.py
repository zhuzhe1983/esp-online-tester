"""Advance exact H2 source fragments after the independent native runtime proof."""
from pathlib import Path
import datetime,json
ROOT=Path(__file__).resolve().parents[3];OUT=ROOT/'tools/waveshare-batches/c6';E='docs/evidence/waveshare-h2-sram-2026-11-23';ARCH='docs/platform/research/waveshare-h2-runtime-sources';manifest=json.loads((ROOT/ARCH/'manifest.json').read_text());now=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def save(p,d):p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
notes=['部分适配：独立 ESP32-H2 SRAM 运行时已经原生验证 BOOT GPIO9、UART0 和 GPIO8 的 RMT RGB；完整原厂应用尚未通过。','Only explicit SRAM app-direct loading is supported. ROM, flash-cache/MMU, interrupt routing, USB, Bluetooth and IEEE802.15.4 remain unimplemented. No C6 SoC/ROM alias.','Hardware flash remains4MiB with noPSRAM. Header capacities1/2/4MiB fit; larger headers fail. Wafer revision is unverified, so full min=0 and max=0/65535 are required; deprecated byte14 is ignored.','Native evidence: '+E+'/native-execution.ndjson and native-tests.log. Authored fixture is diagnostic only, never manufacturer firmware or a default public catalogue image.']
replacements={
 '运行时尚不可用：真实ESP32-H2，当前仅来源、型号、镜像布局和模型覆盖；不能计作可运行适配。':notes[0],
 'No H2 image executed. True chip16 source/layout only.':'No original manufacturer H2 image executed. Authored SRAM diagnostic executes independently on both exact H2 boards; native evidence is separate from original firmware acceptance.',
 'Initial batch runtime is unavailable; this listing is source/model coverage, not a runtime-adapted completion. Independent H2 minimal SRAM/GPIO/UART work is tracked separately.':'The initial source/model batch had no H2 runtime. Independent H2 SRAM/GPIO/UART/RMT native execution is now verified by core5972cabf; full original applications remain unaccepted.',
 ' · 官方示例（仅参考，H2运行时未接入）':' · 官方示例（需完整启动支持，仅参考）'
}
def rewrite(v):
 if isinstance(v,str):
  for a,b in replacements.items():v=v.replace(a,b)
  return v
 if isinstance(v,list):return[rewrite(x)for x in v]
 if isinstance(v,dict):return{k:rewrite(x)for k,x in v.items()}
 return v
sel=[r for r in manifest['sources'] if r['originalPath'] in['esp32-h2_technical_reference_manual_en.pdf','esp32-h2_datasheet_en.pdf','files/components/hal/esp32h2/include/hal/rmt_ll.h','files/components/bootloader_support/include/esp_app_format.h','files/components/bootloader_support/src/bootloader_common_loader.c']]
for listing,board in[('esp32-h2-zero','waveshare-h2-zero'),('esp32-h2-dev-kit-n4','waveshare-h2-dev-kit-n4')]:
 path=OUT/(listing+'.json');d=rewrite(json.loads(path.read_text()))
 for p in d['products']:
  p['runtime'].update(board=board,status='partial',flashMb=4,psramMb=0,touch=False);p['limitations']=list(dict.fromkeys(notes+p['limitations']))
 for k in d['knowledge']:
  k['updatedAt']=now;k['collectionNotes']=list(dict.fromkeys(notes+k['collectionNotes']))
  for i,r in enumerate(sel):
   sid='h2-silicon-'+str(i);s={'id':sid,'title':r['originalPath'],'url':r['url'],'kind':'silicon-primary-source','status':'fetched','resolvedUrl':r['resolvedUrl'],'fetchedAt':datetime.datetime.fromisoformat(r['fetchedAt'].replace('Z','+00:00')).isoformat(timespec='seconds').replace('+00:00','Z'),'bytes':r['bytes'],'sha256':r['sha256'],'archiveName':r['archive'],'notes':'Original bytes retained asgzip at '+ARCH+'/'+r['archive']}
   k['sources']=[x for x in k['sources'] if x['id']!=sid]+[s]
  k['facts']=[f for f in k['facts']if f['id']!='h2-silicon-map']+[{'id':'h2-silicon-map','label':'H2 独立芯片内存和时钟来源','value':{'chipId':16,'hpSram':{'start':'0x40800000','endExclusive':'0x40850000','bytes':327680},'lpSram':{'start':'0x50000000','endExclusive':'0x50001000','bytes':4096},'maximumCpuHz':96000000,'boardCrystalHz':32000000,'rmtBase':'0x60007000','gpioBase':'0x60091000','ioMuxBase':'0x60090000','physicalPads':'GPIO0–5,8–14,22–27;19pads across28register positions','romBytes':131072},'basis':'official','sourceIds':['h2-silicon-'+str(i)for i in range(len(sel))],'notes':'Hardware facts from retained Espressif sources. Implemented/tested subset is described separately in collection notes and '+E+'/README.md; source identity alone is not execution acceptance.'}]
  save(ROOT/'web/platform/knowledge'/(k['id']+'.json'),k)
 for fw in d['firmwares']:
  assert fw['status']=='reference'and fw['source']is None
  fw['limitations']=list(dict.fromkeys(fw['limitations']+['原厂18个完整示例均保留原始字节；真实loader拒绝和芯片/分区/校验见'+E+'/original-image-audit.json。独立SRAM诊断通过不代表这些原厂应用可运行。']))
 d['boardCapabilities']={board:{'chip':'esp32h2','flashMb':4,'psramMb':0,'sdCard':False,'boardButtons':[]}}
 d['tests']=list(dict.fromkeys(d['tests']+['cargo test -p esp32h2','cargo clippy -p esp32h2 --all-targets -- -D warnings',E+'/native-execution.ndjson',E+'/original-image-audit.json']))
 d['sourceReceiptPaths']=list(dict.fromkeys(d['sourceReceiptPaths']+[ARCH+'/manifest.json']))
 d['limitations']=notes+['Exact original source/image bytes retained; neither full original application nor physical hardware acceptance is claimed.']
 save(path,d)
 print(listing,board,len(d['products']),'exact products now have independent partial H2 runtime')
