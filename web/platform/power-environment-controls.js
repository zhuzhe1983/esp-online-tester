import {appendEnvironmentControls} from './environment-controls.js';
import {hasPowerEnvironment,powerParameters,POWER_BOUNDS_UV} from './power-environment.js';

const MICROVOLTS_PER_VOLT = 1000000;
const POWER_FIELDS = [
  ['batteryVoltageUv', '电池虚拟电压 V', 0, POWER_BOUNDS_UV.batteryVoltageUv[1] / MICROVOLTS_PER_VOLT, 0.000001, MICROVOLTS_PER_VOLT],
  ['batteryPresent', '虚拟电池已连接'],
  ['vbusVoltageUv', 'VBUS 虚拟电压 V', 0, POWER_BOUNDS_UV.vbusVoltageUv[1] / MICROVOLTS_PER_VOLT, 0.000001, MICROVOLTS_PER_VOLT],
  ['vbusPresent', '虚拟 VBUS 存在'],
  ['vbusValid', '虚拟 VBUS 有效'],
  ['charging', '虚拟充电状态'],
];

export function appendPowerEnvironmentControls(container, {
  product, scene, state, status, onInitial, onInput, document = container.ownerDocument,
}) {
  if (!hasPowerEnvironment(product)) return null;
  const card = document.createElement('article');
  card.className = 'accessory-card';
  const heading = document.createElement('h3');
  heading.textContent = 'AXP192 虚拟电源环境';
  const note = document.createElement('p');
  note.className = 'helper';
  note.textContent = '固件通过 AXP192 ADC 读取虚拟电压。分享保留初始值，运行中的调整随录制分享。';
  card.append(heading, note);
  const running = status === 'running';
  const form = appendEnvironmentControls(card, {
    fields: POWER_FIELDS,
    values: powerParameters(running ? state : scene.environment?.power),
    prefix: 'AXP192 ',
    disabled: ['loading', 'disposed'].includes(status),
    label: running ? '设置运行中的虚拟环境' : '保存初始环境并重新配置',
    onApply: values => running ? onInput({type:'power-environment', ...values}) : onInitial(values),
    document,
  });
  container.append(card);
  return {...form, card};
}
