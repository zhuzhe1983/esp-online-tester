#!/usr/bin/env python3
"""Package this project's built firmware; never downloads or substitutes vendor firmware."""
import argparse,hashlib,json,subprocess,shutil
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('build',type=Path);p.add_argument('destination',type=Path,help='web/wasm/fw or an independent staging directory');p.add_argument('--source-commit',required=True);a=p.parse_args()
project=Path(__file__).resolve().parent;repo=project.parents[1]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def git(*args):return subprocess.check_output(['git','-C',str(repo),*args])
commit=git('rev-parse',a.source_commit+'^{commit}').decode().strip()
source_files=git('ls-tree','-r','--name-only',commit,'firmware/stampfly-uart-demo').decode().splitlines()
if not source_files:raise SystemExit('Source commit does not contain this project')
for name in source_files:
 if git('show',commit+':'+name)!=(repo/name).read_bytes():raise SystemExit('Source differs from named commit: '+name)
merged=a.build/'stampfly-uart-demo.bin';data=merged.read_bytes()
if data[0]!=0xe9 or data[0x10000]!=0xe9 or data[12:14]!=b'\x09\x00':raise SystemExit('Expected complete S3 image at 0x0/0x10000')
if b'/Users/' in data or b'/home/' in data:raise SystemExit('Private build path found in distributable image')
public=a.destination/'public';public.mkdir(parents=True,exist_ok=True)
name='stampfly-uart-demo-v0.1.0';shutil.copyfile(merged,public/(name+'.bin'));(public/(name+'-licenses.txt')).write_text((project/'LICENSES.txt').read_text().expandtabs(4))
artifacts={key:{'bytes':(a.build/path).stat().st_size,'sha256':sha(a.build/path)} for key,path in {'merged':'stampfly-uart-demo.bin','application':'stampfly_uart_demo.bin','bootloader':'bootloader/bootloader.bin','partitionTable':'partition_table/partition-table.bin'}.items()}
record={'name':'StampFly UART Demo','version':'0.1.0','sourceCommit':commit,'sourceRepository':'https://github.com/zhuzhe1983/esp-online-tester','sourceSha256':{name:sha(repo/name) for name in source_files},'artifacts':artifacts,'build':{'sdk':'ESP-IDF v5.5.1','compiler':'xtensa-esp-elf esp-14.2.0_20241119 GCC14.2.0','esptool':'4.12.0','configuration':'sdkconfig.defaults plus IDF defaults','command':'firmware/stampfly-uart-demo/build.sh <private-build> <pinned-M5StampFly-source>','layout':{'bootloader':'0x0','partitionTable':'0x8000','application':'0x10000'},'flashBytes':8388608,'psramBytes':0},'dependencies':[{'project':'m5stack/M5StampFly','revision':'2b96752c72c465c63b260996046c71c7675f81b2','path':'lib/vl53l3c','aggregateSha256':'5fe2ab98f3879e6de905bf89a7d18233a8d23386c8202f6e25354d3c597fb49f','verification':'76 unmodified .c/.h files, sorted name + NUL + bytes + NUL; caller supplied source, no claimed new download','license':'BSD-3-Clause option of ST dual license'},{'project':'bosch-sensortec/BMI270_SensorAPI','revision':'41129fcfe39c583ee5462d79195741945d51c1fe','path':'bmi270.c configuration array','license':'BSD-3-Clause'}],'hardwareValidated':False,'identity':'Independent UART-controlled demonstration; neither factory firmware nor the internal onboarding diagnostic','licenseFile':name+'-licenses.txt'}
(public/(name+'-source.json')).write_text(json.dumps(record,indent=2)+'\n')
for board,suffix,sku in [('stampfly','','K138'),('stampfly-v11','-v11','K138-V11')]:
 manifest={'name':'stampfly_uart_demo','version':'0.1.0','board':board,'flash_mb':8,'psram_mb':0,'files':{'flash':{'url':'public/'+name+'.bin','sha256':artifacts['merged']['sha256']}},'description':'Independent UART-controlled StampFly demonstration, '+sku+'; normal ROM boot, motors initially zero; physics requires explicit user configuration.','source':'public/'+name+'-source.json','license':'public/'+name+'-licenses.txt'}
 (a.destination/('stampfly-uart-demo'+suffix+'.json')).write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'sourceCommit':commit,'artifacts':artifacts,'manifestSha256':{q.name:sha(q) for q in sorted(a.destination.glob('stampfly-uart-demo*.json'))}},indent=2))
