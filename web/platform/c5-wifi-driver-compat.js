// Explicit, product-scoped execution selection. This never enables itself.
export const C5_WIFI_DRIVER_COMPAT_PROFILE = 'waveshare-lcd273-idf553-offline-v1';
export const C5_WIFI_DRIVER_COMPAT_BOARD = 'waveshare-c5-lcd273';
export const C5_WIFI_DRIVER_COMPAT_PRODUCT_IDS = Object.freeze(['waveshare-esp32-c5-lcd-2-73','waveshare-esp32-c5-lcd-2-73-en']);
export const C5_WIFI_DRIVER_COMPAT_NOTICE = '离线模式，无热点/网络连接；Wi-Fi配置仅本次运行保留。';
export const C5_WIFI_DRIVER_COMPAT_RELOAD_NOTICE = '切换模式会停止当前运行；需要重新加载固件后启动。';

export const c5WifiDriverCompatAllowed = (product, profile) => product?.runtime?.board === C5_WIFI_DRIVER_COMPAT_BOARD && profile === C5_WIFI_DRIVER_COMPAT_PROFILE;

export function c5WifiDriverCompatIssue(scene, product) {
  const profile = scene?.c5WifiDriverCompat;
  if (profile === undefined) return null;
  if (profile !== C5_WIFI_DRIVER_COMPAT_PROFILE) return {code:'C5_WIFI_DRIVER_COMPAT_PROFILE',path:'/c5WifiDriverCompat',message:'Unknown C5 Wi-Fi driver compatibility profile; omit the field for strict hardware simulation.'};
  if (!c5WifiDriverCompatAllowed(product, profile)) return {code:'C5_WIFI_DRIVER_COMPAT_BOARD',path:'/c5WifiDriverCompat',message:'This offline Wi-Fi driver profile requires board waveshare-c5-lcd273.'};
  if (scene.firmware?.bootMode === 'app-direct') return {code:'C5_WIFI_DRIVER_COMPAT_BOOT_MODE',path:'/firmware/bootMode',message:'The offline Wi-Fi driver profile requires ROM startup; app-direct is not supported.'};
  // A local Files selection has firmware:null. The loader must check the actual
  // merged image/profile/ROM before creating an enabled worker.
  return null;
}

export function withC5WifiDriverCompat(scene, profile) {
  if (profile !== undefined && profile !== '' && profile !== C5_WIFI_DRIVER_COMPAT_PROFILE) throw Object.assign(new Error('Unknown C5 Wi-Fi driver compatibility selection.'), {code:'C5_WIFI_DRIVER_COMPAT_PROFILE'});
  const next = structuredClone(scene);
  if (profile) next.c5WifiDriverCompat = profile;
  else delete next.c5WifiDriverCompat;
  return next;
}
