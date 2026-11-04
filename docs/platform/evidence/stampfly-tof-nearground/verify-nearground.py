#!/usr/bin/env python3
"""Repeat the StampFly near-ground ST-driver check without downloads or repo edits."""
import argparse,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('source',type=Path);p.add_argument('--root',type=Path,default=Path.cwd());p.add_argument('--output',type=Path,required=True);a=p.parse_args();here=Path(__file__).resolve().parent;source=a.source/'lib/vl53l3c';files=sorted(x for x in source.iterdir()if x.suffix in('.c','.h'));digest=hashlib.sha256()
for x in files:digest.update(x.name.encode()+b'\0'+x.read_bytes()+b'\0')
assert len(files)==76 and digest.hexdigest()=='5fe2ab98f3879e6de905bf89a7d18233a8d23386c8202f6e25354d3c597fb49f','Require unmodified M5StampFly2b96752c ST source'
module=a.root/'esp-periph/src/i2c/vl53l3cx.rs';module_sha=hashlib.sha256(module.read_bytes()).hexdigest();assert module_sha=='7ae18780fc4708467a57c1b078d33666798bab3a8ae34e976d614a050e4671a8','Sensor model revision differs from tested input'
template=a.root/'docs/platform/evidence/stampfly-tof/wrapper.rs.in';wrapper=template.read_text().replace('@MODULE_PATH@',str(module.resolve()).replace('\\','\\\\').replace('"','\\"'));algorithms=[str(x)for x in files if x.suffix=='.c'and x.name not in{'vl53lx_platform.c','vl53lx_platform_init.c','vl53lx_platform_log.c'}]
with tempfile.TemporaryDirectory(prefix='stampfly-tof-near-')as scratch:
 work=Path(scratch);(work/'wrapper.rs').write_text(wrapper)
 subprocess.run(['rustc','--edition=2021','-C','codegen-units=2','--crate-type','staticlib',str(work/'wrapper.rs'),'-o',str(work/'libmodel.a')],check=True)
 subprocess.run(['cc','-O0','-g','-DSTDINT_H','-I'+str(source),str(here/'bottom-driver.c'),*algorithms,str(work/'libmodel.a'),'-lm','-o',str(work/'driver')],check=True)
 r=subprocess.run([str(work/'driver')],capture_output=True,check=True);assert not r.stderr;a.output.write_bytes(r.stdout)
print(json.dumps({'passed':True,'sourceAggregateSha256':digest.hexdigest(),'moduleSha256':module_sha,'harnessSha256':hashlib.sha256((here/'bottom-driver.c').read_bytes()).hexdigest(),'stdoutSha256':hashlib.sha256(r.stdout).hexdigest(),'sampleFrames':sum(line.startswith(b'sample,')for line in r.stdout.splitlines())}))
