// Keep the vendored SLIP framing, but always release Web Serial's writer lock:
// esptool-js v0.7.0 Transport.write leaves it held when writer.write rejects.
export function createFlashTransport(Transport,port) {
  const transport=new Transport(port,false);
  transport.write=async data=>{
    const bytes=transport.slipWriter(data),device=transport.device || port;
    if(!device.writable)throw Object.assign(new Error('串口已断开。'),{code:'DEVICE_DISCONNECTED'});
    const writer=device.writable.getWriter();
    try {await writer.write(bytes);} finally {writer.releaseLock();}
  };
  return transport;
}

// Await a bounded close without cancelling it. The caller retains the port lock
// until `closing` really resolves; a late rejection is observed by Promise.race.
export async function waitForFlashClose(closing,timeoutMs=5000) {
  let timer;
  try {
    await Promise.race([closing,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Object.assign(new Error('串口关闭超时；请断开设备，确认串口释放后再重试。'),{code:'SERIAL_CLOSE_FAILED'})),timeoutMs);})]);
  } finally {clearTimeout(timer);}
}
