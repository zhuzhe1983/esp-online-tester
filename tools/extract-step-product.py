#!/usr/bin/env python3
"""Extract one explicitly named STEP product without changing its geometry.

Large EasyEDA exports contain hundreds of thousands of silkscreen/track entities.
This bounded subgraph export selects a named PCB product, its representation,
native-unit context and all referenced topology. It does not infer other parts,
merge assemblies, change coordinates, or manufacture a simplified shape.
The original source and the selected product name remain in the model receipt.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path

def extract(raw, suffix):
    text=raw.decode('utf-8');data=text.index('\nDATA;')+len('\nDATA;')
    entities={int(m[1]):m[2] for m in re.finditer(r'^#(\d+)\s*=\s*([\s\S]*?);',text[data:],re.M)}
    def refs(value):return [int(v)for v in re.findall(r'#(\d+)',re.sub(r"'(?:''|[^'])*'",'',value))]
    products=[(i,re.match(r"PRODUCT\('((?:''|[^'])*)'",v)[1])for i,v in entities.items()if re.match(r"PRODUCT\('((?:''|[^'])*)'",v)]
    matching=[(i,name)for i,name in products if name.endswith(suffix)]
    if len(matching)!=1:raise ValueError('Expected one exact product suffix match: '+str(matching))
    product,name=matching[0];current={product}
    for kind in ['PRODUCT_DEFINITION_FORMATION','PRODUCT_DEFINITION','PRODUCT_DEFINITION_SHAPE','SHAPE_DEFINITION_REPRESENTATION']:
        current={i for i,v in entities.items() if re.match(kind+r'\(',v)and current.intersection(refs(v))}
        if len(current)!=1:raise ValueError('Ambiguous STEP relationship '+kind+': '+str(current))
    roots=sorted(current);pending=roots[:];keep=set()
    while pending:
        i=pending.pop()
        if i in keep:continue
        if i not in entities:raise ValueError('Missing STEP entity #'+str(i))
        keep.add(i);pending.extend(refs(entities[i]))
    protocol=[i for i,v in entities.items()if v.startswith('APPLICATION_PROTOCOL_DEFINITION(')]
    for i in protocol:
        if all(j in keep for j in refs(entities[i])):keep.add(i)
    output=(text[:data]+'\n'+'\n'.join('#'+str(i)+' = '+entities[i]+';'for i in sorted(keep))+'\nENDSEC;\nEND-ISO-10303-21;\n').encode('utf-8')
    receipt={'method':'STEP named-product dependency subgraph; geometry entities unchanged','product':name,'productEntity':product,'rootEntities':roots,'retainedEntities':len(keep),'originalEntities':len(entities),'sha256':hashlib.sha256(output).hexdigest(),'bytes':len(output),'coordinateScope':'Selected product local coordinates and native unit context; no assembly placement inferred.'}
    return output,receipt

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('source',type=Path);p.add_argument('output',type=Path);p.add_argument('--product-suffix',required=True);a=p.parse_args();out,receipt=extract(a.source.read_bytes(),a.product_suffix);a.output.write_bytes(out);print(json.dumps(receipt,ensure_ascii=False))
