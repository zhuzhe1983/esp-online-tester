import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {existsSync,mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {delimiter,dirname,join,resolve} from 'node:path';
import {test} from 'node:test';
import {fileURLToPath} from 'node:url';
import {gunzipSync} from 'node:zlib';

const here=dirname(fileURLToPath(import.meta.url)),generator=join(here,'audit-m5-runtime-coverage.mjs');
const sourceRoot=resolve(process.env.M5_COVERAGE_SOURCE_ROOT||resolve(here,'..')),revision=process.env.M5_COVERAGE_REVISION||'HEAD';
const production=join(sourceRoot,'web/wasm/esp32sim.wasm'),env={...process.env};delete env.ESP32SIM_WASM;
const run=(args,extraEnv={})=>spawnSync(process.execPath,[generator,'--source-root',sourceRoot,'--revision',revision,...args],{env:{...env,...extraEnv},encoding:'utf8',maxBuffer:1024*1024,timeout:120000});
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const cohortBytes=readFileSync(join(here,'fixtures/m5-runtime-coverage-historical-cohorts.json')),cohort=JSON.parse(cohortBytes);

test('historical cohort projection is self-contained with a canonical 141-record identity',()=>{
 assert.equal(cohort.kind,'derived-historical-catalog-projection');assert.equal(cohort.records,141);
 assert.equal(cohort.items.length,141);assert.equal(new Set(cohort.items.map(p=>p.id)).size,141);
 assert.deepEqual(cohort.items.map(p=>p.id),cohort.items.map(p=>p.id).sort());
 assert.equal(sha(JSON.stringify(cohort.items)),cohort.subsetCanonicalSha256);
 assert.deepEqual(cohort.items.reduce((a,p)=>(a[p.status]=(a[p.status]||0)+1,a),{}),{unavailable:40,partial:101});
});

test('contribution provenance matches the exact original historical catalog',
 {skip:process.env.M5_COVERAGE_VERIFY_HISTORICAL_SOURCE==='1'?false:'Contribution-only: set M5_COVERAGE_VERIFY_HISTORICAL_SOURCE=1 when the original source object is available'},()=>{
 const result=spawnSync('git',['-C',sourceRoot,'show',cohort.historicalRevision+':'+cohort.catalogPath],{env:{...env,GIT_NO_LAZY_FETCH:'1'},maxBuffer:64*1024*1024});
 assert.equal(result.status,0,result.stderr?.toString());assert.equal(result.stdout.length,cohort.fullCatalogBytes);
 assert.equal(sha(result.stdout),cohort.fullCatalogSha256);
 const items=JSON.parse(result.stdout).items.filter(p=>p.manufacturer==='M5Stack').map(p=>({id:p.id,status:p.runtime?.status})).sort((a,b)=>a.id<b.id?-1:a.id>b.id?1:0);
 assert.deepEqual(cohort.items,items);assert.equal(sha(JSON.stringify(items)),cohort.subsetCanonicalSha256);
});

test('wrong explicit WASM pin fails before constructor execution or report creation',()=>{
 const temp=mkdtempSync(join(tmpdir(),'m5-coverage-pin-'));
 try {
  const wasm=join(temp,'fixture.wasm'),out=join(temp,'report.json');writeFileSync(wasm,Buffer.from([0,97,115,109,1,0,0,0]));
  const result=run(['--wasm',wasm,'--expect-wasm-sha256','0'.repeat(64),'--out',out]);
  assert.equal(result.status,1);assert.match(result.stderr,/Constructor WASM SHA-256 mismatch/);assert.equal(existsSync(out),false);
 }finally{rmSync(temp,{recursive:true,force:true});}
});

test('missing WASM artifact fails without an optional pin and creates no report',()=>{
 const temp=mkdtempSync(join(tmpdir(),'m5-coverage-missing-'));
 try {
  const out=join(temp,'report.json'),result=run(['--wasm',join(temp,'missing.wasm'),'--out',out]);
  assert.equal(result.status,1);assert.match(result.stderr,/Unable to read constructor WASM artifact/);assert.match(result.stderr,/ENOENT/);assert.equal(existsSync(out),false);
 }finally{rmSync(temp,{recursive:true,force:true});}
});

test('default production constructs all 141 profiles with historical Git access explicitly blocked',
 {skip:existsSync(production)?false:'Build web/wasm/esp32sim.wasm or set M5_COVERAGE_SOURCE_ROOT for the real constructor check'},()=>{
 const temp=mkdtempSync(join(tmpdir(),'m5-coverage-production-')),out=resolve(process.env.M5_COVERAGE_REPORT||join(temp,'report.json.gz'));
 try {
  const bin=join(temp,'bin'),marker=join(temp,'blocked-history');mkdirSync(bin);
  const realGit=spawnSync('sh',['-c','command -v git'],{env,encoding:'utf8'}).stdout.trim();assert.ok(realGit);
  writeFileSync(join(bin,'git'),'#!/bin/sh\ncase "$*" in *'+cohort.historicalRevision+'*) printf blocked > "$M5_COVERAGE_GIT_MARKER"; exit 77;; esac\nexec "$M5_COVERAGE_REAL_GIT" "$@"\n',{mode:0o755});
  const wrapperEnv={PATH:bin+delimiter+env.PATH,M5_COVERAGE_REAL_GIT:realGit,M5_COVERAGE_GIT_MARKER:marker};
  const blocked=spawnSync('git',['show',cohort.historicalRevision+':'+cohort.catalogPath],{env:{...env,...wrapperEnv}});assert.equal(blocked.status,77);assert.equal(existsSync(marker),true);rmSync(marker);
  const expected=sha(readFileSync(production)),result=run(['--require-all-construct','--out',out],wrapperEnv);
  assert.equal(result.status,0,result.stderr||result.error?.message);assert.equal(result.signal,null);
  assert.equal(existsSync(marker),false,'Audit attempted to access the unavailable historical object');
  const report=JSON.parse(out.endsWith('.gz')?gunzipSync(readFileSync(out)):readFileSync(out));
  assert.equal(report.schemaVersion,2);assert.equal(report.constructorArtifact.selection,'production-web/wasm');
  assert.equal(report.constructorArtifact.expectedSha256,null);assert.equal(report.constructorArtifact.sha256,expected);
  assert.equal(sha(readFileSync(production)),expected,'Production artifact changed during construction');
  assert.equal(report.constructorArtifact.firmwareLoaded,false);assert.equal(report.constructorArtifact.firmwareExecuted,false);
  assert.equal(report.summary.products,141);assert.deepEqual(report.summary.construction,{constructed:141});
  assert.deepEqual(report.summary.cohorts,{original101:101,continuation40:40});
  assert.equal(report.historicalCohortSnapshot.kind,cohort.kind);assert.equal(report.historicalCohortSnapshot.sha256,sha(cohortBytes));
  assert.equal(report.historicalCohortSnapshot.runtimeRequiresHistoricalGitObject,false);assert.equal(report.historicalCohortSnapshot.rerunByThisAudit,false);
  assert.equal(report.products.length,141);assert.ok(report.products.every(p=>p.runtime.construction.firmwareExecuted===false));
  assert.ok(report.references.length>0);assert.ok(report.references.every(ref=>ref.rerunByThisAudit===false));
  assert.ok(report.references.some(ref=>ref.constructorArtifactMatch==='recorded-hash-differs'));
  assert.ok(report.references.every(ref=>['recorded-hash-matches','recorded-hash-differs','wasm-hash-not-recorded'].includes(ref.constructorArtifactMatch)));
 }finally{rmSync(temp,{recursive:true,force:true});}
});
