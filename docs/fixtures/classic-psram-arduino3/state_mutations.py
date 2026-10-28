#!/usr/bin/env python3
"""Semantic Arduino-state mutations in a caller-owned stock installation only.
Usage: state_mutations.py TREE RECIPE_OUTPUT EVIDENCE
Requires CARGO_TARGET_DIR and ESP32SIM_ROM_DIR. Restores original source always.
These are guest-library mutants, not new emulator behavior or accepted firmware.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
repo, sdk, out = (Path(p).resolve() for p in sys.argv[1:])
assert os.environ.get('CARGO_TARGET_DIR') and os.environ.get('ESP32SIM_ROM_DIR')
out.mkdir(parents=True, exist_ok=True)
source = sdk/'arduino-A30/data/packages/esp32/hardware/esp32/3.0.7/cores/esp32/esp32-hal-psram.c'
original = source.read_bytes()
cases = [
 ('failed-reprobe', b'if (spiramFailed) {', b'if (false && spiramFailed) {', 'external_arduino3_no_device_is_rejected', 'cached Arduino retry and SDK duplicate entry counts'),
 ('detected-duplicate-false', b'if (spiramDetected) {\n    return true;', b'if (spiramDetected) {\n    return false;', 'external_arduino3_psram_startup_and_readback', 'successful Arduino duplicate'),
 ('failed-retry-true', b'if (spiramFailed) {\n    return false;', b'if (spiramFailed) {\n    return true;', 'external_arduino3_no_device_is_rejected', 'preserves failed state on retry'),
 ('failed-add-true', b'log_e("PSRAM not initialized!");\n    return false;', b'log_e("PSRAM not initialized!");\n    return true;', 'external_arduino3_no_device_is_rejected', 'heap-add must reject undetected PSRAM'),
]
rows=[]
try:
 for name, old, new, test, assertion in cases:
  assert original.count(old)==1
  source.write_bytes(original.replace(old,new))
  build=out/('build-'+name); fw=out/name
  cmd=['nice','-n','10','arduino-cli','--config-file',str(sdk/'arduino-A30/config.yaml'),'compile','-j4','--fqbn','esp32:esp32:esp32cam','--build-path',str(build),'--output-dir',str(fw),str(repo/'docs/fixtures/classic-psram-arduino3/p6_probe')]
  with (out/(name+'-build.log')).open('w') as log:
   p=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT)
  assert p.returncode==0,'compile errors are not kills'
  env=os.environ.copy();env['ARDUINO3_FIRMWARE_DIR']=str(fw)
  cmd=['nice','-n','10','cargo','+1.99.0','test','--release','-j4','-p','esp32sim','--test','classic_psram_arduino3',test,'--','--ignored','--exact','--nocapture']
  p=subprocess.run(cmd,cwd=repo,env=env,capture_output=True,text=True)
  text=p.stdout+p.stderr;(out/(name+'.log')).write_text(text)
  killed=p.returncode==101 and assertion in text and 'panicked at cli/tests/classic_psram_arduino3.rs:' in text and 'could not compile' not in text
  rows.append(dict(mutation=name,assertion=assertion,assertion_killed=killed));(out/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
  print(name,killed,flush=True)
  shutil.rmtree(build)
finally:
 source.write_bytes(original)
assert all(r['assertion_killed'] for r in rows)
