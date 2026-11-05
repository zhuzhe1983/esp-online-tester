#!/usr/bin/env node
// Development-only source audit. Pass an installed Acorn module path; the site
// and CI tests have no parser dependency. Templates are samples, not proof that
// every runtime interpolation is translated; use the browser fixture as well.
import fs from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {setLocale,t} from '../web/platform/i18n.js';
import en from '../web/platform/locales/en.js';

if(!process.argv[2])throw Error('Usage: node tools/audit-platform-i18n.mjs <path-to-acorn.mjs>');
const {parse}=await import(pathToFileURL(resolve(process.argv[2])).href);
const root=new URL('../web/platform/',import.meta.url),han=/[\u4e00-\u9fff]/;
const files=fs.readdirSync(root).filter(name=>name.endsWith('.js')&&!/^(?:review|model-review|gps-model-review|papermono-model-review|i18n)/.test(name));
const missing=new Map(),composed=new Map(),templates=[];setLocale('en');
for(const file of files) {
 const ast=parse(fs.readFileSync(new URL(file,root),'utf8'),{ecmaVersion:'latest',sourceType:'module',locations:true});
 function walk(node) {
  if(!node||typeof node!=='object')return;
  if(node.type==='Literal'&&typeof node.value==='string'&&han.test(node.value)) {
   const translated=t(node.value),target=han.test(translated)?missing:!Object.hasOwn(en,node.value.trim())?composed:null;
   if(target){let row=target.get(node.value);if(!row){row={text:node.value,english:translated,sources:[]};target.set(node.value,row);}row.sources.push(`${file}:${node.loc.start.line}`);}
  }
  if(node.type==='TemplateLiteral') {
   const sample=node.quasis.map((part,index)=>(part.value.cooked??part.value.raw)+(index<node.expressions.length?`{${index}}`:'')).join('');
   if(han.test(sample)&&han.test(t(sample)))templates.push({sample,source:`${file}:${node.loc.start.line}`});
  }
  for(const value of Object.values(node))if(Array.isArray(value))value.forEach(walk);else if(value&&typeof value==='object')walk(value);
 }
 walk(ast);
}
console.log(JSON.stringify({modules:files.length,missingLiterals:[...missing.values()],composedLiterals:[...composed.values()],missingTemplateSamples:templates,scope:'Root platform modules; excludes diagnostic review pages, vendors and catalog prose. Runtime combinations and protected data require browser checks.'},null,2));
