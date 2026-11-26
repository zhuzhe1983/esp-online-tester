import {appendEnvironmentControls} from './environment-controls.js';
import {THERMAL_PRODUCT_IDS,THERMAL_FIELDS,DEFAULT_THERMAL,thermalFields,thermalDefaults} from './thermal-input.js';

export function mountThermalEnvironment({container, runtime, beforeInput, onError = console.error}) {
  const document = container.ownerDocument;
  container.hidden = true;
  container.dataset.thermalEnvironment = '';
  container.className = 'accessory-card thermal-environment';
  const title = document.createElement('h3');
  title.textContent = 'Lepton 合成 Raw14 环境';
  const note = document.createElement('p');
  note.className = 'helper';
  note.textContent = '固件通过 VoSPI 读取合成的 Raw14 计数并绘制图像。温度、辐射率和 FFC 尚未校准。';
  const status = document.createElement('p');
  status.className = 'helper';
  status.setAttribute('role', 'status');
  container.append(title, note);

  let disposed = false, current = null, pending = false;
  const fields=THERMAL_FIELDS.map(field=>[...field]);
  const form = appendEnvironmentControls(container, {
    fields,
    values: DEFAULT_THERMAL,
    label: '应用模拟热源',
    disabled: true,
    async onApply(values) {
      if (Object.values(form.inputs).some(input => !input.reportValidity())) return;
      pending = true;
      form.setDisabled(true);
      try {
        await beforeInput?.();
        const receipt = await runtime.input({type:'thermal-environment', ...values});
        status.textContent = `已排队，客体时钟 ${receipt.guestTime.toFixed(3)} 秒。请观察固件随后绘制的图像。`;
      } catch (error) {
        status.textContent = error.message;
        onError(error);
      } finally {
        pending = false;
        if (!disposed) form.setDisabled(current?.status !== 'running');
      }
    },
  });
  for (const [key, input] of Object.entries(form.inputs)) input.dataset.thermalField = key;
  form.apply.dataset.thermalApply = '';
  container.append(status);

  function refresh(state) {
    if (disposed) return;
    if (state.scene?.productId !== current?.scene?.productId || state.status !== 'running') status.textContent = '';
    const id=state.scene?.productId;
    if(id!==current?.scene?.productId) {
      fields.splice(0,fields.length,...thermalFields(id).map(field=>[...field]));
      for(const [key,label,min,max]of fields) {const input=form.inputs[key];input.min=String(min);input.max=String(max);input.setAttribute('aria-label',label);if(input.parentElement?.firstChild)input.parentElement.firstChild.textContent=label;}
    }
    current = state;
    container.hidden = !THERMAL_PRODUCT_IDS.includes(id);
    const mlx=id==='m5stack-u149'||id==='m5stack-k126';
    title.textContent=mlx?'MLX90640 合成 ADC 环境':'Lepton 合成 Raw14 环境';
    note.textContent=id==='m5stack-k126'?'固件通过 I²C 读取 32×24 合成 ADC 与虚拟 EEPROM；热点坐标为160×120归一化空间，未作实物标定。':mlx?'原始 ADC 计数通过内部 I²C 传给固件；客体使用公开虚拟 EEPROM 计算温度。此输入不是摄氏温度，也不是实物标定。':'固件通过 VoSPI 读取合成的 Raw14 计数并绘制图像。温度、辐射率和 FFC 尚未校准。';
    form.write(state.thermalEnvironment ?? thermalDefaults(id));
    form.setDisabled(state.status !== 'running' || pending);
  }
  const stop = runtime.subscribe(event => {
    if (event.type === 'state') refresh(event.state);
    if (event.type === 'thermal-environment') form.write(event.thermalEnvironment);
  });
  void runtime.observe().then(refresh, onError);
  return {dispose() { disposed = true; stop(); container.replaceChildren(); }};
}
