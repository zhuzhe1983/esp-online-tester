// MicroPython's documented raw REPL: interrupt, Ctrl-A, source, Ctrl-D.
// USB input is chunked and guest time advances between chunks; bytes are never
// inserted directly into guest RAM or the TM1680 framebuffer.
export function createCoreMatrixRepl(runtime, {timeoutMs=120000}={}) {
  const wait=async predicate=>{
    const end=Date.now()+timeoutMs;
    while(Date.now()<end) {
      const state=await runtime.observe();
      if(state.status==='error')throw Error(JSON.stringify(state.lastError));
      if(predicate(state))return state;
      await new Promise(resolve=>setTimeout(resolve,20));
    }
    throw Error('CoreMatrix guest did not reach the expected raw-REPL condition.');
  };
  const send=data=>runtime.input({type:'serial',port:'usb',data});
  const advance=async seconds=>{const at=(await runtime.observe()).stats.time;return wait(state=>state.stats.time>=at+seconds);};
  return {
    wait,advance,
    async enter() {
      const start=(await runtime.observe()).serial.length;
      await send('\r\x03\x03');await advance(.15);await send('\x01');
      await wait(state=>state.serial.slice(start).includes('raw REPL; CTRL-B to exit\r\n>'));
    },
    async exec(source) {
      const start=(await runtime.observe()).serial.length;
      for(let offset=0;offset<source.length;offset+=64) {await send(source.slice(offset,offset+64));await advance(.002);}
      await send('\x04');
      const state=await wait(state=>state.serial.slice(start).includes('\x04\x04>'));
      const result=state.serial.slice(start),ok=result.indexOf('OK'),eot=result.indexOf('\x04',ok+2);
      if(ok<0||eot<0)throw Error('Invalid raw-REPL response.');
      const stdout=result.slice(ok+2,eot),stderr=result.slice(eot+1,result.indexOf('\x04',eot+1));
      if(stderr)throw Error('Guest MicroPython rejected the fixture: '+stderr.slice(0,1200));
      return stdout;
    },
  };
}
