"""Audit downloaded unmodified exact ESP images; binary distribution stays remote-only."""
from pathlib import Path
import argparse,importlib.util,json,hashlib,struct,datetime
ap=argparse.ArgumentParser();ap.add_argument('--sources',type=Path,required=True);a=ap.parse_args();root=Path(__file__).resolve().parents[3];S=a.sources/'s3-tail-sources';spec=importlib.util.spec_from_file_location('audit',root/'tools/audit-factory-firmware-sources.py');audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
choices=[
 ('touch28-v1','ESP32-S3-Touch-LCD-2.8','archive-members/ESP32-S3-Touch-LCD-2.8-Demo/Firmware/ESP32-S3-Touch-LCD-2.8.bin','example'),
 ('touch28-v2','ESP32-S3-Touch-LCD-2.8','firmware/ESP32-S3-Touch-LCD-2.8_FactoryOnly_20260523.bin','factory'),
 ('lcd28','ESP32-S3-LCD-2.8','firmware/ESP32-S3-LCD-2.8_FactoryOnly_20261120.bin','factory'),
 ('touch169','ESP32-S3-Touch-LCD-1.69','firmware/ESP32-S3-Touch-LCD-1.69_FactoryOnly_20230612.bin','factory'),
 ('touch7','ESP32-S3-Touch-LCD-7','firmware/ESP32-S3-Touch-LCD-7_FactoryOnly_20260625.bin','factory'),
 ('lcd7','ESP32-S3-Touch-LCD-7','firmware/lvgl_Porting_No_Touch.bin','example'),
 ('touch128','ESP32-S3-Touch-LCD-1.28','firmware/ESP32-S3-Touch-LCD-1.28_FactoryOnly_20251120.bin','factory'),
 ('lcd128','ESP32-S3-LCD-1.28','firmware/ESP32-S3-LCD-1.28_FactoryOnly_20250909.bin','factory')]
results=[]
for key,repo,name,role in choices:
 p=S/repo/'files'/name;b=p.read_bytes();r=json.loads(Path(str(p)+'.receipt.json').read_text());assert len(b)==r['bytes']and hashlib.sha256(b).hexdigest()==r['sha256'];assert audit.image(b,0)==9;parts=[];md5=False
 for at in range(0x8000,0x9000,32):
  magic=struct.unpack_from('<H',b,at)[0]
  if magic==0x50aa:
   _,kind,sub,off,size,label,flags=struct.unpack_from('<HBBII16sI',b,at);part={'type':kind,'subtype':sub,'offset':off,'bytes':size,'label':label.split(b'\0')[0].decode(),'flags':flags};assert off+size<=16<<20
   if kind==0 and off<len(b)and b[off]==0xe9:assert audit.image(b,off)==9;part.update(imageChipId=9,imageChecksumsValid=True)
   parts.append(part)
  elif magic==0xebeb:assert b[at+16:at+32]==hashlib.md5(b[0x8000:at]).digest();md5=True;break
  elif magic==0xffff:break
  else:raise ValueError((key,hex(magic)))
 assert any(p.get('imageChipId')==9 for p in parts);r.pop('containerFile',None);r.update(key=key,repo=repo,path=name,role=role,verificationTime=datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),result='source-layout-checksums-pass',executionVerified=False,layout={'loadOffset':0,'chipId':9,'bootloaderChecksumAndDigestValid':True,'partitionTableOffset':0x8000,'partitionTableMd5Valid':md5,'partitions':parts,'fitsFlashMiB':16,'headerFlashSizeCode':b[3]>>4});results.append(r)
(S/'factory-tail-verified.json').write_text(json.dumps(results,indent=2)+'\n');print([(r['key'],r['bytes'],r['result'])for r in results])
