// Public virtual-router credentials. These do not configure the host's Wi-Fi.
export const DEFAULT_VIRTUAL_ROUTER=Object.freeze({ssid:'esp32_online_tester',password:'rilakkuma_cool'});
export const SIMULATOR_UIFLOW2_PROFILE=Object.freeze({
  id:'builtin-uiflow2-simulator',name:'ESP Online Tester · UIFlow2 网络启动',
  type:'uiflow2',server:'uiflow2.m5stack.com',...DEFAULT_VIRTUAL_ROUTER,bootOption:2,builtin:true,
});
