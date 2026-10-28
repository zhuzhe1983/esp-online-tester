async (page) => {
 const wait=async predicate=>{const end=Date.now()+60000;while(Date.now()<end){if(await page.evaluate(predicate))return;await page.waitForTimeout(50);}throw Error('PIR mini condition timeout');};
 await page.goto(new URL('/web/platform/',page.url()).href);await wait(()=>!!window.espTester);
 await page.evaluate(async()=>{const api=window.espTester,s=(await api.observe()).scene;s.productId='m5stack-stopwatch';s.firmware=null;s.accessories=[{id:'pir',catalogId:'m5stack-unit-pir-u004',parameters:{motion:false},wiring:{mode:'physical',interfaceId:'grove'}}];const url=new URL('/output/pir-browser-manifest.json',location.href).href;const bytes=await(await fetch(url)).arrayBuffer();const sha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(v=>v.toString(16).padStart(2,'0')).join('');s.firmware={manifestUrl:url,sha256};await api.configure(s);});
 const share=await page.evaluate(()=>espTester.share({mode:'embed',autostart:true,includeRecording:false}));await page.goto(share);
 await wait(async()=>{if(!window.espTester)return false;const s=await espTester.observe();return s.status==='running'&&/U004_READ G11=0 G10=1 US=\d+\r?\n/.test(s.serialByPort?.uart0||'');});
 await page.evaluate(()=>espTester.input({type:'accessory-motion',accessoryId:'pir',motion:true}));
 await wait(async()=>/U004_READ G11=1 G10=1 US=\d+\r?\n/.test((await espTester.observe()).serialByPort?.uart0||''));
 await page.evaluate(()=>espTester.input({type:'accessory-motion',accessoryId:'pir',motion:false}));
 await wait(async()=>/U004_READ G11=1 G10=1 US=\d+\r?\n[\s\S]*U004_READ G11=0 G10=1 US=\d+\r?\n/.test((await espTester.observe()).serialByPort?.uart0||''));
 await page.evaluate(()=>espTester.pause());await page.screenshot({path:'output/playwright/pir-mini-final.png'});
 return await page.evaluate(async()=>{const s=await espTester.observe();return {status:s.status,productId:s.product.id,view:s.scene.view,inputs:s.accessoryInputs,serialPort:'uart0',serial:(s.serialByPort.uart0||'').split(/\r?\n/).filter(l=>l.startsWith('U004_'))};});
}
