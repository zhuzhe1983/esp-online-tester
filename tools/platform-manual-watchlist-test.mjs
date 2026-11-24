#!/usr/bin/env node
import assert from 'node:assert/strict';
import test from 'node:test';
import {validateWatchlist,checkWatchEntry} from '../web/platform/watchlist.js';
import {syncReleaseMetadata} from './sync-firmware-release-metadata.mjs';
import {updateFirmwareCatalog} from './sync-firmware-catalog.mjs';
import {releaseRequest,publicReleases} from './firmware-release-source.mjs';
const reviewed='2026-01-02T03:04:05Z',now='2026-11-23T19:00:00Z';
const entry={id:'manual-vendor-board',category:'factory',metadataProvider:'manual',upstreamRepo:'manufacturer:Waveshare/Example-Board',metadataUrl:'https://example.com/wiki/board',releaseUrl:'https://example.com/wiki/board',checkedAt:reviewed,version:'fixed-original',monitoringNote:'Manufacturer publishes a static archive, with no established release API. New revisions require manual source and image review.',assets:[{name:'recovery.zip',url:'https://example.com/recovery.zip',sha256:'a'.repeat(64)}],catalogTargets:[],compatibility:{status:'reference-only',productIds:['board'],simulationVerified:false,hardwareValidated:false},storage:'remote-links-only'};
const list=()=>({schemaVersion:'1',categories:[{id:'factory'}],entries:[structuredClone(entry)]});
test('manual source review never performs network I/O or renews the observation',async()=>{
 const previous={entries:[{id:entry.id,upstreamRepo:entry.upstreamRepo,checkedAt:now,release:{tag_name:'unrelated-old-api'},releases:[{tag_name:'unrelated-old-api'}]}]};
 const snapshot=await syncReleaseMetadata(list(),{previous,now,token:'must-not-leave-this-call',fetchImpl:async()=>assert.fail('Manual source must not request a page, binary, or invented API')});
 assert.equal(snapshot.entries[0].status,'manual-review-required');assert.equal(snapshot.entries[0].checkedAt,reviewed);assert.equal(snapshot.entries[0].release,null);assert.deepEqual(snapshot.entries[0].releases,[]);assert.equal(snapshot.entries[0].error,null);assert.equal(JSON.stringify(snapshot).includes('must-not-leave-this-call'),false);
 const repeated=await syncReleaseMetadata(list(),{previous:snapshot,now:'2026-11-24T19:00:00Z',fetchImpl:async()=>assert.fail('No network')});assert.deepEqual(repeated,snapshot,'A scheduled skip leaves reviewed evidence and snapshot stable');
});
test('manual adapters cannot fall through to GitHub metadata parsing',()=>{
 assert.throws(()=>releaseRequest(entry,'unused'),/no automatic/);assert.throws(()=>publicReleases(entry,{tag_name:'fake'}),/human review/);
 const document=list();document.entries[0].upstreamRepo=null;assert.equal(validateWatchlist(document),document);
});
test('manual review rejects automatic targets and misleading metadata selectors',()=>{
 for(const change of [{catalogTargets:[{firmwareId:'auto'}]},{assetMetadata:'meshtastic-targets'},{metadataAssetNamePattern:'^any$'},{monitoringNote:''},{upstreamRepo:'manufacturer:../private'}]){
  const document=list();Object.assign(document.entries[0],change);assert.throws(()=>validateWatchlist(document));
 }
});
test('public check explains manual review without reporting a newer or broken release',async()=>{
 const report=await checkWatchEntry(entry,{productId:'board',firmwareId:'firmware',currentVersion:'selected-fixed',fetchImpl:async()=>assert.fail('Manual check must not use an old snapshot as a current release')});
 assert.equal(report.metadata.source,'manual-review');assert.equal(report.metadata.checkedAt,reviewed);assert.equal(report.metadata.matchReason,'manual-review-required');assert.equal(report.latestVersion,null);assert.equal(report.previousVersion,'selected-fixed');assert.equal(report.updateAvailable,false);assert.equal(report.issueUrl,null);assert.deepEqual(report.assets,[]);
});
test('catalog sync preserves the pinned trial and emits a manual review status',()=>{
 const firmwares={schemaVersion:'1',items:[{id:'firmware',productIds:['board'],status:'trial',source:{url:'https://example.com/recovery.zip',sha256:'a'.repeat(64)}}]};
 const result=updateFirmwareCatalog({products:{items:[{id:'board'}]},firmwares,watchlist:list(),snapshot:{entries:[]},now});assert.deepEqual(result.firmwares,firmwares);assert.equal(result.report.entries[0].reason,'manual-review-required');assert.equal(result.report.entries[0].metadataStatus,'manual');assert.equal(result.report.summary.updated,0);
});
