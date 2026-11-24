"""Final basic boards: native manufacturer assemblies or visibly UNKNOWN photo envelopes."""
from pathlib import Path
import argparse,json
import model_source_helpers as h

def cad(dst,sources,root,repo,pattern):
    files=list((root/repo/'files/archive-members').rglob(pattern));assert len(files)==1,files
    f=files[0];rec=json.loads(Path(str(f)+'.receipt.json').read_text())
    container=Path(rec['containerFile'])if rec.get('containerFile')else next(p for p in(root/repo/'files/official-resources').iterdir()if p.name==Path(rec['url']).name)
    sources.append(h.source(dst,container,'official-archive','source-archive'+container.suffix,'cad-container'))
    row=h.source(dst,f,'official-cad','source.stp','cad-source',compress=True);row.update(origin=rec.get('archiveFormat','zip')+'-member',containerSourceId='official-archive',archive={'entry':rec['archiveMember'],'containerSha256':rec['containerSha256']});row.pop('containerFile',None);sources.append(row)
    return f,json.loads(f.with_suffix('.topology.json').read_text()),json.loads(f.with_suffix('.colors.json').read_text())

def native(p,top,colors,names):
    groups={}
    for i in range(len(top)):
        rgb=colors[i].get('color');color='#'+''.join(f'{round(255*(12.92*x if x<=.0031308 else 1.055*x**(1/2.4)-.055)):02x}'for x in rgb)if rgb else'#a9a9a9'
        key=(names.get(i),color)if i in names else(None,color);groups.setdefault(key,[]).append(i)
    for(name,color),indices in groups.items():p['parts'].append({'name':name or'native-material-'+color[1:],'sourceFile':'source.stp.gz','sourceComponents':indices,'sourceTopology':'solid-or-shell','color':color,'role':'component'})
    p['method']='manufacturer-cad';p['sourceTopologyCounts']={'source.stp.gz':len(top)}
    p['sourceCoverage']={'source.stp.gz':{'includedIndices':list(range(len(top))),'omittedIndices':[],'grouping':'Every native solid/shell retained at original millimetre scale; identical source materials grouped for rendering.'}}
    p['sourceSearch'].update(status='official-native-STEP-found',result='Exact original manufacturer-linked STEP retained with all components. Electrical revision and mechanical qualifications are explicit.')
    p['materialBasis']='Native linear source colors converted to sRGB; absent source materials are neutral grey. No generated silkscreen or fabricated photo texture.'
    p['limitations']=['Native component dimensions and original material distinctions are preserved without scaling. Colors are not calibrated measurements.','No physical fit, browser WebGL, connector contact simulation or hardware acceptance is claimed.','BOOT/RESET workbench controls and RGB state do not establish physical actuator/LED registration. No guessed3D click target or LED position.']

def zero(S):
    repo='ESP32-C6-Zero'
    for m in[False,True]:
        pid='waveshare-esp32-c6-zero'+('-m'if m else'');dst,sources,t=h.common(pid,repo);p=h.basics(pid,sources,t)
        h.picture(dst,sources,repo,'*Dimensions-*.webp','dimension-photo','dimension-photo.webp');h.add_all_photos(dst,sources,repo)
        f,top,colors=cad(dst,sources,S,repo,'ESP32-C6-Zero-M_asm.stp'if m else'ESP32-C6-Zero_asm.stp')
        names={0:'pcb-main',63:'usb-shell',114:'native-mcu-package'}
        if m:names.update({116:'native-header-right',117:'native-header-left'})
        native(p,top,colors,names)
        p['assemblyNotes']='Original native millimetre coordinates retained, front+Z andUSB toward+Y. No recentering, scaling, component replacement or assembly-depth fitting.'
        p['dimensions']={'pcb':{'publishedSizeMm':[18,23.5],'nativeSizeMm':top[0]['size'],'sourceIds':['official-cad','dimension-photo']},'header':{'pitchMm':2.54,'rowSpacingMm':15.24,'pinsPerRow':9,'nativeAssembly':m,'sourceIds':['official-cad','dimension-photo']},'hardwareRevision':{'currentOverview':'C6FH8 integrated8MiB','retainedSchematic':'C6FH4 integrated4MiB','CADRevisionMatch':'unverified; native MCU geometry does not establish embedded flash capacity'}}
        p['dimensionTargets']=[{'part':'pcb-main','sizeMm':top[0]['size'],'toleranceMm':.001,'sourceId':'official-cad'}]
        p['sourceSearch']['archiveMember']=f.name
        p['limitations']+=['Current official overview statesESP32-C6FH8/8MiB; the retained schematic statesESP32-C6FH4/4MiB. Both source profiles use this unscaled mechanical source, but exact electrical revision/CAD correspondence remains UNVERIFIED. No undocumented V1/V2 sale name is invented.','Published18×23.5-mm PCB dimensions differ from native18.0173×23.5173-mm source envelope by approximately0.0173mm. The native source is retained without forced resizing.']
        if m:p['limitations'].append('Both nine-position pre-soldered headers are original native components116/117, not estimated replacements. Source native pin extent is preserved.')
        h.finish(dst,p)

def headers(p,xs,ys,rear,sourceids,basis):
    for side,x in enumerate(xs):
        p['parts'].append({'name':f'header-{side}-body-estimate','role':'header','color':'#202126','geometry':{'shape':'box','sizeMm':[2.4,max(ys)-min(ys)+2.4,2.5],'centerMm':[x,(max(ys)+min(ys))/2,rear-1.25]},'basis':basis+' Unmeasured body width2.4mm and height2.5mm are explicit estimates.','sourceIds':sourceids})
        for i,y in enumerate(ys):p['parts'].append({'name':f'header-{side}-pin-{i}-estimate','role':'header','color':'#bfc0b7','geometry':{'shape':'box','sizeMm':[.64,.64,7.5],'centerMm':[x,y,rear-3]},'basis':basis+' Pin cross-section0.64mm and length7.5mm are illustrative estimates.','sourceIds':sourceids})
    p['limitations'].append('Header body/pin dimensions are explicit unmeasured estimates. Source pin count/spacing or photo registration does not establish an exact mechanical fit.')

def remaining(S):
    photo_profiles=[('ESP32-C6-Pico',21,57.3,'*details-size-*.webp',8.9,17.78,20,24.13,'Published2.54mm pitch and20-pin rows; source17.78mm header row spacing. Pin registration follows the dimension image.'),('ESP32-C3-Zero',18,23.5,'*Dimensions-*.webp',9.3,15.24,9,10.16,'Published2.54mm pitch and9-pin rows; source1.38mm edge-to-pad center establishes15.24mm row spacing.'),('ESP32-H2-DEV-KIT-N4',25.4,51.6,'*Diagram_4-*.webp',9.1,22.86,15,14.8,'Published22.86mm row spacing and schematic15-pin rows.2.54mm pitch and longitudinal registration are photo-derived estimates (pitch uncertainty±0.05mm, registration±0.4mm).')]
    for repo,w,height,pattern,usb,row,count,first,basis in photo_profiles:
        for m in[False,True]:
            pid='waveshare-'+repo.lower()+('-m'if m else'');dst,sources,t=h.common(pid,repo);p=h.basics(pid,sources,t);h.picture(dst,sources,repo,pattern,'dimension-photo','dimension-photo.webp');h.add_all_photos(dst,sources,repo)
            p['parts']=[{'name':'usb-photo-envelope','role':'connector','color':'#bdc2c7','geometry':{'shape':'open-usb-shell','sizeMm':[usb,7.5,3.5],'centerMm':[0,height/2-1.75,1.75],'wallMm':.3},'basis':f'Unchanged official photo scaled against{w}mm PCB width: USB metal width approximately{usb}mm with±0.5mm image uncertainty. Depth7.5mm,height3.5mm,wall0.3mm and registration are explicit estimates; contact internals omitted.','sourceIds':['dimension-photo']}]
            p['derivedParts']=[{'id':'pcb-main','kind':'pcb-placeholder','color':'#263a4a'if'Pico'in repo or'H2'in repo else'#214b7a','geometry':{'sizeMm':[w,height,1.6],'centerMm':[0,0,-.8],'labelSide':'both','labelSizeMm':[min(w-3,18),3]},'basis':'Published PCB XY; estimated1.6mm thickness. Component heights, unseen PCB topology, traces and connector contacts remain visibly UNKNOWN.','sourceIds':['dimension-photo','resources']}]
            p['dimensions']={'pcb':{'publishedXYMm':[w,height],'thicknessMm':None,'illustrativeThicknessMm':1.6,'sourceIds':['dimension-photo']},'headers':{'pinsPerRow':count,'rowSpacingMm':row,'pitchMm':2.54,'basis':basis,'sourceIds':['dimension-photo','resources']}}
            p['dimensionTargets']=[{'part':'pcb-main','sizeMm':[w,height,None],'toleranceMm':.001,'sourceId':'dimension-photo'}]
            p['limitations']+=['No matching native solid assembly was found in the retained canonical resource scope. This is an explicit partial photo/dimension reconstruction, not a claim that CAD does not exist elsewhere.','Only the approximate open USB envelope and optional illustrative headers surround a visibly UNKNOWN PCB. Physical switch/LED registration and component heights are not invented.','Front+Z,USB toward+Y is the model orientation. Source diagrams/photographs remain unmodified independent references.']
            if m:headers(p,[-row/2,row/2],[first-i*2.54 for i in range(count)],-1.6,['dimension-photo'],basis)
            h.finish(dst,p)
    repo='ESP32-C6-DEV-KIT-N8'
    for m in[False,True]:
        pid='waveshare-esp32-c6-dev-kit-n8'+('-m'if m else'');dst,sources,t=h.common(pid,repo);p=h.basics(pid,sources,t);h.add_all_photos(dst,sources,repo)
        drawing=next((S/repo/'files/archive-members').glob('*.pdf'));row=h.source(dst,drawing,'dimension-drawing','dimension-drawing.pdf');row.pop('containerFile',None);sources.append(row)
        f,top,colors=cad(dst,sources,S,repo,'*.stp');native(p,top,colors,{0:'pcb-main',22:'usb-shell',212:'native-module-pcb',215:'native-module-shield'})
        p['assemblyNotes']='Native millimetres and coordinates retained, front+Z,USB toward-Y. Every217 original component is preserved. No scaling or inferred electronic layout for theN16 capacity.'
        p['dimensions']={'pcb':{'publishedXYMm':[25.4,55],'nativeSizeMm':top[0]['size'],'publishedThicknessMm':1.6,'sourceIds':['official-cad','dimension-drawing']},'module':{'nativeFootprintMm':top[212]['size'],'sourceIds':['official-cad']},'variantScope':'N8 native assembly; N16 electrical capacity option mechanical/layout match remains UNVERIFIED.'}
        p['dimensionTargets']=[{'part':'pcb-main','sizeMm':top[0]['size'],'toleranceMm':.001,'sourceId':'official-cad'}]
        p['limitations']+=['Official source identifiesN8. Both current8MiB and16MiB sale options reference this original N8 mechanical assembly; exactN16 layout/CAD revision match is UNVERIFIED. Embedded flash/package identity is not inferred from enclosure geometry.','Published25.4×55mm and native25.3807×55.0207mm PCB envelopes differ slightly. Native source remains unscaled.']
        if m:
            holes=json.loads(f.with_suffix('.holes.json').read_text());pins=[x['center']for x in holes if .44<x['radius']<.46];assert len(pins)==32,pins
            xs=sorted(set(round(q[0],5)for q in pins));ys=sorted(set(round(q[1],4)for q in pins));assert len(xs)==2 and len(ys)==16,(xs,ys)
            headers(p,xs,ys,top[0]['bounds'][0][2],['official-cad','dimension-drawing'],'Header pin centers use native PCB holes rounded to0.0001mm;16 positions/row and2.54mm pitch match manufacturer drawing.')
            p['dimensions']['headerNativeHoleCentersMm']=[[x,y]for x in xs for y in ys]
        h.finish(dst,p)

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--sources',type=Path,required=True);ap.add_argument('--listing',choices=['zero','remaining'],default='zero');args=ap.parse_args();h.configure(args.sources);(zero if args.listing=='zero'else remaining)(args.sources/'c6-sources')
