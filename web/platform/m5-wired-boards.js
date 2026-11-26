// Exact implemented Ethernet endpoints. A wired virtual peer is independent of Wi-Fi association.
export const M5_WIRED_PROTOCOLS=Object.freeze({'unit-poesp32':'native-rmii','m5stack-poe-p4':'native-rmii','m5stack-u121':'w5500-macraw','m5stack-u121-b':'w5500-macraw','m5stack-u121-b-v11':'w5500-macraw'});
export const m5WiredProtocol=board=>M5_WIRED_PROTOCOLS[board]||null;
