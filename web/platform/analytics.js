// Optional website statistics. This module is independent of the emulator runtime.
// Stream: ESP Online Tester · GitHub Pages. Enhanced measurement is disabled.
export const MEASUREMENT_ID='G-425HV3H2P0';
const origin='https://zhuzhe1983.github.io',base='/esp-online-tester/';
const pages=new Map([
  ['platform/','ESP Workbench'],['platform/index.html','ESP Workbench'],
  ['platform/embed.html','ESP Player'],['platform/agents.html','ESP Online Tester · Agent guide'],
  ['platform/integration.html','ESP Online Tester · Embed example'],
  ['emulator.html','ESP32 emulator demos'],['run.html','ESP32 emulator'],
]);

export function analyticsPage(href,referrer='') {
  let url;try{url=new URL(href);}catch{return null;}
  if(url.origin!==origin||!url.pathname.startsWith(base))return null;
  const relative=url.pathname.slice(base.length),title=pages.get(relative);if(!title)return null;
  // The legacy emulator redirects these links to run.html; count the destination only.
  if(relative==='emulator.html'&&/[?&](wasm|fw)(=|&|$)/.test(url.search))return null;
  let source='';try{const ref=new URL(referrer);if(['https:','http:'].includes(ref.protocol))source=ref.origin+'/';}catch{}
  return {page_location:origin+base+(relative==='platform/index.html'?'platform/':relative),page_title:title,page_referrer:source};
}

export function installAnalytics(win,doc) {
  const page=analyticsPage(win.location.href,doc.referrer);
  if(!page||win.__espWebsiteAnalytics||win.navigator?.globalPrivacyControl===true||win.navigator?.doNotTrack==='1'||win.doNotTrack==='1'||win[`ga-disable-${MEASUREMENT_ID}`])return false;
  // Do not add tracking to third-party websites embedding the simulator.
  if(win.top!==win.self)return false;
  win.__espWebsiteAnalytics=true;
  win.dataLayer=win.dataLayer||[];
  win.gtag=win.gtag||function(){win.dataLayer.push(arguments);};
  win.gtag('js',new Date());
  win.gtag('config',MEASUREMENT_ID,{
    ...page,send_page_view:false,allow_google_signals:false,allow_ad_personalization_signals:false,
    cookie_domain:'zhuzhe1983.github.io',cookie_path:base,cookie_prefix:'eot',cookie_flags:'SameSite=Lax;Secure',
  });
  win.gtag('event','page_view',{...page,send_to:MEASUREMENT_ID});
  const script=doc.createElement('script');script.async=true;script.referrerPolicy='strict-origin';
  script.src=`https://www.googletagmanager.com/gtag/js?id=${MEASUREMENT_ID}`;
  doc.head.append(script);
  return true;
}

if(typeof window!=='undefined'&&typeof document!=='undefined')installAnalytics(window,document);
