// Only the site operator configures a relay; it is never read from shared scenes.
let configuredRelay='';
export function firmwareRelayTarget(value) {
  try {
    const url=new URL(value);
    if(url.protocol!=='https:'||url.port||url.username||url.password||url.search||url.hash||url.href.length>2048)return null;
    const github=url.hostname==='github.com'&&/^\/[\w.-]+\/[\w.-]+\/releases\/download\/[^/]+\/[^/]+\.bin$/.test(url.pathname);
    const mesh=url.hostname==='release.meshtastic.org'&&/^\/[0-9][A-Za-z0-9.-]{0,79}\/firmware-[A-Za-z0-9.-]+\.bin$/.test(url.pathname);
    return github||mesh?url.href:null;
  } catch {return null;}
}
export function firmwareRelayBase(value) {
  if(!value)return '';
  const url=new URL(value);
  if(url.username||url.password||url.search||url.hash||url.pathname!=='/'||url.protocol!=='https:'&&!(url.protocol==='http:'&&['localhost','127.0.0.1','[::1]'].includes(url.hostname)))throw new Error('Firmware relay requires an HTTPS origin (HTTP is allowed only for localhost).');
  return url.origin;
}
export function configureFirmwareRelay(value) {configuredRelay=firmwareRelayBase(value);}
export function firmwareDownloadUrl(value,sha256,relay=configuredRelay) {
  if(!relay||!firmwareRelayTarget(value)||!/^[a-f0-9]{64}$/i.test(sha256||''))return value;
  const url=new URL('/firmware',firmwareRelayBase(relay));url.searchParams.set('url',value);url.searchParams.set('sha256',sha256.toLowerCase());return url.href;
}
