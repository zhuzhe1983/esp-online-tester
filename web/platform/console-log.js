// Serial events are transport chunks, not log entries. Copy from the retained,
// assembled port buffer so chunk boundaries and visual wrapping do not count.
export function stripAnsi(value) {
  const text=String(value); let result='',i=0;
  while(i<text.length){
    const code=text.charCodeAt(i);
    let kind=null;
    if(code===0x1b){
      i++; const next=text[i];
      if(next==='['){kind='csi';i++;}
      else if(next===']'){kind='osc';i++;}
      else if(['P','X','^','_'].includes(next)){kind='string';i++;}
      else {while(i<text.length&&text.charCodeAt(i)>=0x20&&text.charCodeAt(i)<=0x2f)i++;if(i<text.length&&text.charCodeAt(i)>=0x30&&text.charCodeAt(i)<=0x7e)i++;continue;}
    } else if(code===0x9b){kind='csi';i++;}
    else if(code===0x9d){kind='osc';i++;}
    else if([0x90,0x98,0x9e,0x9f].includes(code)){kind='string';i++;}
    if(kind==='csi'){
      while(i<text.length&&text.charCodeAt(i)>=0x20&&text.charCodeAt(i)<=0x3f)i++;
      if(i<text.length&&text.charCodeAt(i)>=0x40&&text.charCodeAt(i)<=0x7e)i++;
    } else if(kind){
      while(i<text.length){
        if(text.charCodeAt(i)===0x9c||(kind==='osc'&&text.charCodeAt(i)===7)){i++;break;}
        if(text.charCodeAt(i)===0x1b&&text[i+1]==='\\'){i+=2;break;}
        i++;
      }
    } else {if(code>=0x20&&!(code>=0x7f&&code<=0x9f)||code===9||code===10||code===13)result+=text[i];i++;}
  }
  return result;
}

export function prepareConsoleCopy(buffer,limit='all') {
  if(!['all',50,100].includes(limit))throw new RangeError('Unsupported console copy range');
  const lines=stripAnsi(buffer).replace(/\r\n?/g,'\n').split('\n');
  const records=[];for(let i=0;i<lines.length;i++)if(lines[i].trim())records.push(i);
  const selected=limit==='all'?records:records.slice(-limit);
  return {count:selected.length,text:selected.length?lines.slice(selected[0],selected.at(-1)+1).join('\n'):''};
}
