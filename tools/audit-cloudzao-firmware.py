#!/usr/bin/env python3
"""Audit the two pinned public CloudZao images without persisting firmware bytes.

Only the JSON report is written. No flashing, execution, credential access, or
binary output is performed. The source URLs, expected length and release SHA-256
come from the adjacent firmware fragment. Redirect query strings are not logged.

Usage: python tools/audit-cloudzao-firmware.py --output firmware-memory-audit.json
Default input: docs/platform/evidence/cloudzao-20261123/official-firmware-sources.json
Without --output the complete JSON report is printed to stdout.
"""
from __future__ import annotations
import argparse, concurrent.futures, datetime, hashlib, json, pathlib, struct, time, urllib.error, urllib.parse, urllib.request

TARGETS = {
    'cloudzao-claw-154': {'chipId':9, 'chip':'esp32s3', 'flashBytes':16*1024*1024, 'bootOffset':0, 'repo':'Metalio-Claw-1.54'},
    'cloudzao-claw4': {'chipId':18, 'chip':'esp32p4', 'flashBytes':32*1024*1024, 'bootOffset':0x2000, 'repo':'MetalioClaw4'},
}
class AuditFailure(Exception):
    pass

def need(condition, text):
    if not condition:
        raise AuditFailure(text)

def u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]

def u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]

def c_string(data, offset, length):
    # Fixed public firmware descriptor strings only; never dump arbitrary bytes.
    return bytes(data[offset:offset+length]).split(b'\0',1)[0].decode('utf-8','replace')

def image_at(data, at, end_limit, chip_id, expect_app=False):
    need(at+24 <= end_limit <= len(data), 'Image header outside its permitted byte range')
    need(data[at] == 0xe9, 'Missing ESP image magic')
    count=data[at+1]
    need(1 <= count <=16, 'Invalid ESP segment count')
    need(u16(data,at+12)==chip_id, 'ESP header chip ID mismatch')
    need(data[at+23] in (0,1), 'Invalid appended hash flag')
    cursor=at+24; checksum=0xef; segments=[]
    for index in range(count):
        need(cursor+8<=end_limit, 'Segment header truncated')
        address,length=struct.unpack_from('<II',data,cursor);cursor+=8
        need(cursor+length<=end_limit, 'Segment data truncated or exceeds declared partition')
        # XOR of data only; image and segment headers are excluded.
        for value in memoryview(data)[cursor:cursor+length]:
            checksum ^= value
        segments.append({'index':index,'loadAddress':address,'fileOffset':cursor,'bytes':length})
        cursor+=length
    checksum_at=at+((cursor-at)//16)*16+15
    need(checksum_at<end_limit,'Image checksum byte missing')
    need(data[checksum_at]==checksum,'ESP XOR checksum mismatch')
    image_end=checksum_at+1
    hash_appended=bool(data[at+23]);digest=None
    if hash_appended:
        need(image_end+32<=end_limit,'Appended image SHA-256 truncated')
        digest=hashlib.sha256(memoryview(data)[at:image_end]).digest()
        need(digest==data[image_end:image_end+32],'Appended image SHA-256 mismatch')
        image_end+=32
    first=segments[0]
    descriptor=None
    if first['bytes']>=256 and u32(data,first['fileOffset'])==0xabcd5432:
        desc=first['fileOffset']
        descriptor={'magic':'0xabcd5432','secureVersion':u32(data,desc+4),'version':c_string(data,desc+16,32),'projectName':c_string(data,desc+48,32),'compileTime':c_string(data,desc+80,16),'compileDate':c_string(data,desc+96,16),'idfVersion':c_string(data,desc+112,32),'appElfSha256':bytes(data[desc+144:desc+176]).hex()}
    if expect_app:
        need(descriptor is not None,'ESP application descriptor missing')
    size_code=data[at+3]>>4
    return {'offset':at,'entryAddress':u32(data,at+4),'chipId':chip_id,'spiMode':data[at+2],'flashSizeHeaderCode':size_code,'flashSizeHeaderBytes':(1024*1024)<<size_code if size_code<=7 else None,'minChipRevisionLegacy':data[at+14],'minChipRevisionFull':u16(data,at+15),'maxChipRevisionFull':u16(data,at+17),'segments':segments,'checksumOffset':checksum_at,'xorChecksum':checksum,'xorChecksumVerified':True,'checksumPaddingZero':all(v==0 for v in memoryview(data)[cursor:checksum_at]),'hashAppended':hash_appended,'imageSha256':digest.hex() if digest else None,'appendedSha256Verified':True if hash_appended else None,'imageBytes':image_end-at,'imageEnd':image_end,'appDescriptor':descriptor}

def partition_table(data, offset, flash_bytes):
    need(offset%4096==0 and offset>=0x8000,'Invalid partition table location')
    need(offset+32<=len(data)and u16(data,offset)==0x50aa,'Missing partition table at selected offset')
    rows=[];at=offset;md5=None;md5_seen=False
    while at+32<=min(len(data),offset+4096):
        magic=u16(data,at)
        if magic==0x50aa:
            need(not md5_seen,'Partition entry after MD5 terminator')
            typ,subtype=data[at+2],data[at+3];start,size=u32(data,at+4),u32(data,at+8)
            need(size>0 and start>=offset+4096 and start+size<=flash_bytes,'Partition outside product flash capacity')
            need(start%4096==0,'Partition offset is not sector aligned')
            need(typ!=0 or start%65536==0,'App partition not 64 KiB aligned')
            need(not any(start<r['offset']+r['bytes']and r['offset']<start+size for r in rows),'Overlapping partitions')
            rows.append({'name':c_string(data,at+12,16),'type':typ,'subtype':subtype,'offset':start,'bytes':size,'flags':u32(data,at+28)})
        elif magic==0xebeb:
            need(not md5_seen,'Repeated partition table MD5 marker')
            need(all(v==0xff for v in data[at+2:at+16]),'Unexpected partition MD5 marker padding')
            actual=hashlib.md5(memoryview(data)[offset:at]).digest()
            need(actual==data[at+16:at+32],'Partition table MD5 mismatch')
            md5=actual.hex();md5_seen=True
        elif magic==0xffff:
            need(all(v==0xff for v in data[at:at+32]),'Invalid partition end marker')
            break
        else:
            raise AuditFailure('Unexpected partition table record marker')
        at+=32
    need(rows,'Empty partition table')
    return {'offset':offset,'entryCount':len(rows),'md5Present':md5_seen,'md5':md5,'md5Verified':True if md5_seen else None,'partitions':rows}

def inspect_memory(data,target):
    need(len(data)<=target['flashBytes'],'Merged image exceeds configured product capacity')
    table_offsets=[off for off in (0x8000,0x9000)if off+2<=len(data)and u16(data,off)==0x50aa]
    need(len(table_offsets)==1,'Partition table absent or ambiguous at 0x8000 / 0x9000')
    table=partition_table(data,table_offsets[0],target['flashBytes'])
    boot=image_at(data,target['bootOffset'],table['offset'],target['chipId'])
    apps=[];empty=[];other=[]
    for part in table['partitions']:
        if part['type']!=0:
            continue
        at=part['offset'];limit=min(len(data),at+part['bytes'])
        if at>=len(data):
            empty.append({'partition':part['name'],'offset':at,'reason':'outside downloaded extent / unpopulated'})
        elif data[at]==0xe9:
            app=image_at(data,at,limit,target['chipId'],True);app['partition']=part['name'];apps.append(app)
        elif all(v==0xff for v in memoryview(data)[at:limit]):
            empty.append({'partition':part['name'],'offset':at,'reason':'erased 0xff'})
        else:
            other.append({'partition':part['name'],'offset':at,'reason':'nonempty app partition has no ESP image magic'})
    need(apps,'No complete ESP application found at declared app partitions')
    need(not other,'Unrecognized populated app partition')
    return {'bootloader':boot,'partitionTable':table,'applications':apps,'unpopulatedAppPartitions':empty,'allPresentImagesChipVerified':True,'allPresentImagesBoundsVerified':True,'allPresentImagesXorVerified':True,'allAppendedDigestsVerified':True,'capacityVerified':True,'layoutVerified':True}

def audit_one(item):
    pid=item['productIds'][0];target=TARGETS[pid];source=item['source'];expected_size=item['sourceIdentity']['bytes'];url=source['url'];u=urllib.parse.urlsplit(url)
    need(u.scheme=='https'and u.netloc=='github.com'and not u.query and not u.fragment and u.path.startswith('/CloudZao/'+target['repo']+'/releases/download/'),'Only the selected public CloudZao release URL is permitted')
    report={'firmwareId':item['id'],'productId':pid,'sourceUrl':url,'expectedSha256':source['sha256'],'expectedBytes':expected_size,'target':target,'persistence':'RAM-only; firmware bytes never written to disk','simulationExecuted':False,'simulationVerified':False,'hardwareFlashed':False,'hardwareValidated':False,'checkedAt':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        request=urllib.request.Request(url,headers={'User-Agent':'esp-online-tester static firmware integrity audit','Accept':'application/octet-stream'})
        started=time.monotonic();data=bytearray();digest=hashlib.sha256()
        with urllib.request.urlopen(request,timeout=45)as response:
            report.update({'httpStatus':response.status,'mediaType':response.headers.get('Content-Type'),'finalHost':urllib.parse.urlsplit(response.url).hostname,'accessControlAllowOriginObserved':response.headers.get('Access-Control-Allow-Origin')})
            while True:
                chunk=response.read(min(1024*1024,expected_size+1-len(data)))
                if not chunk:break
                data.extend(chunk);digest.update(chunk)
                need(len(data)<=expected_size,'Download exceeds expected byte length')
                need(time.monotonic()-started<120,'Download time limit exceeded')
        actual=digest.hexdigest();report.update({'actualBytes':len(data),'actualSha256':actual,'elapsedSeconds':round(time.monotonic()-started,3)})
        need(len(data)==expected_size,'Downloaded length differs from release metadata')
        need(actual==source['sha256'],'Downloaded SHA-256 differs from release metadata')
        report['sourceDigestVerified']=True;report.update(inspect_memory(data,target));report['result']='static-integrity-and-layout-pass'
        del data
    except urllib.error.HTTPError as exc:
        report.update({'result':'failed','error':{'type':'HTTPError','httpStatus':exc.code}})
    except (urllib.error.URLError, TimeoutError) as exc:
        report.update({'result':'failed','error':{'type':type(exc).__name__,'message':'Public firmware download failed; redirect URLs are intentionally not logged.'}})
    except AuditFailure as exc:
        report.update({'result':'failed','error':{'type':'AuditFailure','message':str(exc)}})
    return report

def main():
    ap=argparse.ArgumentParser()
    default_input=pathlib.Path(__file__).resolve().parents[1]/'docs/platform/evidence/cloudzao-20261123/official-firmware-sources.json'
    ap.add_argument('--input','--fragment',dest='input',type=pathlib.Path,default=default_input)
    ap.add_argument('--output',type=pathlib.Path,default=None,help='Write only the JSON report to this path; default is stdout.')
    args=ap.parse_args()
    items=json.loads(args.input.read_text())['items'];need(len(items)==2,'Expected exactly the two selected CloudZao items')
    need({x['productIds'][0]for x in items}==set(TARGETS),'Unexpected firmware target set')
    with concurrent.futures.ThreadPoolExecutor(max_workers=2)as executor:reports=list(executor.map(audit_one,items))
    out={'schemaVersion':'1','method':'Independent ESP header / segment / partition MD5 / XOR / appended SHA-256 static inspection; entire downloads kept only in process memory.','execution':'No firmware executed, flashed, or saved. Static checks do not establish application or board runtime compatibility.','cors':'Executor HTTP reads are not browser CORS tests.','scriptSha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),'reports':reports}
    report_text=json.dumps(out,ensure_ascii=False,indent=2)+'\n'
    if args.output is None:
        print(report_text,end='')
    else:
        args.output.write_text(report_text)
    return 0 if all(r['result']=='static-integrity-and-layout-pass'for r in reports)else 1
if __name__=='__main__':raise SystemExit(main())
