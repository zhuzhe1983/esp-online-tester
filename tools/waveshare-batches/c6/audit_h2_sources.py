"""Retain exact H2 primary sources; audit original full images without modifying them."""
from pathlib import Path
import argparse,datetime,gzip,hashlib,importlib.util,json,re,struct,subprocess
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--sources',type=Path,required=True);ap.add_argument('--probe',type=Path,required=True);a=ap.parse_args()
ROOT=Path(__file__).resolve().parents[3];SRC=a.sources/'h2-runtime-sources';ARCH=ROOT/'docs/platform/research/waveshare-h2-runtime-sources';E=ROOT/'docs/evidence/waveshare-h2-sram-2026-11-23';ARCH.mkdir(parents=True,exist_ok=True);E.mkdir(parents=True,exist_ok=True)
now=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,d):p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
receipts=[]
for f in sorted(SRC.rglob('*')):
 if not f.is_file() or f.name.endswith('.receipt.json') or not Path(str(f)+'.receipt.json').exists():continue
 b=f.read_bytes();rec=json.loads(Path(str(f)+'.receipt.json').read_text());assert len(b)==rec['bytes'] and sha(b)==rec['sha256'];name=re.sub(r'[^A-Za-z0-9._-]','_',str(f.relative_to(SRC)))+'.gz';gz=gzip.compress(b,mtime=0);(ARCH/name).write_bytes(gz);receipts.append({**rec,'originalPath':str(f.relative_to(SRC)),'archive':name,'archiveSha256':sha(gz)})
revision=json.loads((SRC/'revision.json').read_text());save(ARCH/'manifest.json',{'schemaVersion':'1','createdAt':now,'repository':'espressif/esp-idf','revision':revision,'sources':receipts,'scope':'Original downloaded bytes compressed without rewriting; TRM/datasheet are current versioned source documents, IDF paths are fixed commit. No hardware wafer revision is inferred.'})
spec=importlib.util.spec_from_file_location('audit',ROOT/'tools/audit-factory-firmware-sources.py');audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
def header(data,off):
 h=data[off:off+24];assert len(h)==24 and h[0]==0xe9;pos=off+24;segments=[]
 for _ in range(h[1]):
  addr,n=struct.unpack_from('<II',data,pos);pos+=8;segments.append({'loadAddress':f'0x{addr:08x}','bytes':n});pos+=n
 return {'chipId':int.from_bytes(h[12:14],'little'),'flashSizeCode':h[3]>>4,'entry':f'0x{int.from_bytes(h[4:8],"little"):08x}','legacyRevisionByteIgnoredByIdf':h[14],'minimumRevisionFull':int.from_bytes(h[15:17],'little'),'maximumRevisionFull':int.from_bytes(h[17:19],'little'),'segments':segments,'xorAndDigestValid':audit.image(data,off)==16}
rows=[];base=a.sources/'c6-sources/ESP32-H2-Zero/files'
for p in sorted((base/'firmware').rglob('*.bin')):
 data=p.read_bytes();rec=json.loads(Path(str(p)+'.receipt.json').read_text());assert len(data)==rec['bytes'] and sha(data)==rec['sha256'];boot=header(data,0);parts=[];md5=False
 for pos in range(0x8000,0x9000,32):
  magic=int.from_bytes(data[pos:pos+2],'little')
  if magic==0x50aa:
   _,kind,sub,off,n,label,flags=struct.unpack_from('<HBBII16sI',data,pos);part={'type':kind,'subtype':sub,'offset':off,'bytes':n,'label':label.split(b'\0')[0].decode(),'flags':flags};assert off+n<=4<<20
   if kind==0 and off<len(data) and data[off]==0xe9:
    part['header']=header(data,off);part['nativeSramLoader']=json.loads(subprocess.check_output([str(a.probe),'--check-image',str(p),str(off)],text=True));assert not part['nativeSramLoader']['accepted'];assert part['header']['chipId']==16
   parts.append(part)
  elif magic==0xebeb:
   assert hashlib.md5(data[0x8000:pos]).digest()==data[pos+16:pos+32];md5=True;break
  elif magic==0xffff:break
  else:raise ValueError((str(p),pos,magic))
 assert md5 and any('header' in x for x in parts)
 boot_result=json.loads(subprocess.check_output([str(a.probe),'--check-image',str(p),'0'],text=True));assert not boot_result['accepted']
 rows.append({'originalPath':str(p.relative_to(base)),**rec,'chip':'esp32h2','bootloader':boot,'partitionTableOffset':32768,'partitionMd5Valid':md5,'partitions':parts,'nativeWholeImageLoader':boot_result,'modified':False,'executed':False,'result':'source-identity-pass; full-image execution unsupported by bounded SRAM runtime'})
save(E/'original-image-audit.json',{'schemaVersion':'1','createdAt':now,'repository':'waveshareteam/ESP32-H2-Zero','sourceImages':len(rows),'scope':'Original manufacturer example images retained unmodified. Full header/checksum/partition audit and actual native SRAM loader rejection; not execution acceptance.','images':rows})
print('H2',len(receipts),'primary source receipts;',len(rows),'unmodified image identities and app rejections verified')
