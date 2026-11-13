import * as THREE from './vendor/three.module.min.js';
import {createStickerGeometry} from './sticker-geometry.js';
import {readStickers,saveStickers,prepareSticker,STICKER_LIMITS,validateStickerCollection} from './sticker-store.js';

export function createStickerController({draw=()=>{},onChange=()=>{}}={}){
  let productId=null,meshes=[],items=[],selected=null,placing=false,generation=0,persistent=true,message='',timer=null,disposed=false,busy=false,saving=0,saveQueue=Promise.resolve();
  const resources=new Map(),pending=new Set();
  const state=()=>({productId,items:items.map(({blob,...item})=>({...item,thumbnail:resources.get(item.id)?.url,rendered:!!resources.get(item.id)?.mesh})),selected,placing,persistent,message,busy,saving:saving>0,available:meshes.length>0});
  const emit=()=>{if(!disposed)onChange(state());};
  function release(){clearTimeout(timer);pending.clear();for(const r of resources.values()){r.mesh?.removeFromParent();r.mesh?.geometry.dispose();r.mesh?.material.dispose();r.texture?.dispose();r.bitmap?.close();URL.revokeObjectURL(r.url);}resources.clear();}
  async function renderItem(item){
    const r=resources.get(item.id),target=meshes.find(mesh=>mesh.name===item.anchor?.part);if(!r)return;const revision=r.revision=(r.revision||0)+1;
    r.mesh?.removeFromParent();r.mesh?.geometry.dispose();r.mesh?.material.dispose();r.mesh=null;
    if(!target||!item.anchor)return;
    const token=generation;
    if(!r.texture){
      if(!r.loading)r.loading=createImageBitmap(item.blob,{imageOrientation:'flipY'}).then(bitmap=>{if(disposed||token!==generation||resources.get(item.id)!==r){bitmap.close();return;}r.bitmap=bitmap;r.texture=new THREE.Texture(bitmap);r.texture.flipY=false;r.texture.colorSpace=THREE.SRGBColorSpace;r.texture.needsUpdate=true;}).finally(()=>{r.loading=null;});
      await r.loading;if(disposed||token!==generation||!r.texture||resources.get(item.id)!==r||r.revision!==revision)return;
      // A newer slider or anchor value may have arrived while decoding the image.
      item=items.find(value=>value.id===item.id);if(!item?.anchor)return;
    }
    const actual=meshes.find(mesh=>mesh.name===item.anchor.part);if(!actual)return;
    actual.geometry.computeBoundingBox();const size=actual.geometry.boundingBox.getSize(new THREE.Vector3()),extent=Math.max(size.x,size.y,size.z);
    const geometry=createStickerGeometry(actual,item,extent),material=new THREE.MeshStandardMaterial({map:r.texture,roughness:.76,metalness:0,transparent:true,alphaTest:.01,depthWrite:false,polygonOffset:true,polygonOffsetFactor:-3,polygonOffsetUnits:-8});
    if(actual.material.map&&actual.material.alphaTest>0){
      material.onBeforeCompile=shader=>{shader.uniforms.stickerSupportMap={value:actual.material.map};shader.vertexShader='attribute vec2 surfaceUv; varying vec2 vStickerSupportUv;\n'+shader.vertexShader;shader.vertexShader=shader.vertexShader.replace('#include <begin_vertex>','#include <begin_vertex>\nvStickerSupportUv=surfaceUv;');shader.fragmentShader='uniform sampler2D stickerSupportMap; varying vec2 vStickerSupportUv;\n'+shader.fragmentShader;shader.fragmentShader=shader.fragmentShader.replace('#include <map_fragment>','#include <map_fragment>\nif(texture2D(stickerSupportMap,vStickerSupportUv).a<0.5) discard;');};
      material.customProgramCacheKey=()=> 'private-sticker-support-alpha-v1';
    }
    const decal=new THREE.Mesh(geometry,material);decal.name=`user-sticker:${item.id}`;decal.userData.sticker=true;decal.userData.framingIgnore=true;decal.userData.ignoreRaycast=true;decal.raycast=()=>{};decal.renderOrder=3+items.indexOf(item);actual.add(decal);r.mesh=decal;draw();emit();
  }
  function render(){clearTimeout(timer);timer=null;pending.clear();for(const item of items)void renderItem(item).catch(()=>{message='有一张贴纸无法显示，请重新添加。';emit();});draw();}
  function persist(nextItems=items){
    const scope=productId,snapshot=structuredClone(nextItems);saving++;emit();saveQueue=saveQueue.then(()=>saveStickers(scope,snapshot)).then(saved=>{if(scope===productId&&!disposed){persistent=saved;}}).finally(()=>{saving--;emit();});return saveQueue;
  }
  async function setModel(id,nextMeshes=[]){
    const token=++generation;release();productId=id;meshes=nextMeshes.filter(mesh=>mesh.isMesh&&mesh.name&&!mesh.userData.screen&&!mesh.userData.control&&!mesh.userData.encoder&&mesh.userData.ledIndex===undefined);items=[];selected=null;placing=false;message='';busy=false;emit();
    if(!id||!meshes.length)return;
    await saveQueue;const loaded=await readStickers(id);if(disposed||token!==generation)return;
    items=loaded.items;persistent=loaded.persistent;selected=items[0]?.id||null;
    for(const item of items)resources.set(item.id,{url:URL.createObjectURL(item.blob)});
    render();emit();
  }
  async function addFiles(files){
    if(busy||!meshes.length)return;const candidates=[...files],token=generation;
    if(items.length+candidates.length>STICKER_LIMITS.count)throw Error(`每款设备最多保存 ${STICKER_LIMITS.count} 张贴纸。`);
    busy=true;message='正在处理图片…';emit();const prepared=[];
    try{
      for(const file of candidates){prepared.push(await prepareSticker(file));if(token!==generation)return false;}
      validateStickerCollection([...items,...prepared]);items.push(...prepared);for(const item of prepared)resources.set(item.id,{url:URL.createObjectURL(item.blob)});
      selected=prepared[0]?.id||selected;placing=!!prepared.length;message=`已添加 ${prepared.length} 张；点选模型外壳放置当前贴纸。`;await persist();return true;
    }finally{if(token===generation){busy=false;emit();}}
  }
  function select(id){if(busy)return;if(!items.some(item=>item.id===id))return;selected=id;placing=false;message='';emit();}
  function place(value=true){if(busy)return;if(!selected)return;placing=value;message=value?'点选可见的外壳表面；可先关闭面板，拖动模型换到背面。':'';emit();}
  function placeAt(hit){
    if(!placing||busy)return false;
    const item=items.find(item=>item.id===selected),mesh=hit?.object;
    if(!item||!meshes.includes(mesh)||!hit.face){message='请选择外壳表面，屏幕、按键和接口不能贴图。';emit();return false;}
    item.anchor={part:mesh.name,point:mesh.worldToLocal(hit.point.clone()).toArray(),normal:hit.face.normal.clone().normalize().toArray()};item.x=item.y=0;placing=false;message='已贴到外壳。可调整大小、角度，或重新定位。';render();void persist();emit();return true;
  }
  function update(patch,{save=false}={}){
    const index=items.findIndex(item=>item.id===selected);if(index<0||busy)return;
    const next={...items[index],...Object.fromEntries(Object.entries(patch).filter(([key])=>['size','rotation','x','y'].includes(key)))};
    validateStickerCollection(items.map((item,i)=>i===index?next:item));items[index]=next;
    pending.add(next.id);clearTimeout(timer);timer=setTimeout(()=>{const changed=[...pending];pending.clear();for(const item of items.filter(item=>changed.includes(item.id)))void renderItem(item).catch(()=>{message='贴纸无法显示，请重新添加。';emit();});},60);if(save)void persist();emit();
  }
  async function remove(){
    const id=selected,index=items.findIndex(item=>item.id===id),token=generation;if(index<0||busy)return;busy=true;emit();
    try{
      const next=items.filter(item=>item.id!==id);await persist(next);if(token!==generation)return;
      const r=resources.get(id);r?.mesh?.removeFromParent();r?.mesh?.geometry.dispose();r?.mesh?.material.dispose();r?.texture?.dispose();r?.bitmap?.close();if(r)URL.revokeObjectURL(r.url);resources.delete(id);items=next;
      selected=items[Math.min(index,items.length-1)]?.id||null;placing=false;message='';
    }finally{if(token===generation){busy=false;draw();emit();}}
  }

  return {state,setModel,addFiles,select,place,placeAt,update,remove,accepts(mesh){return meshes.includes(mesh);},get placing(){return placing;},dispose(){disposed=true;generation++;release();}};
}
