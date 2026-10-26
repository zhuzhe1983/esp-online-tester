#!/usr/bin/env node
// Fetch ONLY public release metadata. No firmware/body asset download, no token output.
import { readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import {releaseRequest,publicRelease} from './firmware-release-source.mjs';
import { validateWatchlist } from '../web/platform/watchlist.js';
const root = new URL('../web/platform/catalog/', import.meta.url);
const list = validateWatchlist(JSON.parse(await readFile(new URL('firmware-watchlist.json', root), 'utf8')));
const output = new URL('firmware-release-snapshot.json', root);
let previous = {entries: []}; try { previous = JSON.parse(await readFile(output, 'utf8')); } catch {}
let token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN;
if (!token && process.argv.includes('--use-gh-auth')) token = execFileSync('gh', ['auth', 'token'], {encoding:'utf8', stdio:['ignore','pipe','pipe']}).trim();
const now = new Date().toISOString(), entries = [], metadataCache=new Map();
for (const entry of list.entries) {
  const saved = previous.entries?.find(item => item.id === entry.id && item.upstreamRepo === entry.upstreamRepo);
  try {
    const request=releaseRequest(entry,token);
    let data=metadataCache.get(request.url);
    if(!data){
    const response = await fetch(request.url,{headers:request.headers,redirect:'error',signal:AbortSignal.timeout(20000)});
    if (!response.ok) {
      await response.body?.cancel();
      const limit = [403,429].includes(response.status);
      throw Object.assign(new Error(limit ? '发布源 API 查询限额已用尽；保留上次结果，稍后自动重试。' : response.status === 404 ? '此仓库暂未提供 latest release；保留已收录固件入口。' : `发布元信息 HTTP ${response.status}`), {status:response.status, retryAt:response.headers.get('x-ratelimit-reset')});
    }
    const reader = response.body.getReader(), chunks = []; let length = 0;
    try { while (true) { const {done,value}=await reader.read(); if(done)break; length+=value.byteLength;if(length>request.maxBytes)throw Error('Release metadata exceeds limit');chunks.push(value); } }
    finally { await reader.cancel(); }
    data=JSON.parse(Buffer.concat(chunks).toString('utf8'));metadataCache.set(request.url,data);
    }
    entries.push({id:entry.id,upstreamRepo:entry.upstreamRepo,checkedAt:new Date().toISOString(),sourceUrl:entry.metadataUrl,release:publicRelease(entry,data),error:null});
  } catch(error) {
    entries.push({id:entry.id,upstreamRepo:entry.upstreamRepo,checkedAt:saved?.checkedAt || null,sourceUrl:entry.metadataUrl,release:saved?.release || null,
      error:{status:error.status || null,message:error.status ? error.message : '发布元信息同步失败；保留上次结果。',attemptedAt:new Date().toISOString()}});
  }
}
await writeFile(output, JSON.stringify({schemaVersion:'1',generatedAt:now,policy:'Public release metadata only; firmware binaries stay at upstream sources. No client GitHub API token.',entries},null,2)+'\n');
console.log(`Release metadata: ${entries.filter(item=>!item.error).length}/${entries.length} updated; ${entries.filter(item=>item.error).length} retained/unavailable. No firmware downloaded.`);
