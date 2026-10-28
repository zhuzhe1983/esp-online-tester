import {mountFirmwareHub} from './m5burner-ui.js';
import {createLilygoSparkClient,matchSparkDevice,sparkFirmware,sparkHistoryFirmware,LILYGO_SPARK_HOME} from './lilygo-spark.js';
export function mountLilygoSpark(options) {
  return mountFirmwareHub({...options,client:options.client||createLilygoSparkClient(),provider:{name:'LILYGO Spark',prefix:'spark',detailsLabel:'查看项目来源 ↗',home:LILYGO_SPARK_HOME,match:matchSparkDevice,firmware:sparkFirmware,history:sparkHistoryFirmware}});
}
