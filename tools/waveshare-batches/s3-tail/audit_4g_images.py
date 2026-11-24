"""Check exact original precompiled 4G examples without changing their bytes or role."""
from pathlib import Path
import argparse, importlib.util, json, hashlib, struct, datetime
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--sources',type=Path,required=True);a=ap.parse_args()
root=Path(__file__).resolve().parents[3];S=a.sources/'s3-tail-4g-sources'
spec=importlib.util.spec_from_file_location('audit',root/'tools/audit-factory-firmware-sources.py');audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
result=[]
for key,stem in [('a7670e','ESP32-S3-A7670E_USB_4G_CDC'),('sim7670g','ESP32-S3-SIM7670G_USB_4G_CDC')]:
 p=next((S/'members'/stem).rglob('*.bin'));b=p.read_bytes();r=json.loads(Path(str(p)+'.receipt.json').read_text())
 assert len(b)==r['bytes'] and hashlib.sha256(b).hexdigest()==r['sha256'] and len(b)<=16<<20
 assert audit.image(b,0)==9;parts=[];md5=False
 for at in range(0x8000,0x9000,32):
  magic=struct.unpack_from('<H',b,at)[0]
  if magic==0x50aa:
   _,kind,sub,off,size,label,flags=struct.unpack_from('<HBBII16sI',b,at);part={'type':kind,'subtype':sub,'offset':off,'bytes':size,'label':label.split(b'\0')[0].decode(),'flags':flags};assert off+size<=16<<20
   if kind==0 and off<len(b) and b[off]==0xe9:assert audit.image(b,off)==9;part.update(imageChipId=9,imageChecksumsValid=True)
   parts.append(part)
  elif magic==0xebeb:assert b[at+16:at+32]==hashlib.md5(b[0x8000:at]).digest();md5=True;break
  elif magic==0xffff:break
  else:raise ValueError((key,hex(magic)))
 assert any(x.get('imageChipId')==9 for x in parts)
 r.pop('containerFile',None);r.update(key=key,repo='ESP32-S3-'+key.upper()+'-4G',path=str(p.relative_to(S)),role='example',
   verificationTime=datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),result='source-layout-checksums-pass',executionVerified=False,
   roleBasis='Manufacturer Firmware page calls these precompiled example programs; no exact factory-purpose image found. Firmware pages explicitly prohibit mixing modem models.',
   layout={'loadOffset':0,'chipId':9,'bootloaderChecksumAndDigestValid':True,'partitionTableOffset':0x8000,'partitionTableMd5Valid':md5,'partitions':parts,'fitsFlashMiB':16,'headerFlashSizeCode':b[3]>>4})
 result.append(r)
(S/'examples-verified.json').write_text(json.dumps(result,indent=2)+'\n')
print([(r['key'],r['bytes'],r['sha256']) for r in result])
