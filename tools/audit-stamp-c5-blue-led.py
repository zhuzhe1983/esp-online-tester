#!/usr/bin/env python3
"""Audit the exact Stamp C5 D1 native emitting face; no CAD package or network needed.

The pinned embedded STEP keeps designators above the reused 0402 LED solid.
This checks that electrical D1 is the right-hand native component 32, not D2,
and that only two existing top triangles receive runtime output index 0.
"""
import argparse,hashlib,json,re,struct
from pathlib import Path

SOURCE_SHA='69f7f228edbcb001c3111e3f8f36f557222916b48473a9523fcee178d83b51d1'

def digest(data):return hashlib.sha256(data).hexdigest()
def refs(value):return [int(x)for x in re.findall(r'#(\d+)',value)]
def point(value):return [float(x)for x in re.search(r'\(\s*([-+0-9.E, ]+)\)\s*\)',value).group(1).split(',')]
def check(product,root):
 assets=root/'web/platform/assets'/product;model=json.loads((assets/'model.json').read_text());plan=json.loads((assets/'model-plan.json').read_text());source=(assets/'source.step').read_bytes();assert digest(source)==SOURCE_SHA
 entities={int(i):v for i,v in re.findall(r'#(\d+)\s*=\s*(.*?);',source.decode(),re.S)}
 # Exact PRODUCT→definition→placement and reused LED-definition relationships.
 assert entities[39277].startswith("PRODUCT('D1Stamp_C5_PCB_main_V0.3_20260127_0940'")
 assert refs(entities[39276])==[39277] and refs(entities[39275])==[39276,39279]
 assert refs(entities[39273])==[39274,39280] and refs(entities[39274])==[39275]
 assert refs(entities[39280])[:2]==[11,39281] and refs(entities[39281])==[39282,39283,39284]
 assert refs(entities[39294])==[39275,39144] and refs(entities[39153])==[37832,39144]
 assert entities[39146].startswith("PRODUCT('User Library-0402(1005M) LED'")
 assert entities[37834].startswith("PRODUCT('D2Stamp_C5_PCB_main_V0.3_20260127_0940'")
 d1=point(entities[39282]);d2=point(entities[37839]);matrix=model['sourceTransform']['matrixRows'];center=[sum(matrix[i][j]*d1[j]for j in range(3))+matrix[i][3]for i in range(3)]
 led=next(p for p in model['parts']if p['name']=='status-blue-led');body=next(p for p in model['parts']if p['name']=='cad-part-32');red=next(p for p in model['parts']if p['sourceComponent']==19)
 assert led['sourceComponent']==body['sourceComponent']==32 and led['ledIndex']==0
 assert 'ledIndex'not in body and 'ledIndex'not in red
 assert led['sourceFaceRanges']==[[248,250]] and body['sourceFaceRanges']==[[0,248],[250,1616]]
 assert len([p for p in model['parts']if'ledIndex'in p])==1
 assert not any('ledIndex'in p for p in model['derivedParts'])
 binary=(assets/'geometry.bin').read_bytes();data=binary[led['byteOffset']:led['byteOffset']+led['vertexCount']*24];assert len(data)==144
 triangles=[struct.unpack_from('<18f',data,i)for i in [0,72]];vertices=[t[j:j+3]for t in triangles for j in [0,6,12]];lo=[min(v[i]for v in vertices)for i in range(3)];hi=[max(v[i]for v in vertices)for i in range(3)]
 assert abs((lo[0]+hi[0])/2-center[0])<1e-5 and abs((lo[1]+hi[1])/2-center[1])<1e-5
 assert abs(hi[0]-lo[0]-.43)<1e-5 and abs(hi[1]-lo[1]-.78)<1e-5 and abs(lo[2]-(center[2]+.45))<1e-6
 assert all(t[j]>.98 for t in triangles for j in [5,11,17])
 assert red['boundsMm'][1][0]<0<lo[0]
 # The exact plan must recreate the metadata binding; an output-only JSON edit is insufficient.
 binding=next(p for p in plan['parts']if p['name']=='status-blue-led');assert binding['sourceFaceRanges']==led['sourceFaceRanges'] and binding['ledIndex']==0
 return {'productId':product,'sourceSha256':SOURCE_SHA,'modelSha256':digest((assets/'model.json').read_bytes()),'geometrySha256':digest(binary),'sourceGraph':{'d1Product':39277,'d1Definition':39275,'d1Placement':39282,'sharedLedProduct':39146,'sharedLedDefinition':39144,'d2Product':37834,'d2Placement':37839},'nativeD1CenterMm':d1,'nativeD2CenterMm':d2,'modelD1CenterMm':center,'emittingFace':{'sourceComponent':32,'sourceFaceRanges':[[248,250]],'triangles':2,'boundsMm':[lo,hi],'sizeMm':[hi[i]-lo[i]for i in range(3)],'ledIndex':0},'nonEmittingD1Triangles':1614,'d2HasNoBinding':True,'electricalContract':'Official D1 BLUE_LED from VSYS_3V3 via R1=1k to G28_BOOT; active-low. Runtime index 0=[0,0,255] only for enabled/routed guest GPIO28 LOW; otherwise [0,0,0].','evidenceLimits':'CAD package top used as visual emitting surface; actual LED die/photometry/beam pattern not measured. This source audit does not execute guest firmware.'}

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1]);p.add_argument('--output',type=Path);a=p.parse_args();results=[check(s,a.root)for s in ['m5stack-s016','m5stack-s016-dip']]
 assert results[0]['geometrySha256']==results[1]['geometrySha256']
 if a.output:a.output.mkdir(parents=True,exist_ok=True);[(a.output/(r['productId']+'-blue-led-source.json')).write_text(json.dumps(r,indent=2)+'\n')for r in results]
 print(json.dumps({'products':[r['productId']for r in results],'passed':True,'emittingTrianglesPerProduct':2,'chargingLedExcluded':True}))
if __name__=='__main__':main()
