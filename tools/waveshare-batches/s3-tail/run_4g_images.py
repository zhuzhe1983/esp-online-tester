"""Run two unmodified, exact-model 4G examples through ROM for a bounded interval."""
from pathlib import Path
import argparse,gzip,hashlib,json,subprocess,time
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
ap=argparse.ArgumentParser(description=__doc__)
for name in ['sources','binary','rom','sd','evidence']:ap.add_argument('--'+name,type=Path,required=True)
ap.add_argument('--guest-ms',type=int,default=5000);a=ap.parse_args();S=a.sources/'s3-tail-4g-sources';manifest=S/'examples-verified.json'
a.evidence.mkdir(parents=True,exist_ok=True)
inputs={'binarySha256':sha(a.binary),'romSha256':sha(a.rom),'imageAuditSha256':sha(manifest),'initialSdSha256':sha(a.sd),'initialSdBytes':a.sd.stat().st_size,
        'scope':'Native ROM execution only. Exact 16 MiB Flash / 8 MiB PSRAM. USB modem, cellular/GNSS network, camera and fuel gauge are not implemented; no original application or hardware acceptance.'}
(a.evidence/'native-run-inputs.json').write_text(json.dumps(inputs,indent=2)+'\n');runs=[]
for r in json.loads(manifest.read_text()):
 board='waveshare-s3-'+r['key']+'-4g-v2';fw=S/r['path'];assert sha(fw)==r['sha256'];out=a.evidence/board;out.mkdir(exist_ok=True)
 cmd=[str(a.binary.resolve()),board,str(a.rom.resolve()),str(fw.resolve()),'16','8',str(a.guest_ms),str(out.resolve()),str(a.sd.resolve())]
 start=time.monotonic();p=subprocess.run(cmd,capture_output=True,text=True,timeout=240)
 row={'board':board,'firmwareKey':r['key'],'firmwareSha256':r['sha256'],'role':'example','flashMiB':16,'psramMiB':8,'guestBoundMs':a.guest_ms,
      'defaultFat32MiB':64,'exitCode':p.returncode,'hostSeconds':round(time.monotonic()-start,3),'stdout':p.stdout,'stderr':p.stderr,
      'state':json.loads((out/'state.json').read_text()) if (out/'state.json').exists() else None}
 uart=out/'uart.txt'
 if uart.exists():
  raw=uart.read_bytes();(out/'uart.original.gz').write_bytes(gzip.compress(raw,mtime=0))
  row['uartNormalization']={'originalBytes':len(raw),'originalSha256':hashlib.sha256(raw).hexdigest(),'rule':'Original retained gzip; text view normalizes CRLF/CR and line-end whitespace.'}
  uart.write_bytes(b'\n'.join(x.rstrip() for x in raw.replace(b'\r\n',b'\n').replace(b'\r',b'').split(b'\n')).rstrip()+b'\n')
 (out/'frame.rgb565').unlink(missing_ok=True)
 row['artifacts']={f.name:{'bytes':f.stat().st_size,'sha256':sha(f)} for f in sorted(out.iterdir()) if f.is_file()};runs.append(row)
 (a.evidence/'native-runs.json').write_text(json.dumps(runs,indent=2)+'\n');print(board,row['state'],flush=True)
