// Exact classic ESP32 camera board identities; not a regex matching every M5 SKU.
// Hardware sources and revision choices: research/m5-classic-camera-runtime.json.
export const M5_CLASSIC_CAMERA_BOARDS=Object.freeze([
  'm5stack-u017','m5stack-u017-b','m5stack-u017-pcba','m5stack-u038','m5stack-u037',
  'm5stack-u007','m5stack-u109','m5stack-u109-x','m5stack-u109-b',
  'm5stack-u121','m5stack-u121-b','m5stack-u121-b-v11',
  'm5stack-u082','m5stack-u082-x','m5stack-u082-f',
]);
export const isM5ClassicCameraBoard=board=>M5_CLASSIC_CAMERA_BOARDS.includes(board);
