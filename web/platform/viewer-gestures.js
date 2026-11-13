// A second finger cancels the one-finger firmware gesture before zooming.
// After lifting one finger, consume the remainder until both are released.
export function createPinchGesture({onStart=()=>{},onScale=()=>{}}={}){
  const points=new Map();let pinching=false,distance=0;
  const span=()=>{const [a,b]=[...points.values()];return a&&b?Math.hypot(a.x-b.x,a.y-b.y):0;};
  return {
    get active(){return pinching;},
    down(id,x,y){points.set(id,{x,y});if(points.size===2&&!pinching){pinching=true;distance=span();onStart();}return pinching;},
    move(id,x,y){if(!points.has(id))return false;points.set(id,{x,y});if(!pinching)return false;const next=span();if(points.size===2&&distance>0&&next>0)onScale(next/distance);distance=next;return true;},
    up(id){const consumed=pinching&&points.has(id);points.delete(id);distance=span();if(!points.size)pinching=false;return consumed;},
    reset(){points.clear();pinching=false;distance=0;},
  };
}
