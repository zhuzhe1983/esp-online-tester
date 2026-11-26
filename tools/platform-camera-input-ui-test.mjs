import test from 'node:test';
import assert from 'node:assert/strict';
import {mountCameraInput} from '../web/platform/camera-input-ui.js';

// Minimal DOM surface: assertions observe the human controls, not source strings.
class Element {
  constructor(tag){this.tagName=tag;this.children=[];this.listeners=new Map();this.hidden=false;this.disabled=false;this.attributes={};}
  append(...children){for(const child of children){child.parentElement=this;this.children.push(child);}}
  setAttribute(name,value){this.attributes[name]=value;}
  addEventListener(type,handler){this.listeners.set(type,handler);}
  remove(){this.parentElement.children=this.parentElement.children.filter(child=>child!==this);}
}
const settle=()=>new Promise(resolve=>setImmediate(resolve));
function runtimeFixture(){
  let subscriber,hostPicture=false,status='configured',formats=['bmp','ppm'],calls=0;
  return {async capabilities(){return {camera:{hostPicture,formats}};},async observe(){return {status};},subscribe(listener){subscriber=listener;return()=>subscriber=null;},
    update(next){({hostPicture=hostPicture,status=status,formats=formats}=next);subscriber?.({type:'state',state:{status}});},
    get subscribed(){return Boolean(subscriber);},get calls(){return calls;},async setCameraPicture({bytes}){calls++;return {bytes:bytes.byteLength};}};
}

test('camera file control follows public state events through device switch and firmware lifecycle',async()=>{
  const original=globalThis.document;globalThis.document={createElement:tag=>new Element(tag)};
  try{
    const container=new Element('section'),runtime=runtimeFixture(),mounted=mountCameraInput({container,runtime});
    const panel=container.children[0],input=panel.children[2];await settle();
    assert.equal(panel.hidden,true);assert.equal(input.disabled,true);
    runtime.update({hostPicture:true,status:'configured',formats:['bmp','ppm','baseline-jpeg']});await settle();
    assert.equal(panel.hidden,false,'switching from a screen-only product reveals its camera control');assert.equal(input.disabled,true,'no firmware is loaded');assert.equal(input.accept,'.bmp,.ppm,.jpg,.jpeg');
    for(const status of ['loading','ready','running','paused','loading','ready']){
      runtime.update({status});await settle();assert.equal(input.disabled,!['ready','running','paused'].includes(status),status);
    }
    runtime.update({hostPicture:false,status:'configured'});await settle();assert.equal(panel.hidden,true,'switching away hides the old camera control');
    runtime.update({hostPicture:true,status:'ready',formats:['bmp','ppm']});await settle();assert.equal(panel.hidden,false);assert.equal(input.disabled,false);assert.equal(input.accept,'.bmp,.ppm','a sensor without JPEG does not offer JPEG');
    mounted.dispose();assert.equal(runtime.subscribed,false);assert.equal(container.children.length,0);
  }finally{globalThis.document=original;}
});

test('a selected image uses the same runtime method and restores the ready-state control',async()=>{
  const original=globalThis.document;globalThis.document={createElement:tag=>new Element(tag)};
  try{
    const container=new Element('section'),runtime=runtimeFixture();runtime.update({hostPicture:true,status:'ready'});
    const mounted=mountCameraInput({container,runtime}),panel=container.children[0],input=panel.children[2],state=panel.children[3];await settle();
    input.files=[{size:14,arrayBuffer:async()=>new ArrayBuffer(14)}];input.value='red.ppm';await input.listeners.get('change')();
    assert.equal(runtime.calls,1);assert.match(state.textContent,/14/);assert.equal(input.value,'');assert.equal(input.disabled,false);
    mounted.dispose();
  }finally{globalThis.document=original;}
});
