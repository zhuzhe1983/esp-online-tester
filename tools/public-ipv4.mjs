import {isIP} from 'node:net';
const ipNumber=bytes=>bytes.reduce((n,b)=>n*256+b,0);
// Conservative IANA special-purpose exclusions plus multicast. Checked 2026-10-28:
// https://www.iana.org/assignments/iana-ipv4-special-registry/
const SPECIAL=['0.0.0.0/8','10.0.0.0/8','100.64.0.0/10','127.0.0.0/8','169.254.0.0/16','172.16.0.0/12','192.0.0.0/24','192.0.2.0/24','192.31.196.0/24','192.52.193.0/24','192.88.99.0/24','192.168.0.0/16','192.175.48.0/24','198.18.0.0/15','198.51.100.0/24','203.0.113.0/24','224.0.0.0/4','240.0.0.0/4'].map(cidr=>{const [ip,bits]=cidr.split('/'),size=2**(32-Number(bits));return [ipNumber(ip.split('.').map(Number)),size];});
export function publicIpv4(ip) {
  const bytes=typeof ip==='string'?(isIP(ip)===4?ip.split('.').map(Number):[]):Array.from(ip??[]);
  if(bytes.length!==4||bytes.some(b=>!Number.isInteger(b)||b<0||b>255))return false;
  const value=ipNumber(bytes);return !SPECIAL.some(([start,size])=>value>=start&&value<start+size);
}
