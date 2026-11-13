#!/usr/bin/env python3
"""Deterministic photo-landmark reconstruction; no source image pixels are modified.

Run after acquiring the hash-pinned Assembly Kit sources recorded in model.json.
Coordinates come from the 1600px official component photograph 06. The board
width between x=310 and x=1280 is 61.15mm in the independent assembly drawing.
The M2 bore (800,574) registers to the existing enclosure bore (0,-20).
"""
import json,math
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ASSET=ROOT/'web/platform/assets/m5stack-k159-b';PLAN=ASSET/'model-plan.json'
S=970/61.15;FACE=.05;SOURCE='assembly-board-components';DRAWING='assembly-dimensions'
plan=json.loads(PLAN.read_text());old={p['id']:p for p in plan['derivedParts']};parts=[]
def xy(x,y):return [round((800-x)/S,6),round(-20-(y-574)/S,6)]
def box(identity,rect,height,color='#252825',base=FACE,material=None):
 x0,y0,x1,y1=rect;x,y=xy((x0+x1)/2,(y0+y1)/2)
 p={'id':identity,'kind':'box','basis':'photo-and-drawing','sourceIds':[SOURCE,DRAWING],'color':color,'geometry':{'sizeMm':[round((x1-x0)/S,6),round((y1-y0)/S,6),height],'centerMm':[x,y,round(base-height/2,6)]},'photoLandmarkPx':rect,'notes':'XY footprint measured in official component-face photo; height is an estimate constrained by independent side views and enclosure fit.'}
 if material:p['material']=material
 parts.append(p);return p
metal={'metalness':.35,'roughness':.48}
def photo(identity,rect,z=FACE-.008):
 x0,y0,x1,y1=rect;x,y=xy((x0+x1)/2,(y0+y1)/2)
 p={'id':identity,'kind':'label','basis':'manufacturer-photo','sourceIds':[SOURCE],'texture':'assembly-board-components.webp','geometry':{'sizeMm':[(x1-x0)/S,(y1-y0)/S],'centerMm':[x,y,z],'rotation':[0,math.pi,0],'uvRect':[v/1600 for v in rect]},'notes':'Unmodified original pixels, UV region only; lettering/antenna surface, not a photo covering raised components.'};parts.append(p)
def ring(cx,cy,r,n=40):return [[round(cx+r*math.cos(t*2*math.pi/n),6),round(cy+r*math.sin(t*2*math.pi/n),6)] for t in range(n)]
# Antenna is part of the WROOM module and overhangs the notched main PCB.
outline_px=[[408,428],[710,428],[724,411],[1280,411],[1280,787],[1292,803],[1292,1016],[1304,1027],[1304,1058],[1289,1071],[1289,1137],[1270,1156],[1070,1156],[1054,1133],[1054,1100],[550,1100],[549,1129],[529,1147],[341,1147],[310,1114],[310,1067],[296,1058],[296,1026],[310,1016],[310,740],[405,740],[405,444],[408,444]]
center=[0,-33,.55];outline=[[x,y-center[1]] for x,y in map(lambda p:xy(*p),outline_px)]
mount=xy(357,1122);holes=[ring(0,-20-center[1],1.05),ring(mount[0],mount[1]-center[1],.7)]
parts.append({'id':'pcb-main','kind':'pcb-placeholder','label':'UNKNOWN','color':'#171b19','basis':'photo-and-drawing','sourceIds':[SOURCE,'assembly-board-reverse',DRAWING],'geometry':{'shape':'polygon','sizeMm':[64,48,1],'outlineMm':outline,'holesMm':holes,'centerMm':center,'labelSide':'front','labelOffsetMm':[10,0],'labelSizeMm':[18,4.5]},'photoLandmarkPx':outline_px,'notes':'Visible outline and 1mm thickness follow the official dimension drawing and both board photographs. UNKNOWN marks unverified hidden layer stack/electrical CAD, not absence of photographed exterior evidence. Board normal +Z faces the display; components face -Z.'})
# Reverse photo 07 uses a vertical flip relative to photo 06; bore (800,1026), USB in upper-right.
parts.append({'id':'pcb-reverse-photo','kind':'photo-surface','basis':'manufacturer-photo','sourceIds':['assembly-board-reverse',DRAWING],'texture':'assembly-board-reverse.webp','geometry':{'outlineMm':[[x,y] for x,y in outline],'holesMm':holes,'centerMm':[0,-33,1.053],'side':'front','uvFromMm':[-S/1600,0,.5,0,-S/1600,1-(1026+(-33+20)*S)/1600]},'notes':'Original image mapped to the measured board contour and clipped around both through-bores; substrate only. Backside visible traces are photographic evidence, not editable copper or verified layer stack.'})
# Photo 07 includes switch terminals and the mid-mount USB: remove those rectangles from the photo face.
reverse=parts.pop();top_edge=xy(1280,411)[1]+33;lower_edge=xy(310,1100)[1]+33
# Use the board body's exact contour with edge cutouts; retain the principal bore only in this inset image patch.
reverse['geometry']['outlineMm']=[[xy(408,428)[0],xy(408,428)[1]+33],[xy(710,428)[0],xy(710,428)[1]+33],[xy(724,411)[0],top_edge],[-27.1,top_edge],[-27.1,lower_edge],[29.8,lower_edge],[29.8,xy(310,740)[1]+33],[xy(405,740)[0],xy(310,740)[1]+33]]
reverse['geometry']['holesMm']=[holes[0]]
parts.append(reverse)
# Keep the native display surface and source-derived outer ports; the original enclosure CAD is untouched.
for name in ['display-substrate','display-black-border','top-release-recess','indicator-recess-0','indicator-recess-1']:
 parts.append(old[name])
# ESP32-S3-WROOM module substrate, gold castellations and independent metal shield.
box('pcb-esp32-module',[310,428,717,732],.7,'#171b19')
antenna_back=[xy(x,y) for x,y in [[313,450],[403,450],[403,728],[313,728]]]
parts.append({'id':'pcb-esp32-antenna-reverse','kind':'photo-surface','basis':'manufacturer-photo','sourceIds':['assembly-board-reverse'],'texture':'assembly-board-reverse.webp','geometry':{'outlineMm':antenna_back,'centerMm':[0,0,.053],'side':'front','uvFromMm':[-S/1600,0,.5,0,-S/1600,1-(1026+20*S)/1600]},'notes':'Exposed WROOM antenna underside uses the corresponding original reverse-photo marking region; not a green blank rectangle.'})
box('esp32-shield',[416,457,697,717],2.38,'#c8ceca',base=FACE-.7,material=metal)
# The metal stays physically shaded; only independently legible maker/model text is printed.
parts.append({'id':'esp32-shield-marking','kind':'printed-marking','basis':'manufacturer-photo','sourceIds':['product-photo-4bcd22006c','assembly-board-oblique'],'color':'#5e6661','lines':['ESPRESSIF','ESP32-S3-WROOM-1'],'geometry':{'sizeMm':[15,6],'centerMm':[*xy(556.5,587),-3.04],'rotation':[0,math.pi,math.pi/2]},'notes':'Transparent typographic approximation of legible maker/model text. No baked metal highlights, no invented certification marks or serial number.'})
photo('pcb-esp32-antenna',[314,451,403,725],FACE-.706)
for row,y in [('upper',444),('lower',726)]:
 for i,x in enumerate(range(427,699,20)):box(f'pcb-esp32-pad-{row}-{i}',[x-5,y-3,x+5,y+3],.15,'#bea65b',base=FACE-.7,material=metal)
for i,y in enumerate(range(476,717,21)):box(f'pcb-esp32-pad-end-{i}',[710,y-5,720,y+5],.15,'#bea65b',base=FACE-.7,material=metal)
# Existing M2 insert location is independently confirmed by the dimension drawing and photo 06.
p=old['m2-brass-insert'];p['geometry']['centerMm']=[0,-20,-1.46];p['geometry']['depthMm']=3.0;parts.append(p)
# Recess below the open brass bore (do not seal the through-hole with a bright plane).
# Full TF socket, not just the narrow slot visible outside the case.
box('pcb-microsd-base',[311,756,546,1009],.26,'#202322')
lid=box('microsd-top',[315,773,539,999],.18,'#c8ceca',base=-1.56,material=metal)
lid_x,lid_y=lid['geometry']['centerMm'][:2]
lid_outline=[xy(x,y) for x,y in [[315,773],[539,773],[539,999],[315,999]]]
slots=[[[414,800],[464,800],[464,805],[414,805]],[[414,820],[464,820],[464,825],[414,825]],[[459,905],[496,905],[496,911],[459,911]],[[335,973],[378,973],[378,979],[335,979]]]
lid['kind']='plate';lid['geometry']={'centerMm':lid['geometry']['centerMm'],'depthMm':.18,'outlineMm':[[x-lid_x,y-lid_y] for x,y in lid_outline],'holesMm':[[[x-lid_x,y-lid_y] for x,y in map(lambda p:xy(*p),ring)]for ring in slots]}

box('microsd-bottom',[313,762,546,1006],.12,'#717a77',base=-.06,material=metal)
box('microsd-back',[536,770,546,1001],1.63,'#565d5b',base=-.06,material=metal)
box('microsd-end-upper',[312,761,544,771],1.63,'#929b97',base=-.06,material=metal)
box('microsd-end-lower',[312,998,544,1008],1.63,'#929b97',base=-.06,material=metal)
# True side opening is left in photo coordinates, aligned with the case's right edge.
for i,y in enumerate(range(790,984,27)):box(f'microsd-contact-{i}',[529,y-3,538,y+3],.08,'#c3a14c',base=-.24,material=metal)

# Battery SH connector: back and side walls leave the actual cavity open.
box('battery-connector-body',[925,430,996,499],.7,'#d9dcd6')
box('battery-connector-lip',[925,430,996,440],2.78,'#e5e5df')
box('pcb-battery-wall-left',[925,439,935,498],2.6,'#d6d9d2')
box('pcb-battery-wall-right',[986,439,996,498],2.6,'#d6d9d2')
box('battery-connector-mouth',[937,451,983,493],.05,'#434a42',base=-.72)
for i,x in enumerate([949,974]):box(f'battery-connector-contact-{i}',[x-3,471,x+3,490],.12,'#b9b6a1',base=-.87,material=metal)
# Two horizontal FPC connectors, measured independently, including white housing, latch and 12 contacts.
for name,rect in [('spi',[739,450,864,499]),('i2s',[1072,449,1199,499])]:
 x0,y0,x1,y1=rect;box('pcb-fpc-'+name+'-body',rect,1.12,'#d2d8d2');box('pcb-fpc-'+name+'-latch',[x0+3,y1-16,x1-3,y1],1.35,'#303630')
 for i in range(12):
  x=x0+8+i*(x1-x0-16)/11;box(f'pcb-fpc-{name}-pin-{i}',[x-1.5,y1-21,x+1.5,y1+5],.12,'#c2c2b2',base=-1.08,material=metal)
# Screen board-to-board connector and white location mark.
photo('pcb-display-location-mark',[1126,781,1228,850])
box('pcb-display-fpc-body',[1145,855,1214,1027],1.0,'#1c211e')
box('pcb-display-fpc-slot',[1163,863,1196,1016],.12,'#080c0a',base=-.96)
for side,x in [('a',1153),('b',1205)]:
 for i in range(20):
  y=865+i*7.5;box(f'pcb-display-fpc-pin-{side}-{i}',[x-2,y-1,x+2,y+1],.07,'#b4b4a0',base=-1.05,material=metal)
# Three small physical switches, distinct from the manufacturer CAD external plastic key caps.
for name,rect in [('a',[1235,447,1282,535]),('b',[1235,638,1282,725]),('power',[382,1103,474,1147])]:
 box(f'pcb-switch-{name}-body',rect,.85,'#343a34');x0,y0,x1,y1=rect;box(f'pcb-switch-{name}-metal',[x0+3,y0+4,x1-3,y1-4],.16,'#bac0ba',base=-.81,material=metal)
 if name=='power':box('pcb-switch-power-actuator',[414,1144,442,1161],.6,'#dadbd4',base=-.35)
 else:box(f'pcb-switch-{name}-actuator',[1278,y0+27,1300,y0+57],.62,'#dadbd4',base=-.35)
# Small parts use individually observed bounds, not schematic guesses or a repeated decorative scatter.
passives=[(750,520,34,16),(874,524,28,14),(961,523,29,13),(961,541,29,13),(1000,554,13,27),(1022,554,13,27),(1063,521,29,13),(1063,539,29,13),(1063,558,29,13),(1015,585,23,10),(848,556,10,21),(848,580,10,20),(849,600,18,9),(871,600,18,9),
(750,787,33,17),(750,807,34,17),(751,831,34,16),(799,778,34,17),(799,804,34,16),(846,763,31,15),(848,778,24,11),(901,804,16,35),(926,805,16,35),(951,805,16,35),(984,801,19,19),(1007,801,19,19),(984,823,19,19),(1007,823,19,19),(984,849,19,19),(1007,849,19,19),(984,871,19,19),(1007,871,19,19),
(860,870,29,16),(864,896,30,17),(834,899,29,16),(800,900,25,13),(816,875,13,24),(858,923,16,26),(889,924,16,26),(918,924,15,22),(960,879,24,12),(960,898,24,12),(976,930,15,30),(1003,910,15,14),(1028,927,16,30),(1057,910,18,19),(1081,910,18,19),(1057,936,18,19),(1081,936,18,19),(1057,961,18,19),(1081,961,18,19),
(801,962,25,13),(846,971,28,16),(875,974,17,28),(844,993,26,13),(845,1011,26,13),(845,1033,25,13),(872,1030,25,15),(903,1031,25,15),(929,1015,16,25),(950,1016,16,25),(978,1003,15,26),(1002,1003,15,26),(1018,1028,23,13),(1129,932,16,12),(1129,951,16,12),
(660,832,27,12),(660,849,27,12),(660,909,27,13),(655,946,26,13),(681,947,12,26),(680,762,12,25),(694,762,12,25),(435,747,25,12),(1215,460,10,23),(1213,681,10,23),(452,1079,20,11),(514,1065,10,25),(514,1109,12,30)]
# Narrow resistor/capacitor column adjacent to the card socket, visible separately in photo 06.
passives += [(604,y,24,11) for y in [762,779,797,815,833,850,867,884,902,920,937,955,973,992,1010,1028,1046,1063]]
for i,(x,y,w,h) in enumerate(passives):
 # Terminal shape follows the long axis; body heights deliberately vary by visible package size.
 depth=.48 if max(w,h)>28 else .32;rect=[x-w/2,y-h/2,x+w/2,y+h/2];box(f'pcb-passive-{i:03}-body',rect,depth,'#63675d' if max(w,h)>27 else '#353a33')
 if w>=h:
  ends=[[x-w/2,y-h/2,x-w/2+w*.22,y+h/2],[x+w/2-w*.22,y-h/2,x+w/2,y+h/2]]
 else:ends=[[x-w/2,y-h/2,x+w/2,y-h/2+h*.22],[x-w/2,y+h/2-h*.22,x+w/2,y+h/2]]
 for j,r in enumerate(ends):box(f'pcb-passive-{i:03}-terminal-{j}',r,depth+.035,'#afb6af',material=metal)
ics=[('power-a',[866,553,897,588],.65,8),('power-b',[947,552,984,589],.65,8),('i2s',[1106,530,1135,571],.65,6),('epd-a',[842,793,874,835],.7,8),('epd-b',[1026,796,1065,835],.7,6),('epd-c',[1078,793,1108,837],.7,6),('epd-d',[1027,848,1065,889],.7,6),('epd-e',[1079,847,1110,888],.7,6),('epd-f',[821,918,851,951],.6,6),('epd-g',[758,987,802,1036],.6,8),('pmic',[641,990,694,1041],.7,20),('sd',[645,861,674,900],.6,6),('epd-h',[1067,1016,1096,1040],.6,6)]
for name,rect,height,pins in ics:
 box('pcb-ic-'+name,rect,height,'#1f2520');x0,y0,x1,y1=rect
 for side,x in [('l',x0-3),('r',x1+3)]:
  for i in range(pins//2):
   y=y0+5+i*(y1-y0-10)/(pins//2-1);box(f'pcb-ic-{name}-pin-{side}-{i}',[x-3,y-1.5,x+3,y+1.5],.16,'#b3b9b1',material=metal)
for name,r,h in [('a',[789,819,824,848],.8),('b',[893,864,927,894],1.05),('c',[792,918,818,949],.9),('d',[897,980,924,1006],.8)]:box('pcb-inductor-'+name,r,h,'#373e37')
# Photographic silk/gold markings, never the large metal faces or the screenshot display.
for name,r in [('title',[816,627,1100,682]),('spi-label',[770,503,825,529]),('i2s-label',[1091,502,1163,526]),('spi-pins',[741,412,864,444]),('i2s-pins',[1069,412,1199,444]),('key-a',[1203,540,1280,566]),('key-b',[1201,729,1279,752]),('power',[316,1033,434,1091]),('power-pads',[873,450,919,504]),('input-pads',[1006,450,1070,503])]:photo('pcb-silk-'+name,r)
# Native CAD USB retains its exact geometry and location, with the original recessed tongue.
for name in ['usb-cavity-back','usb-tongue']:parts.append(old[name])
board_ids=[p['id'] for p in parts if p['id'] not in ['display-substrate','display-black-border','top-release-recess','indicator-recess-0','indicator-recess-1']]
board_ids.append('cad-connector-insert')
plan['derivedParts']=parts
plan['textureSources']=[t for t in plan.get('textureSources',[]) if t['file']!='esp32-shield-marking.webp']
plan['presentation']['inspectionGroups']=[g for g in plan['presentation']['inspectionGroups'] if not g['id'].startswith('pcb-assembly')]
for g in plan['presentation']['inspectionGroups']:
 if g['id']=='rear-case':
  g['parts']=list(dict.fromkeys(g['parts']+['top-release-recess']));g['notes']='Native removable rear cover moved for inspection. Board below is independently photo/drawing constrained.'
for g in plan['presentation']['inspectionGroups']:
 if g['id']=='front-case':g['parts']=list(dict.fromkeys(g['parts']+['indicator-recess-0','indicator-recess-1']))
for i in range(0,len(board_ids),128):plan['presentation']['inspectionGroups'].append({'id':f'pcb-assembly-{i//128}','parts':board_ids[i:i+128],'offsetMm':[-72,0,-12],'sourceIds':[SOURCE,'assembly-board-reverse',DRAWING],'notes':'PCB, source-CAD USB and all observed components move as one unit; illustrative viewing separation, not an assembly/disassembly sequence.'})
plan['pcbReconstruction']={'method':'Official photo landmarks with independent manufacturer dimensions; no generated reference pixels','componentPhotoPixels':[1600,1600],'pixelsPerMm':S,'registration':{'sourceM2BorePx':[800,574],'worldM2BoreMm':[0,-20],'sourceWidthPx':[310,1280],'drawingWidthMm':61.15},'boardThicknessMm':1,'mainBoardCenterZMm':.55,'componentFaceZMm':FACE,'reversePhotoRegistration':{'sourceM2BorePx':[800,1026],'verticalFlipRelativeToComponentPhoto':True},'sourceOutlinePx':outline_px,'passiveCount':len(passives),'icCount':len(ics),'heightStatus':'Photo-constrained estimate; side photos08/09 plus available casing clearances, not package manufacturer CAD','unknown':'Hidden copper layers, exact stackup, package heights and physical manufacturing tolerances are unverified.'}
plan['limitations']=[p for p in plan['limitations'] if 'PCB UNKNOWN' not in p and 'Hidden lengths' not in p]
plan['limitations'] += ['PCB is reconstructed from the official Assembly Kit component/reverse photos and dimension drawing. Outline, black finish, two bores and visible layout have evidence; hidden layer stack and exact package heights are not board CAD. UNKNOWN on the inner face identifies that remaining scope.', 'Antenna module, card socket, FPC connectors and observed SMT packages are independent meshes. Lettering and reverse-face traces use unmodified official image UVs; metallic component highlights are rendered with PBR materials. Photo brightness and color are not calibrated.', 'Complete Kit and Assembly Kit share the official schematic; assembly photo landmarks are registered to the native Complete Kit enclosure bore, keys and USB. This establishes visual assembly fit, not electrical manufacturing equivalence or a physical hardware test.']
plan['limitations']=list(dict.fromkeys(plan['limitations']))
PLAN.write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({'parts':len(parts),'passives':len(passives),'ics':len(ics),'widthScale':S,'boardZ':center[2]}))
