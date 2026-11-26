#!/usr/bin/env python3
"""Validate current M5 records or prove lossless alignment against the pinned baseline."""
import argparse,collections,copy,datetime,hashlib,importlib.metadata,json,pathlib,re,shutil,subprocess,tempfile,urllib.parse
from jsonschema import Draft202012Validator,FormatChecker
BASE='710a6911dc2343b9a097403d5338f4ed7e0b19f7'
def sha(b):return hashlib.sha256(b).hexdigest()
def canonical(x):return json.dumps(x,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()
def write(p,x):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--repo',type=pathlib.Path,default=pathlib.Path(__file__).resolve().parents[4]);p.add_argument('--baseline',default=BASE);p.add_argument('--validate-current',action='store_true',help='Validate current files without comparing concurrent source changes to the frozen baseline');p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args();root=a.repo.resolve();git=lambda path:subprocess.check_output(['git','show',a.baseline+':'+path],cwd=root);schema_path='web/platform/schemas/knowledge.schema.json';schema_bytes=(root/schema_path).read_bytes();schema=json.loads(schema_bytes);before_schema=None if a.validate_current else json.loads(git(schema_path));checker=FormatChecker();source_start=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
 if a.validate_current:
  frozen_names=['validation.json','baseline-errors.json','baseline-structural-errors.json','record-additions.json'];locations=[pathlib.Path(__file__).resolve().parent,root/'docs/platform/evidence/m5-knowledge-schema-20261126']
  if a.output.resolve()in{folder/name for folder in locations for name in frozen_names}:raise ValueError('--validate-current requires a separate receipt path; frozen baseline evidence is preserved')
 @checker.checks('date-time')
 def timestamp(x):
  if not isinstance(x,str):return True
  if not re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})',x):return False
  try:return datetime.datetime.fromisoformat(x.replace('Z','+00:00')).utcoffset()is not None
  except ValueError:return False
 @checker.checks('uri')
 def uri(x):
  if not isinstance(x,str):return True
  try:u=urllib.parse.urlsplit(x);return bool(u.scheme and u.netloc)and not re.search(r'\s',x)and not u.username and not u.password
  except ValueError:return False
 Draft202012Validator.check_schema(schema);validator=Draft202012Validator(schema,format_checker=checker);old_validator=None if a.validate_current else Draft202012Validator(before_schema,format_checker=checker)
 if a.validate_current:
  assert schema['$defs']['id']=={'type':'string','pattern':'^[a-z0-9][a-z0-9-]{0,100}$'};assert schema['properties']['id']=={'$ref':'#/$defs/id'};assert schema['properties']['catalogId']=={'$ref':'#/$defs/id'}
 else:
  assert schema['$defs']['id']==before_schema['$defs']['id'];assert schema['properties']['id']==before_schema['properties']['id'];assert schema['properties']['catalogId']==before_schema['properties']['catalogId']
 assert schema['additionalProperties']is False and schema['$defs']['source']['additionalProperties']is False
 current_catalogs={name:(root/'web/platform/catalog'/f'{name}.json').read_bytes()for name in ['products','accessories','firmwares']}if a.validate_current else{}
 catalog_bytes=current_catalogs['products']if a.validate_current else git('web/platform/catalog/products.json');catalog=json.loads(catalog_bytes);m5={x['id']for x in catalog['items']if x.get('manufacturer')=='M5Stack'};assert len(m5)==141
 index_bytes=(root/'web/platform/knowledge/index.json').read_bytes();index=json.loads(index_bytes);record_bytes={};all_records={};old_records={};rows=[];errors=[];old_errors=[];schema_regressions=[];source_refs=0;all_refs=0;all_unknowns=0;new_fields=[];before_valid=0;after_valid=0;other_errors=0;structural_before=0;structural_errors=[]
 for entry in index['entries']:
  assert re.fullmatch(r'\./[a-z0-9-]+\.json',entry['url']),'Unsafe knowledge index URL'
  relative='web/platform/knowledge/'+entry['url'].removeprefix('./');raw=(root/relative).read_bytes();record_bytes[pathlib.Path(relative).name]=raw;record=json.loads(raw);all_records[record['id']]=record;ve=list(validator.iter_errors(record));after_valid+=not ve;oe=[];old_raw=None;old=None
  if not a.validate_current:
   old_raw=git(relative);old=json.loads(old_raw);old_records[record['id']]=old;oe=list(old_validator.iter_errors(old));before_valid+=not oe
   if not oe and ve:schema_regressions.append(record['id'])
  if record['id']not in m5:
   other_errors+=len(ve)
   if not a.validate_current:assert raw==old_raw,'Unrequested non-M5 data changed'
   continue
  if not a.validate_current:
   structural=list(Draft202012Validator(before_schema).iter_errors(old));structural_before+=len(structural);structural_errors.extend({'productId':record['id'],'path':list(e.path),'validator':e.validator,'message':e.message}for e in structural)
  errors.extend({'productId':record['id'],'path':list(e.path),'validator':e.validator,'message':e.message}for e in ve);old_errors.extend({'productId':record['id'],'path':list(e.path),'validator':e.validator,'message':e.message}for e in oe)
  # Prove that every old value and array position survives; only the documented missing fields may be added.
  additions=[]
  def preserve(x,y,path=()):
   assert type(x)is type(y),(record['id'],path,'type changed')
   if isinstance(x,dict):
    for key,value in x.items():assert key in y,(record['id'],path,'removed field');preserve(value,y[key],path+(key,))
    for key in y.keys()-x.keys():additions.append({'path':list(path+(key,)),'value':y[key]})
   elif isinstance(x,list):
    assert len(x)==len(y),(record['id'],path,'array size changed')
    for i,(a0,b0)in enumerate(zip(x,y)):preserve(a0,b0,path+(i,))
   else:assert x==y,(record['id'],path,'value changed')
  if not a.validate_current:
   preserve(old,record);assert old['sources']==record['sources'];assert old['unknowns']==record['unknowns'];assert old['facts']==record['facts']
  all_unknowns+=len(record['unknowns'])
  for addition in additions:
   path=addition['path'];value=addition['value'];valid=False
   if len(path)==3 and path[0]=='revisions':
    if path[2]=='releaseDate':valid=value is None
    elif path[2]=='id':valid=record['id']=='m5stack-k005'and value=='flash-old-versus-current'
    elif path[2]=='basis':valid=record['id']=='m5stack-k005'and value=='official'and any(f['id']=='spec-0-3'and f['value']=='16MB (Old version 4MB)'and f['basis']=='official'for f in old['facts'])
   if len(path)==3 and path[0]=='assets':
    asset=record['assets'][path[1]]
    valid=path[2]=='id'and value==asset['sourceId']or path[2]=='role'and value==asset['kind']=='photo'
   assert valid,(record['id'],addition)
  new_fields.extend({'productId':record['id'],**x}for x in additions);sources={x['id']:x for x in record['sources']};assert len(sources)==len(record['sources']);refs=[]
  def collect(x,path=()):
   if isinstance(x,dict):
    for key,value in x.items():
     if key in ['sourceId','discoveredFrom','containerId']:refs.append((path+(key,),value))
     elif key in ['sourceIds','summarySourceIds']:refs.extend((path+(key,i),ref)for i,ref in enumerate(value))
     else:collect(value,path+(key,))
   elif isinstance(x,list):
    for i,value in enumerate(x):collect(value,path+(i,))
  collect(record)
  for path,ref in refs:assert ref in sources,(record['id'],path,'dangling source',ref)
  all_refs+=len(refs);source_refs+=len(sources)
  for source in sources.values():
   acquisition=source.get('acquisition',source.get('revision',{}).get('acquisition')if isinstance(source.get('revision'),dict)else None)
   if acquisition:assert sources[acquisition['containerId']]['sha256']==acquisition['containerSha256'];assert acquisition['declaredChecksum']==acquisition['verifiedChecksum']
  assert len({x['id']for x in record['assets']})==len(record['assets'])
  keyboard=record.get('keyboardMapping')
  if keyboard:
   assert len(keyboard['rowGpios'])==keyboard['rows']and len(keyboard['columnGpios'])==keyboard['columns'];occupied=set()
   for key in keyboard['keys']:
    row,col=key['row'],key['column'];assert row<keyboard['rows']and col<keyboard['columns'];assert(row,col)not in occupied;occupied.add((row,col));assert key['scanIndex']==row*keyboard['columns']+col;assert key['rowGpio']==keyboard['rowGpios'][row]and key['columnGpio']==keyboard['columnGpios'][col]
   assert len(occupied)+len(keyboard['unpopulated'])==keyboard['rows']*keyboard['columns']
  row={'productId':record['id'],'recordSha256':sha(raw),'sourceRecords':len(sources),'sourcesCanonicalSha256':sha(canonical(record['sources'])),'unknownCount':len(record['unknowns']),'unknownsCanonicalSha256':sha(canonical(record['unknowns'])),'factsCanonicalSha256':sha(canonical(record['facts'])),'sourceReferencesChecked':len(refs),'schemaErrorsAfter':len(ve)}
  if not a.validate_current:row.update(beforeSha256=sha(old_raw),afterSha256=sha(raw),changed=old_raw!=raw,newFields=additions,originalValuesPreserved=True,schemaErrorsBefore=len(oe))
  rows.append(row)
 assert {r['productId']for r in rows}==m5,'M5 catalog/index coverage differs'
 assert not errors and not schema_regressions, (errors[:8],schema_regressions)
 negatives=[]
 def reject(name,id,mutate,reference_only=False):
  sample=copy.deepcopy(all_records[id]);mutate(sample);fail=bool(list(validator.iter_errors(sample)))
  if reference_only:
   sources={s['id']for s in sample['sources']};fail=any(ref not in sources for f in sample['facts']for ref in f['sourceIds'])
  assert fail,name;negatives.append({'case':name,'rejected':True,'layer':'source-reference-check'if reference_only else'JSON-Schema-with-format-assertions'})
 reject('catalog-id-remains-strict','m5stack-k049',lambda r:r.update(catalogId='M5Stack_K049.json'))
 reject('device-record-id-remains-strict','m5stack-k049',lambda r:r.update(id='M5Stack_K049.json'))
 reject('source-id-rejects-path','m5stack-k049',lambda r:r['sources'][0].update(id='../runtime.c'))
 reject('unknown-top-extension','m5stack-k049',lambda r:r.update(inventedExtension=True))
 reject('source-checkedAt-type','m5stack-k049',lambda r:r['sources'][0].update(checkedAt=7))
 reject('source-checkedAt-impossible-date','m5stack-k049',lambda r:r['sources'][0].update(checkedAt='2026-02-30'))
 reject('source-revision-type','m5stack-k049',lambda r:r['sources'][0].update(revision=True))
 reject('source-revision-unknown-field','m5stack-k049',lambda r:r['sources'][0].update(revision={'invented':True}))
 reject('linked-source-cannot-invent-fetch-receipt','m5stack-k049',lambda r:next(s for s in r['sources']if s['status']=='linked').update(sha256='0'*64))
 reject('unknown-cannot-be-established-value','m5stack-k049',lambda r:r['unknowns'][0].update(value='manufactured claim'))
 reject('missing-basis-still-invalid','m5stack-k049',lambda r:r['facts'][0].pop('basis'))
 reject('release-date-invalid-calendar','m5stack-u037',lambda r:r['revisions'][0].update(releaseDate='2026-02-30'))
 reject('asset-role-still-required','m5stack-k049-c',lambda r:r['assets'][0].pop('role'))
 reject('acquisition-hash-type','m5stack-s016',lambda r:next(s for s in r['sources']if'acquisition'in s)['acquisition'].update(containerSha256=17))
 reject('survey-unknown-nested-key','m5stack-k159',lambda r:r['firmwareSurvey']['uiFlow'].update(invented=True))
 reject('keyboard-gpio-type','m5stack-u215',lambda r:r['keyboardMapping']['rowGpios'].__setitem__(0,'G0'))
 reject('source-locator-rejects-http','m5stack-k049',lambda r:r['sources'][0].update(url='http://example.org/document'))
 reject('source-locator-rejects-host-space','m5stack-k049',lambda r:r['sources'][0].update(url='https://exa mple.org/document'))
 reject('source-locator-rejects-control-character','m5stack-k049',lambda r:r['sources'][0].update(url='https://example.org/a'+chr(1)))
 reject('catalog-role-id-remains-strict','m5stack-u202',lambda r:r['catalogRoles'].update(productId='M5Stack_U202'))
 reject('firmware-survey-id-remains-strict','m5stack-k159',lambda r:r['firmwareSurvey'].update(defaultFirmwareId='Factory_Firmware.bin'))
 reject('acquisition-unknown-field','m5stack-s016',lambda r:next(s for s in r['sources']if'acquisition'in s)['acquisition'].update(invented=True))
 reject('dangling-source-reference','m5stack-k049',lambda r:r['facts'][0]['sourceIds'].append('missing-source'),True)
 # Execute unchanged JS against the same captured current inputs, or the pinned baseline catalogs.
 test_bytes=(root/'tools/platform-knowledge-test.mjs').read_bytes()if a.validate_current else git('tools/platform-knowledge-test.mjs')
 with tempfile.TemporaryDirectory(prefix='m5-knowledge-js-')as tmp:
  stage=pathlib.Path(tmp);(stage/'tools').mkdir();(stage/'web/platform/catalog').mkdir(parents=True);knowledge=stage/'web/platform/knowledge'
  if a.validate_current:
   knowledge.mkdir();(knowledge/'index.json').write_bytes(index_bytes)
   for name,raw in record_bytes.items():(knowledge/name).write_bytes(raw)
  else:knowledge.symlink_to(root/'web/platform/knowledge',target_is_directory=True)
  test=stage/'tools/platform-knowledge-test.mjs';test.write_bytes(test_bytes)
  for name in ['products','accessories','firmwares']:(stage/'web/platform/catalog'/f'{name}.json').write_bytes(current_catalogs[name]if a.validate_current else git('web/platform/catalog/'+name+'.json'))
  result=subprocess.run(['node','--test',str(test)],capture_output=True,text=True,timeout=60);assert result.returncode==0,result.stdout+result.stderr;summary=next(line for line in result.stdout.splitlines()if'platform-knowledge:'in line)
 source_end=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
 if a.validate_current:
  report={'schemaVersion':1,'mode':'validate-current','losslessBaselineComparisonPerformed':False,'auditScriptSha256':sha(pathlib.Path(__file__).read_bytes()),'sourceCommitStart':source_start,'sourceCommitEnd':source_end,'schemaSha256':sha(schema_bytes),'catalogSha256':sha(catalog_bytes),'knowledgeIndexSha256':sha(index_bytes),'validator':{'pythonJsonschema':importlib.metadata.version('jsonschema'),'draft':'2020-12','dateTimeAssertions':'Explicit ISO syntax plus datetime calendar/timezone checks; no optional format plugin required.','remoteRefs':False},'summary':{'m5Products':len(rows),'schemaErrorsAfter':len(errors),'sourceRecordsValidated':source_refs,'unknownsValidated':all_unknowns,'referencesChecked':all_refs,'negativeCases':len(negatives),'allIndexedRecords':len(index['entries']),'currentlySchemaValid':after_valid,'remainingNonM5SchemaErrors':other_errors,'js736Passed':True},'jsValidation':{'sourceTestSha256':sha(test_bytes),'exitCode':result.returncode,'summary':summary,'catalogSource':'current files'},'negativeCases':negatives,'products':rows,'limits':['Current validation performs no baseline comparison and attributes no concurrent data changes to the Schema repair.','The original lossless-baseline receipt remains independent and unchanged; current source/fact/unknown values are validated, not claimed historically identical.','Non-M5 current Schema gaps are counted but not claimed fixed.','No additional source fetching, hardware execution or firmware acceptance.']};write(a.output,report);print(json.dumps(report['summary'],indent=2));print(summary);return
 report={'schemaVersion':1,'mode':'lossless-baseline','losslessBaselineComparisonPerformed':True,'auditScriptSha256':sha(pathlib.Path(__file__).read_bytes()),'baselineCommit':a.baseline,'schemaSha256Before':sha(git(schema_path)),'schemaSha256After':sha(schema_bytes),'catalogSha256':sha(catalog_bytes),'knowledgeIndexSha256':sha(index_bytes),'validator':{'pythonJsonschema':importlib.metadata.version('jsonschema'),'draft':'2020-12','dateTimeAssertions':'Explicit ISO syntax plus datetime calendar/timezone checks; no optional format plugin required.','remoteRefs':False},'summary':{'m5Products':len(rows),'schemaErrorsBefore':structural_before,'formatAwareErrorsBefore':len(old_errors),'schemaErrorsAfter':len(errors),'changedRecords':sum(r['changed']for r in rows),'addedFields':len(new_fields),'sourceRecordsUnchanged':source_refs,'unknownsUnchanged':all_unknowns,'referencesChecked':all_refs,'negativeCases':len(negatives),'nonM5Records':len(index['entries'])-len(rows),'nonM5PreviouslySchemaValid':before_valid-sum(r['schemaErrorsBefore']==0 for r in rows),'nonM5NowSchemaValid':after_valid-len(rows),'allIndexedRecords':len(index['entries']),'previouslySchemaValid':before_valid,'nowSchemaValid':after_valid,'previouslyValidRegressions':schema_regressions,'remainingNonM5SchemaErrors':other_errors,'js736Passed':True},'jsValidation':{'sourceTestSha256':sha(test_bytes),'exitCode':result.returncode,'summary':summary},'negativeCases':negatives,'products':rows,'limits':['Non-M5 pre-existing schema gaps are counted, not claimed fixed. Their data is unchanged.','No source URL/hash/receipt timestamp, source revision, fact, unknown, existing survey or keyboard mapping value was changed.','No additional source fetching, hardware execution or firmware acceptance.']};write(a.output,report);write(a.output.with_name('baseline-errors.json'),old_errors);write(a.output.with_name('baseline-structural-errors.json'),structural_errors);print(json.dumps(report['summary'],indent=2));print(summary)
if __name__=='__main__':main()
