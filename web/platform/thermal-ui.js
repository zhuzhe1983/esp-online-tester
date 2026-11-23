import {appendEnvironmentControls} from './environment-controls.js';
import {THERMAL_PRODUCT_ID,THERMAL_FIELDS,DEFAULT_THERMAL} from './thermal-input.js';

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
  const form = appendEnvironmentControls(container, {
    fields: THERMAL_FIELDS,
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
    current = state;
    container.hidden = state.scene?.productId !== THERMAL_PRODUCT_ID;
    form.write(state.thermalEnvironment ?? DEFAULT_THERMAL);
    form.setDisabled(state.status !== 'running' || pending);
  }
  const stop = runtime.subscribe(event => {
    if (event.type === 'state') refresh(event.state);
    if (event.type === 'thermal-environment') form.write(event.thermalEnvironment);
  });
  void runtime.observe().then(refresh, onError);
  return {dispose() { disposed = true; stop(); container.replaceChildren(); }};
}
