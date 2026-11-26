// Network interface names describe the implemented board transport, not a lease
// or a successful external connection. Addresses still require real leaseReady.
export function networkInterfaceView(state,t=value=>value) {
  const wired=['native-rmii','w5500-macraw'].includes(state.primaryInterface),softApAvailable=!wired&&state.softApSupported!==false;
  const primaryName=t(wired?'以太网':'STA'),a=state.interfaces?.[0],b=state.interfaces?.[1],address=a?.leaseReady?a.guestAddress:t('未取得地址');
  return {wired,softApAvailable,primaryName,serviceOption:t(wired?'以太网应用服务':'STA 应用服务'),
    description:t(wired?'当前板型使用有线以太网，可连接云端中继或本地助手。':softApAvailable?'固件 SoftAP 配网页可直接在浏览器内访问；关闭自动配网即可手动操作。外网默认经云端中继访问，也可改成你自建的网络助手地址。':'当前板型提供 STA 网络中继，不支持固件 SoftAP 配网页或自动配网。可通过固件支持的方式设置网络，再连接云端中继或本地助手。'),
    addresses:wired?`${t('虚拟网关')} · ${primaryName}: ${address}`:`${t('虚拟路由器：')}${state.router.ssid} · STA: ${address} · SoftAP: ${!softApAvailable?t('不支持'):state.softAp?.ssid||t('未发现')}${b?.leaseReady?` / ${b.guestAddress}`:''}`};
}
