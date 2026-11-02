// The upstream image palette is a heuristic, not a protocol guarantee.
// Missing or ambiguous labels must leave the manual selection available.
const PALETTE=[[214,167,255],[255,156,125],[143,211,255],[128,224,178],[255,210,122],[255,158,175],[115,223,231],[183,199,225]];
const normal=s=>s.toLowerCase().replace(/[^a-z0-9]/g,'');
function distance(a,b){let row=Array.from({length:b.length+1},(_,i)=>i);for(let i=1;i<=a.length;i++){const next=[i];for(let j=1;j<=b.length;j++)next[j]=Math.min(next[j-1]+1,row[j]+1,row[j-1]+(a[i-1]!==b[j-1]));row=next;}return row[b.length];}
export function instructionTargets(instruction) {
  const match=/^(?:Click\s+in\s+order|请按顺序点击)\s*[:：]+\s*(.+)$/i.exec(instruction||'');
  return match?match[1].split(/[,，]/).map(s=>s.trim()).filter(Boolean):[];
}
export function matchInstructionTargets(instruction,candidates) {
  const targets=instructionTargets(instruction),used=new Set(),matches=[];
  for(const target of targets){
    const ranked=candidates.map((c,i)=>({...c,i,distance:distance(normal(target),normal(c.text))})).filter(c=>!used.has(c.i)&&c.confidence>=35&&c.distance<=(target.length>=4?1:0)).sort((a,b)=>a.distance-b.distance||b.confidence-a.confidence);
    // Similar text alternatives at different positions are ambiguous.
    const first=ranked[0],ambiguous=first&&ranked.slice(1).some(c=>c.distance===first.distance&&Math.hypot(c.x-first.x,c.y-first.y)>12);
    if(first&&!ambiguous){used.add(first.i);matches.push({target,text:first.text,confidence:first.confidence,x:Math.round(first.x),y:Math.round(first.y),approximate:first.distance>0});}
    else matches.push({target,missing:true});
  }
  const complete=targets.length>0&&matches.every(m=>!m.missing);
  return {complete,matches,points:complete?matches.map(({x,y})=>({x,y})):[]};
}

export function captchaTextMasks({data,width,height}) {
  if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>2048||height>2048||data.length!==width*height*4)throw Error('Invalid image dimensions');
  const results=[];
  for(const color of PALETTE){
    const mask=new Uint8Array(width*height),seen=new Uint8Array(mask.length),groups=[];
    for(let i=0;i<mask.length;i++){const at=i*4;if(color.reduce((sum,c,j)=>sum+(data[at+j]-c)**2,0)<22**2)mask[i]=1;}
    for(let i=0;i<mask.length;i++){
      if(!mask[i]||seen[i])continue;const points=[],queue=[i];seen[i]=1;let left=width,right=0,top=height,bottom=0;
      for(let at=0;at<queue.length;at++){const n=queue[at],x=n%width,y=Math.floor(n/width);points.push([x,y]);left=Math.min(left,x);right=Math.max(right,x);top=Math.min(top,y);bottom=Math.max(bottom,y);
        for(const [dx,dy] of [[1,0],[-1,0],[0,1],[0,-1]]){const nx=x+dx,ny=y+dy,j=ny*width+nx;if(nx>=0&&nx<width&&ny>=0&&ny<height&&!seen[j]&&mask[j]){seen[j]=1;queue.push(j);}}
      }
      if(groups.length<512&&points.length>=7&&points.length<=1000&&right-left>1&&bottom-top>2&&(right-left)/(bottom-top)<6&&(bottom-top)/(right-left)<8)groups.push({points,box:[left,top,right,bottom]});
    }
    const joined=new Set();let points=[];
    for(let a=0;a<groups.length;a++){
      if(joined.has(a))continue;const ids=[a],cluster=[];joined.add(a);
      for(let at=0;at<ids.length;at++){const g=groups[ids[at]];cluster.push(...g.points);for(let b=0;b<groups.length;b++){if(joined.has(b))continue;const h=groups[b],dx=Math.max(0,g.box[0]-h.box[2],h.box[0]-g.box[2]),dy=Math.max(0,g.box[1]-h.box[3],h.box[1]-g.box[3]);if(Math.hypot(dx,dy)<14){joined.add(b);ids.push(b);}}}
      if(cluster.length>points.length)points=cluster;
    }
    if(points.length<60)continue;
    const x=points.reduce((s,p)=>s+p[0],0)/points.length,y=points.reduce((s,p)=>s+p[1],0)/points.length;
    let xx=0,yy=0,xy=0;for(const p of points){xx+=(p[0]-x)**2;yy+=(p[1]-y)**2;xy+=(p[0]-x)*(p[1]-y);}const angle=.5*Math.atan2(2*xy,xx-yy),cos=Math.cos(angle),sin=Math.sin(angle);
    const rotated=points.map(p=>[(p[0]-x)*cos+(p[1]-y)*sin,-(p[0]-x)*sin+(p[1]-y)*cos]);
    let minX=Infinity,maxX=-Infinity,minY=Infinity,maxY=-Infinity;for(const [rx,ry] of rotated){minX=Math.min(minX,rx);maxX=Math.max(maxX,rx);minY=Math.min(minY,ry);maxY=Math.max(maxY,ry);}
    if(maxX-minX<20||maxX-minX>300||maxY-minY>70)continue;
    const w=Math.ceil(maxX-minX+15)*4,h=Math.ceil(maxY-minY+15)*4,pixels=new Uint8ClampedArray(w*h*4).fill(255);
    for(const [rx,ry] of rotated){const ox=Math.round((rx-minX+7)*4),oy=Math.round((ry-minY+7)*4);for(let dy=0;dy<4;dy++)for(let dx=0;dx<4;dx++){const at=((oy+dy)*w+ox+dx)*4;pixels[at]=pixels[at+1]=pixels[at+2]=0;}}
    results.push({data:pixels,width:w,height:h,x,y});
  }
  return results;
}

// Own the complete OCR worker tree, including slow initialization. Aborting the
// attempt terminates it instead of leaving a pending library worker behind.
export function readImagePoints(challenge,{signal,onProgress,onReady}={}) {
  return new Promise((resolve,reject)=>{
    const cancelled=()=>Object.assign(new Error('图片处理已取消。'),{code:'FETCH_ABORTED'});
    if(signal?.aborted){reject(cancelled());return;}
    const worker=new Worker(new URL('./m5burner-image-worker.js',import.meta.url),{type:'module'});let settled=false;
    const finish=(error,result)=>{if(settled)return;settled=true;signal?.removeEventListener('abort',abort);worker.terminate();error?reject(error):resolve(result);};
    const abort=()=>finish(cancelled());signal?.addEventListener('abort',abort,{once:true});
    worker.onmessage=({data})=>{
      if(settled)return;
      if(data.type==='progress')onProgress?.(data.message);
      else if(data.type==='ready')onReady?.();
      else if(data.type==='result')finish(null,data.result);
      else if(data.type==='error')finish(Object.assign(new Error('自动识图失败，可手动点选或换一张。'),{code:'CAPTCHA_IMAGE_ERROR'}));
    };
    worker.onerror=event=>{event.preventDefault();finish(Object.assign(new Error('自动识图暂时不可用，可手动点选或换一张。'),{code:'CAPTCHA_IMAGE_ERROR'}));};
    const {captchaImage,width,height,instruction}=challenge;worker.postMessage({captchaImage,width,height,instruction});
  });
}
