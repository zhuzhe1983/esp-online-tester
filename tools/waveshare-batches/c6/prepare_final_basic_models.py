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

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--sources',type=Path,required=True);ap.add_argument('--listing',choices=['zero'],default='zero');args=ap.parse_args();h.configure(args.sources);zero(args.sources/'c6-sources')
