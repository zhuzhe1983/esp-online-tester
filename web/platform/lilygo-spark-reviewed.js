// Official, commit-pinned images independently matched to the modeled revision.
// All nine payloads match Spark's digest and pass validateMergedImage for the
// product's chip and flash capacity. A successful boot is checked separately.
const reviewed = [
  {deviceId:'t-camera-plus-s3', sha256:'25b77114190ffef9b1c71532eb919fbea4e113076e9f837c0803835ad52acb0a',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/T-CameraPlus-S3/d2d6d36194569706c4a48b54d5af7b7d81f6592c/firmware/%5BT-CameraPlus-S3%5D%5BLvgl_UI%5D/%5BT-CameraPlus-S3_V1.2%5D%5BLvgl_UI%5D_firmware_202504081446.bin'},
  {deviceId:'t-can485', sha256:'926818a8d0edd05759be3362d04534bfe6c207b97dcde8dc89b21e567f31ecc6',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/T-CAN485/e6f92dab557b938226741d989124acebbe4fbf94/firmware/%5BT-CAN485_V1.0%5D%5BOriginal_Test%5D_firmware_V1.0.0.bin'},
  {deviceId:'t-circle-s3', sha256:'44003407ef2d2c5c4d370756ccf5588f618835d467dc2bbc34afe2527483edd7',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Circle-S3/86bdee8bd9b2a6ce0f4f4f8757ce82d6a3f43a0a/firmware/%EF%BC%88V1.1%E7%89%88%E6%9C%AC%E4%BF%AE%E6%94%B9%E9%BA%A6%E5%85%8B%E9%A3%8E%E5%9E%8B%E5%8F%B7%EF%BC%89%5BT-Circle-S3_V1.1%5D%5BOriginal_Test%5D_firmware_202502141426.bin'},
  {deviceId:'t-deck-pro', sha256:'35b60973f03e930888242d4fb2a3bcb74f74e833fb73e484de32996a0044d79d',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/T-Deck-Pro/e3246046972716edd5f264a6047ba3572a851b23/firmware/H693_factory_v1.2_20250412_fix.bin'},
  {deviceId:'t-glass', sha256:'0813ea47fd5955b0877e06e1b8d810c5cce26e9a7fad75f3786648a66481de54',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/LilyGO-T-Wristband-and-T-Glass/99ae22477daeaec55714f6b31e0219407dd52439/firmware/T-GlassV2Factory_240927.bin'},
  {deviceId:'t-hmi', sha256:'40d17179c4a53518a168ba384613b4901cff925a05f344895c22af3eb15ada6a',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/T-HMI/7b52eed0a1fa9611a12814a8a922b78a946a4316/firmware/HMI_factory_251202.bin'},
  {deviceId:'t-sim7000g', sha256:'35baa1c6f74405ad7c80a4ab8da7357a3a06d1a0c848b0ca92b03463f05c7ffd',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/LilyGO-T-SIM7000G/3e506bd5cfc1871877fa42f6534a6dcd5e376b8e/firmware/GPS_Decoder.bin'},
  {deviceId:'t-sim7000g', sha256:'47ea74f31390f9969e6b674d72184c25f5ecf071f882d90d0a5a99dbd41bfbba',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/LilyGO-T-SIM7000G/3e506bd5cfc1871877fa42f6534a6dcd5e376b8e/firmware/GPS_RAW_DATA.bin'},
  {deviceId:'t-sim7000g', sha256:'07ec0c82eab61abceb0433137d29c1f52bfd3915e9878b84fd664daff9fee88f',
    url:'https://raw.githubusercontent.com/Xinyuan-LilyGO/LilyGO-T-SIM7000G/3e506bd5cfc1871877fa42f6534a6dcd5e376b8e/firmware/T-sim7000X-AllFunctions.bin'},
];

const pinnedOfficialBin=/^https:\/\/raw\.githubusercontent\.com\/xinyuan-lilygo\/[a-z0-9_.-]+\/[a-f0-9]{40}\/[^?#]+\.bin$/i;
export function createSparkReview(catalog={}) {
  const byDevice=new Map();
  function add(deviceId,productId,source) {
    if(!byDevice.has(deviceId))byDevice.set(deviceId,new Map());
    byDevice.get(deviceId).set(source.sha256,{productId,source});
  }
  for(const {deviceId,url,sha256} of reviewed)add(deviceId,`lilygo-${deviceId}`,{url,sha256,bootMode:'rom'});

  // The local catalog also carries reviewed official images, including files
  // moved or renamed since Spark indexed their bytes. Never inherit a sibling's
  // firmware merely because its upstream product or family name looks similar.
  const products=new Map(),owners=new Map();
  for(const product of catalog.products||[]) {
    const id=product.providers?.['lilygo-spark']?.productId;
    if(product.manufacturer?.toLowerCase()!=='lilygo'||!id)continue;
    products.set(product.id,id);
    owners.set(id,owners.has(id)?null:product.id);
  }
  for(const item of catalog.firmwares||[]) {
    const source=item.source;
    if(item.publisher!=='LILYGO'||!['trial','available'].includes(item.status)||!['factory','demo'].includes(item.role))continue;
    if(!pinnedOfficialBin.test(source?.url)||!/^[a-f0-9]{64}$/.test(source.sha256))continue;
    for(const productId of item.productIds||[]) {
      const deviceId=products.get(productId);
      if(deviceId&&owners.get(deviceId)===productId)add(deviceId,productId,source);
    }
  }
  return (deviceId,sha256)=>{
    const entry=byDevice.get(deviceId)?.get(sha256);
    if(!entry||owners.has(deviceId)&&owners.get(deviceId)!==entry.productId)return null;
    return entry;
  };
}
