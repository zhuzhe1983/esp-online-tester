#!/usr/bin/env python3
"""Verify the exact published source files against the retained checked snapshot mapping."""
import argparse,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--checkout',type=Path,required=True);a=p.parse_args()
r=a.checkout.resolve();m=r/'docs/evidence/c5-final-integration-2026-11-25/source-identity.json'
d=json.loads(m.read_text());count=0
for row in d['files']:
    q=Path(row['path']);assert not q.is_absolute() and '..' not in q.parts
    f=(r/q).resolve();assert f.is_relative_to(r)
    b=f.read_bytes();assert len(b)==row['bytes'] and hashlib.sha256(b).hexdigest()==row['sha256'],str(q)
    count+=1
print(json.dumps({'verifiedSourceFiles':count,'baseline':d['basePublicMain'],'checkedLocalSource':d['validatedLocalCheckpoint'],'declaredPostValidationDifferences':len(d['declaredPostValidationDifferences'])}))
