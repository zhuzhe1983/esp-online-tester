#!/usr/bin/env python3
"""Validate the caller-supplied, unmodified ST source before compiling it."""
import hashlib,sys
from pathlib import Path
source=Path(sys.argv[1])/'lib/vl53l3c'
files=sorted(p for p in source.iterdir() if p.suffix in ('.c','.h'))
h=hashlib.sha256()
for p in files:h.update(p.name.encode()+b'\0'+p.read_bytes()+b'\0')
if len(files)!=76 or h.hexdigest()!='5fe2ab98f3879e6de905bf89a7d18233a8d23386c8202f6e25354d3c597fb49f':raise SystemExit('Require unmodified m5stack/M5StampFly 2b96752c72c465c63b260996046c71c7675f81b2 lib/vl53l3c')
print('ST source verified:',h.hexdigest())
