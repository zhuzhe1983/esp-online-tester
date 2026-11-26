// Exact product identities; physical Flash unknown remains an explicit simulation setting.
const profiles={'m5-acssr':'esp32c3','m5-dcssr':'esp32c3','m5-kmeter':'esp32c3','m5-switchc6':'esp32c6','m5-stamp-addon-c6':'esp32c6','m5-unit-gateway-h2':'esp32h2','m5-module-gateway-h2':'esp32h2','m5-cardkb2':'esp32c61','m5-tough-c5':'esp32c5'};
const aliases={'m5stack-u139':'m5-acssr','m5stack-u140':'m5-dcssr','m5stack-u133':'m5-kmeter','m5stack-k140':'m5-switchc6','m5stack-a172':'m5-stamp-addon-c6','m5stack-u195':'m5-unit-gateway-h2','m5stack-m141':'m5-module-gateway-h2','m5stack-u215':'m5-cardkb2','m5stack-k162':'m5-tough-c5'};
export const M5_EDGE_BOARD_CHIPS=Object.freeze({...profiles,...Object.fromEntries(Object.entries(aliases).map(([a,p])=>[a,profiles[p]]))});
