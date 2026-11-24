#!/usr/bin/env python3
"""Verify unmodified original core-family firmware ZIP members; retain hashes only."""
from pathlib import Path
import argparse,hashlib,importlib.util,json,struct,zipfile
ROOT=Path(__file__).resolve().parents[3]
p=argparse.ArgumentParser();p.add_argument('--sources',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
spec=importlib.util.spec_from_file_location('image_audit',ROOT/'tools/audit-factory-firmware-sources.py');audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
def sha(b):return hashlib.sha256(b).hexdigest()
def partitions(b,off=0):
 rows=[];pos=off
 while b[pos:pos+2]==b'\xaa\x50':
  _,kind,sub,start,size,label,flags=struct.unpack_from('<HBBII16sI',b,pos);rows.append({'type':kind,'subtype':sub,'offset':start,'bytes':size,'label':label.split(b'\0')[0].decode('ascii','replace'),'flags':flags});pos+=32
 assert rows
 md5=b[pos:pos+2]==b'\xeb\xeb'
 if md5:assert b[pos+16:pos+32]==hashlib.md5(b[off:pos]).digest()
 return {'partitions':rows,'md5Present':md5,'md5Passed':md5,'partitionEnd':max(x['offset']+x['bytes']for x in rows)}
base=json.loads((ROOT/'docs/evidence/waveshare-s3-core-2026-11-23/firmware-sources.json').read_text());out=[]
for item in base:
 if item['role']=='rejected-wrong-chip':continue
 name=Path(item['container']['path']).name;paths=list(args.sources.glob('*/'+name));assert len(paths)==1,(name,paths);path=paths[0];raw=path.read_bytes();assert sha(raw)==item['container']['sha256'];z=zipfile.ZipFile(path);segments=[]
 for seg in item['segments']:
  entry=seg['archive']['entry'];b=z.read(entry);assert sha(b)==seg['sha256'] and len(b)==seg['bytes'];record={'entry':entry,'offset':seg['offset'],'bytes':len(b),'sha256':sha(b)}
  if seg['offset']==0x8000:record['layout']=partitions(b)
  else:
   assert audit.image(b,0)==9;record['chipId']=9;record['segmentChecksumsAndAppendedDigest']='passed'
   if b[0x8000:0x8002]==b'\xaa\x50':
    record['layout']=partitions(b,0x8000)
    for app in record['layout']['partitions']:
     if app['type']==0 and app['offset']<len(b):assert audit.image(b,app['offset'])==9
  segments.append(record)
 out.append({'id':item['id'],'container':{**json.loads(path.with_name(path.name+'.source.json').read_text()),'fileName':name},'segments':segments,'status':'passed','originalBytesChanged':False,'executionInheritedByNewSkus':False})
# Both N16R8 images are real original package members. The OCT PSRAM build
# matches the documented8MiB octal PSRAM; neither is labeled factory firmware.
path=args.sources/'esp32-s3-dev-kit-n8r8/ESP32-S3-DEV-KIT-N8R8-micropython-firmware.zip';raw=path.read_bytes();receipt=json.loads(path.with_name(path.name+'.source.json').read_text());assert sha(raw)==receipt['sha256'];z=zipfile.ZipFile(path)
for entry in z.namelist():
 if 'N16R8/'not in entry or not entry.endswith('.bin'):continue
 b=z.read(entry);assert audit.image(b,0)==9;layout=partitions(b,0x8000);assert layout['partitionEnd']<=16<<20;apps=[]
 for part in layout['partitions']:
  if part['type']==0 and part['offset']<len(b):assert audit.image(b,part['offset'])==9;apps.append(part['offset'])
 assert apps
 out.append({'id':'dev-n16r8-oct'if'SPIRAM_OCT'in entry else'dev-n16r8-other','container':{**receipt,'fileName':path.name},'entry':entry,'bytes':len(b),'sha256':sha(b),'flashHeaderMiB':2**(b[3]>>4),'flashMode':b[2],'chipId':9,'applicationOffsets':apps,'layout':layout,'status':'passed','originalBytesChanged':False,'executed':False,'purpose':'Manufacturer-linked development firmware, not identified factory preload; OCT denotes octalPSRAM and does not make this QSPIFlash module OPIFlash.'})
zero=args.sources/'esp32-s3-zero/Esp32-s3-zero-code.zip'
with zipfile.ZipFile(zero)as z:
 b=z.read('BlinkRGB/flash_download_tool_3.9.4/combine/target.bin');chip=audit.image(b,0);assert chip==5;out.append({'id':'zero-rejected-combine','entry':'BlinkRGB/flash_download_tool_3.9.4/combine/target.bin','sha256':sha(b),'bytes':len(b),'chipId':chip,'status':'rejected','reason':'Image chip5 ESP32-C3 conflicts with targetS3 chip9. Original three validS3 files remain separate, never rebuild or merge.'})
args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(out,indent=2)+'\n');print(f'{len(out)-1} source image sets passed; 1 original wrong-chip combine rejected; no firmware executed or rewritten.')
