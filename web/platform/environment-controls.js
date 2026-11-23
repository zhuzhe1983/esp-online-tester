// Collect virtual sensor inputs for both the workbench and mini player.
export function appendEnvironmentControls(container, {
  fields, values, onApply, label, disabled = false, prefix = '', document = container.ownerDocument,
}) {
  const grid = document.createElement('div'), inputs = {};
  grid.className = 'environment-fields';
  for (const [key, title, min, max, step = 1] of fields) {
    const row = document.createElement('label'), input = document.createElement('input');
    row.textContent = title;
    input.setAttribute('aria-label', prefix + title);
    if (typeof values[key] === 'boolean') {
      input.type = 'checkbox';
    } else {
      input.type = 'number';
      input.min = String(min);
      input.max = String(max);
      input.step = String(step);
      input.required = true;
    }
    inputs[key] = input;
    row.append(input);
    grid.append(row);
  }

  const write = next => {
    for (const [key, , , , , scale = 1] of fields) {
      const input = inputs[key];
      if (input.type === 'checkbox') input.checked = next[key];
      else input.value = String(next[key] / scale);
    }
  };
  const read = () => Object.fromEntries(fields.map(([key, , min, max, , scale = 1]) => {
    const input = inputs[key];
    if (input.type === 'checkbox') return [key, input.checked];
    const value = input.value.trim() ? Number(input.value) : NaN;
    if (value < min || value > max) return [key, NaN];
    return [key, scale === 1 ? value : Math.round(value * scale)];
  }));
  const apply = document.createElement('button');
  apply.type = 'button';
  apply.className = 'button secondary';
  apply.textContent = label;
  apply.addEventListener('click', () => { if (!apply.disabled) void onApply(read()); });
  const setDisabled = value => {
    apply.disabled = value;
    for (const input of Object.values(inputs)) input.disabled = value;
  };
  write(values);
  setDisabled(disabled);
  container.append(grid, apply);
  return {inputs, apply, read, write, setDisabled};
}
