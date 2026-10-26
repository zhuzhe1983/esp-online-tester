#!/usr/bin/env python3
"""Produce the lane02 per-SKU port review; never mutates catalog/model assets."""
import hashlib,json,math
from pathlib import Path
from datetime import datetime,timezone
ROOT=Path(__file__).resolve().parents[1]
RUN=ROOT/'.butler/runs/20261027-010438-m5-hardware-83-esp32-esp-product/submissions'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def rotation(v):
 x,y,z=v; a,b,c=math.cos(x),math.sin(x),math.cos(y);d,e,f=math.sin(y),math.cos(z),math.sin(z)
 return [[c*e,-c*f,d],[a*f+b*d*e,a*e-b*d*f,-b*c],[b*f-a*d*e,b*e+a*d*f,a*c]]
def vec(m,i):return [round(m[j][i],6)for j in range(3)]
def axis(v):
 i=max(range(3),key=lambda i:abs(v[i]));return ('+'if v[i]>=0 else'-')+'XYZ'[i]
corrections={
 'm5stack-c149':['Rejected original bottom-center cable position and inherited wire order. Explicit side mouth and back-view mirrored wire order retained. Close-up fit is visual only; hidden plug dimensions remain estimates.'],
 'm5stack-k132':['USB/Grove local width corrected to worldY for mouths on ±X; keyboard photo includes only masked white paper, not keys/case.'],
 'm5stack-k152':['USB/Grove local width corrected to worldY for mouths on ±X.'],
 'm5stack-k128':['Rejected grove-a on -Y; exact side image and CAD cutout put it on -X next to USB.'],
 'm5stack-k151':['Rejected socket rotation with local width alongZ; exact side photo requires width alongY.'],
 'm5stack-k130-v11':['Rejected upside-down rear preview (+Y USB) and missing Grove pair. Same-source CAD and exact SKU rear image establish -Y USB, rear Ared/Bblack; oldS3 paper text was not reused for S3A.'],
 'm5stack-k134':['Rejected single mirrored Grove draft. Exact rear photograph has two white sockets and two-pin green power terminal. Back camera up=-Y preserves physicalX; widths are alongY, outward normal-Z.'],
 'm5stack-k001':['Source glass/LCD was separated3.294mm from frame. Explicit estimated rigid seating corrected; missing battery-base CAD remains a dimension/coverage failure.'],
 'm5stack-c148':['Rejected optional battery as recolorable shell. C148 without battery omits source component6; native assembly oriented to match exact front image.'],
}
missing={
 'm5stack-k034':['Waterproof cable/gland/contact geometry is missing. Internal extension Grove ports are not exterior case sockets; no cable connection drawn.'],
 'm5stack-k138':['Whole drone source CAD is unavailable except battery-protection piece. Exposed USB and two Grove connectors are recorded in product metadata but not modeled as verified ports; no cable connection drawn.'],
 'm5stack-k001':['Matching bottom battery-base geometry and its expansion-pin detail are unavailable.'],
 'm5stack-s007-pin127':['Native STEP USB mechanical surfaces included. Header pitch is1.27mm; no2.54mm header geometry from sibling SKU is reused.'],
 'm5stack-s007-pin254-v033':['S3A cover geometry is distinct from older StampS3 STEP. Do not infer old board/USB/headers as exact hardware revision.'],
 'm5stack-s013':['Native STEP includes USB and BTB mechanical geometry; no fabricated PCB or shell. Copper stackup, exact user revision and electrical behavior remain unknown.'],
}
records=[]
for filename in ['lane-02.json','lane-02-extra.json']:
 for product in json.loads((RUN/filename).read_text())['products']:
  id=product['id'];assets=ROOT/'web/platform/assets'/id;plan=json.loads((assets/'model-plan.json').read_text());model=json.loads((assets/'model.json').read_text());know=json.loads((ROOT/'web/platform/knowledge'/f'{id}.json').read_text());sources={s['id']:s for s in know['sources']};ports=[]
  for part in plan.get('derivedParts',[]):
   if part.get('kind')!='connector-visual':continue
   geo=part['geometry'];r=rotation(geo.get('rotation',[0,0,0]));normal=vec(r,2);width=vec(r,0)
   ports.append({'id':part['id'],'type':part['connectorType'],'mouthCenterMm':geo['centerMm'],'outwardNormal':normal,'face':axis(normal),'localWidthAxis':width,'widthDirection':axis(width),'housingColor':part.get('housingColor'),'basis':part['basis'],'sourceIds':part['sourceIds'],'sourceUrls':[sources[s]['url']for s in part['sourceIds']],'allowConnections':part.get('allowConnections')is True,'pinOrder':part.get('pinOrder'),'matingEvidence':part.get('matingEvidence',{'status':'unverified','notes':'Visual insert only; no physical mating certification.'}),'notes':part.get('notes','')})
  record={'id':id,'name':product['name'],'status':'reviewed-partial','modelSha256':sha(assets/'model.json'),'planSha256':sha(assets/'model-plan.json'),'modeledVisualSocketCount':len(ports),'modeledGroveSocketCount':sum(p['type']=='HY2.0-4P'for p in ports),'nativeMechanicalPorts':(['USB-C','MIPI CSI BTB','SDIO BTB']if id=='m5stack-s013'else['USB-C']if id=='m5stack-s007-pin127'else[]),'declaredInterfaces':plan.get('catalog',{}).get('interfaces',product['interfaces']),'ports':ports,'negativeCorrections':corrections.get(id,[]),'remainingGaps':missing.get(id,[]),'sourceDocument':next(s['url']for s in know['sources']if s['id']=='official-docs')}
  records.append(record)
rejected=[{'id':'m5stack-k128','previousMouthMm':[12,-26.8,6],'previousNormal':[0,-1,0],'reason':'Wrong enclosure face; corrected to -X.'},{'id':'m5stack-k151','previousRotation':[0,-math.pi/2,0],'reason':'Width axis was worldZ instead of worldY.'},{'id':'m5stack-k130-v11','previousGlobalRotation':[[1,0,0],[0,-1,0],[0,0,-1]],'previousUsbMouthMm':[0,24,-11],'previousGroveCount':0,'reason':'Rear pose reversed; missing sockets.'},{'id':'m5stack-k134','previousGroveCount':1,'previousMouthMm':[-16.7,5,-12.8],'reason':'Single socket and mirrored rear coordinates; actual two ports.'},{'id':'m5stack-k001','previousGlassFrameGapMm':3.294048309326172,'reason':'Separated source display assembly had been called assembled.'},{'id':'m5stack-c148','omittedOptionalBatteryComponent':6,'reason':'Battery envelope is not the C148 bare-product shell.'}]
report={'schemaVersion':'1','reviewedAt':datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),'scope':'Original27+extra12 exact-SKU product previews; all remain partial. Port counts refer to modeled sockets, not a declaration that missing geometry does not exist on the physical device.','method':['Each visual socket lists its local mouth, outward normal and transformed pin-width axis. Exact SKU source photograph/drawing citations remain separate from generic insert dimensions.','Only entries with allowConnections=true and an explicit four-pin order can draw a cable. Others are visible-port previews only. No inherited default wire order accepted.','Plastic housing color is a photo-informed approximate display value, independent from shell recoloring and metal/inner-cavity materials.','Native manufacturer USB/BTB surfaces are listed separately from estimated socket inserts. Missing ports are explicit gaps.','Visual close-ups cannot establish electrical routing, current limits, mating tolerances or waterproofing.'],'rejectedDraftConfigurations':rejected,'products':records}
out=ROOT/'docs/platform/evidence/lane02-ports.json';out.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
lines=['# Lane02 端口复核','', '原27款与新增12款均保留 **partial** 状态。表中数量为已建模插口；没有插口模型不代表实物没有该接口。厂家板级CAD中的USB/BTB另列。每个口的坐标、朝外法向、口宽方向、颜色、来源和线序见 [结构化记录](evidence/lane02-ports.json)。','', '| 型号 | 插口模型 / Grove | 颜色与口位（面；口宽轴） | 连接线 | 缺项 / 否定旧草案 |','|---|---:|---|---|---|']
for r in records:
 desc='；'.join(f"{p['id']} {p.get('housingColor')or 'metal'} {p['face']}/{p['widthDirection']}"for p in r['ports'])or('厂家CAD：'+', '.join(r['nativeMechanicalPorts'])if r['nativeMechanicalPorts']else'未建插口模型')
 cable='；'.join(p['id']+':'+','.join(p['pinOrder']or[])for p in r['ports']if p['allowConnections'])or'禁用（未核实）'
 note='；'.join(r['remainingGaps']+r['negativeCorrections'])or'外观插口估计；未实机/机械配合验证'
 lines.append(f"| [{r['id']}]({r['sourceDocument']}) | {r['modeledVisualSocketCount']} / {r['modeledGroveSocketCount']} | {desc} | {cable} | {note} |")
lines+=['','颜色的十六进制值是照片参考用色，未经光学标定。红/蓝/黑/白端子不会随外壳金色或半透明设置改变。口内阴影、针脚及插头独立；是否画线由逐口证据控制。','','复现：`python tools/onboard-lane-02-port-audit.py`。截图和逐型号尺寸/失败记录分别位于 `docs/platform/evidence/<product-id>/`。']
(ROOT/'docs/platform/lane02-port-audit.md').write_text('\n'.join(lines)+'\n');print('Port audit:',len(records),'products,',sum(len(r['ports'])for r in records),'estimated sockets')
