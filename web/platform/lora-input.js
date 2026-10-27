// Explicit digital peer input. Never infer the peer's config from the receiver:
// mismatched frequency/modulation/sync is a useful test, not an auto-repair case.
export function validateLoRaPacket(packet) {
  const fail=()=>{throw Object.assign(new Error('LoRa input needs a valid config, up to 255 bytes as payloadHex, rssiDbm (-127…0), snrQuarterDb (-128…127), and crcError.'),{code:'INVALID_LORA_PACKET'});};
  const integer=(n,min,max)=>Number.isSafeInteger(n)&&n>=min&&n<=max;
  if(!packet||typeof packet!=='object')fail();const c=packet.config;
  if(!c||!integer(c.frequency,1,0xffffffff)||!integer(c.sf,5,12)||![0,8,1,9,2,10,3,4,5,6].includes(c.bandwidth)||!integer(c.codingRate,1,4)||!integer(c.syncWord,0,65535)||['implicitHeader','crc','invertIq'].some(k=>typeof c[k]!=='boolean'))fail();
  if(typeof packet.payloadHex!=='string'||packet.payloadHex.length>510||!/^(?:[a-f\d]{2})*$/i.test(packet.payloadHex)||!integer(packet.rssiDbm,-127,0)||!integer(packet.snrQuarterDb,-128,127)||typeof packet.crcError!=='boolean')fail();
  return {config:{frequency:c.frequency,sf:c.sf,bandwidth:c.bandwidth,codingRate:c.codingRate,syncWord:c.syncWord,implicitHeader:c.implicitHeader,crc:c.crc,invertIq:c.invertIq},payloadHex:packet.payloadHex.toLowerCase(),rssiDbm:packet.rssiDbm,snrQuarterDb:packet.snrQuarterDb,crcError:packet.crcError};
}
