import {unitButtonRoute} from './accessory-routing.js';
import {isPressControl} from './control-input.js';
// Static catalog and portable scene contract. No application server is required.
import {isShellFinish} from './appearance.js';
export const SCHEMA_VERSION = '1';
export const MAX_SCENE_BYTES = 64 * 1024;
const STORAGE_KEY = 'esp-online-tester:accessories:v1';
const unsafeKeys = new Set(['__proto__', 'prototype', 'constructor']);
const idPattern = /^[a-z0-9][a-z0-9:-]{0,79}$/;
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const issue = (code, path, message) => ({code, path, message});

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function checkJson(value, depth = 0, seen = new Set()) {
  if (depth > 20) fail('JSON_DEPTH', 'JSON nesting exceeds 20 levels.');
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return;
  if (typeof value === 'number' && Number.isFinite(value)) return;
  if (typeof value !== 'object') fail('INVALID_JSON', 'Only finite JSON values are accepted.');
  if (seen.has(value)) fail('INVALID_JSON', 'Cyclic objects are not accepted.');
  if (!Array.isArray(value) && Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) {
    fail('INVALID_JSON', 'Only plain JSON objects are accepted.');
  }
  seen.add(value);
  for (const [key, item] of Object.entries(value)) {
    if (unsafeKeys.has(key)) fail('UNSAFE_KEY', `Unsafe JSON key: ${key}`);
    checkJson(item, depth + 1, seen);
  }
  seen.delete(value);
}

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (record(value)) return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  return value;
}

function checkedJson(value, maxBytes = MAX_SCENE_BYTES) {
  checkJson(value);
  const json = JSON.stringify(canonical(value));
  if (new TextEncoder().encode(json).length > maxBytes) fail('INPUT_TOO_LARGE', `JSON exceeds ${maxBytes} bytes.`);
  return json;
}

function parseJson(json, maxBytes = MAX_SCENE_BYTES) {
  if (typeof json !== 'string' || new TextEncoder().encode(json).length > maxBytes) fail('INPUT_TOO_LARGE', 'Invalid or oversized JSON input.');
  let result;
  try { result = JSON.parse(json); } catch { fail('INVALID_JSON', 'Cannot parse JSON.'); }
  checkedJson(result, maxBytes);
  return result;
}

function remoteUrl(value) {
  if (typeof value !== 'string' || value.length > 4096 || value.trim() !== value) return false;
  try {
    const url = new URL(value);
    return !url.username && !url.password && (url.protocol === 'https:' || (url.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)));
  } catch { return false; }
}

export async function loadCatalog() {
  const entries = await Promise.all(['products', 'accessories', 'firmwares'].map(async name => {
    const response = await fetch(new URL(`./catalog/${name}.json`, import.meta.url), {cache: 'no-store'});
    if (!response.ok) fail('CATALOG_FETCH', `Cannot load ${name}: HTTP ${response.status}.`);
    const document = parseJson(await response.text(), 512 * 1024);
    if (document.schemaVersion !== SCHEMA_VERSION || !Array.isArray(document.items)) fail('CATALOG_SCHEMA', `Invalid ${name} catalog.`);
    const ids = new Set();
    for (const item of document.items) {
      if (!idPattern.test(item.id) || ids.has(item.id)) fail('CATALOG_SCHEMA', `Invalid or duplicate ${name} ID.`);
      ids.add(item.id);
      if (name === 'firmwares' && item.source?.manifestUrl) item.source.manifestUrl = new URL(item.source.manifestUrl, import.meta.url).href;
    }
    return [name, document.items];
  }));
  return Object.fromEntries(entries);
}

export function createDefaultScene(catalog, productId = 'm5stack-stopwatch') {
  const product = catalog.products.find(item => item.id === productId);
  if (!product) fail('UNKNOWN_PRODUCT', `Unknown product: ${productId}`);
  const buttons = product.controls.filter(isPressControl), explicit=buttons.filter(control=>control.defaultKeyCode);
  const keyMapping=explicit.length?Object.fromEntries(explicit.map(control=>[control.defaultKeyCode,control.id])):Object.fromEntries(buttons.filter(control=>control.type==='button').slice(0,26).map((button,i)=>[`Key${String.fromCharCode(65+i)}`,button.id]));
  return {
    schemaVersion: SCHEMA_VERSION, productId, firmware: null, accessories: [],
    keyMapping,
    view: {mode: 'studio', deviceView: '3d'},
  };
}

export function firmwareSourceMatches(left, right) {
  if (!left || !right) return false;
  const kind = left.manifestUrl ? 'manifestUrl' : 'url';
  return typeof left[kind] === 'string' && left[kind].length > 0 && left[kind] === right[kind]
    && (!left.sha256 || !right.sha256 || left.sha256.toLowerCase() === right.sha256.toLowerCase());
}

function allowedKeys(value, allowed, path, errors) {
  for (const key of Object.keys(value)) if (!allowed.includes(key)) errors.push(issue('UNKNOWN_FIELD', `${path}/${key}`, `Unknown field: ${key}`));
}

function validateParameters(parameters, path, errors) {
  if (!record(parameters)) { errors.push(issue('PARAMETERS', path, 'Parameters must be an object.')); return; }
  const ranges = {baudRate: [1, 4_000_000], latitude: [-90, 90], longitude: [-180, 180], altitude: [-10000, 100000], speedKnots: [0, 10000], course: [0, 360], satellites: [1, 99]};
  for (const [key, [low, high]] of Object.entries(ranges)) {
    if (parameters[key] !== undefined && (typeof parameters[key] !== 'number' || !Number.isFinite(parameters[key]) || parameters[key] < low || parameters[key] > high || (key === 'course' && parameters[key] === 360) || (['baudRate', 'satellites'].includes(key) && !Number.isInteger(parameters[key])))) {
      errors.push(issue('PARAMETER_RANGE', `${path}/${key}`, `${key} must be between ${low} and ${high}.`));
    }
  }
  if (parameters.positionSource !== undefined) {
    const value = parameters.positionSource, location = `${path}/positionSource`;
    if (!record(value)) { errors.push(issue('POSITION_SOURCE', location, 'Position source must be an object.')); return; }
    allowedKeys(value, ['kind', 'provider', 'fetchedAt', 'city', 'region', 'country'], location, errors);
    if (!['manual', 'ip-estimate'].includes(value.kind)) errors.push(issue('POSITION_SOURCE', location, 'Position source must be manual or ip-estimate.'));
    for (const field of ['provider', 'city', 'region', 'country']) if (value[field] !== undefined && (typeof value[field] !== 'string' || value[field].length > 160)) errors.push(issue('POSITION_SOURCE', `${location}/${field}`, 'Source description must be a short string.'));
    if (value.fetchedAt !== undefined && (typeof value.fetchedAt !== 'string' || !/^\d{4}-\d\d-\d\dT.*Z$/.test(value.fetchedAt) || !Number.isFinite(Date.parse(value.fetchedAt)))) errors.push(issue('POSITION_SOURCE', `${location}/fetchedAt`, 'Fetch time must be an ISO UTC timestamp.'));
  }
}

export function validateScene(scene, catalog) {
  const errors = [], warnings = [];
  try { checkedJson(scene); } catch (error) { return {valid: false, errors: [issue(error.code, '/', error.message)], warnings}; }
  if (!record(scene)) return {valid: false, errors: [issue('SCENE', '/', 'Scene must be an object.')], warnings};
  allowedKeys(scene, ['schemaVersion', 'productId', 'firmware', 'accessories', 'keyMapping', 'view', 'localAccessories'], '', errors);
  if (scene.schemaVersion !== SCHEMA_VERSION) errors.push(issue('SCHEMA_VERSION', '/schemaVersion', 'Only scene schema version 1 is supported.'));
  const product = catalog.products.find(item => item.id === scene.productId);
  if (!product) errors.push(issue('UNKNOWN_PRODUCT', '/productId', `Unknown product: ${scene.productId}`));
  else if (product.runtime.status === 'unavailable') warnings.push(issue('MODEL_UNAVAILABLE', '/productId', 'This product has metadata and a visual preview; its board model is not implemented.'));
  else if (product.runtime.status === 'partial') warnings.push(issue('PARTIAL_MODEL', '/productId', 'The board model is partial. Firmware compatibility requires an actual execution test.'));
  if (scene.firmware === null) warnings.push(issue('FIRMWARE_MISSING', '/firmware', 'Choose a firmware artifact before running.'));
  else if (!record(scene.firmware)) errors.push(issue('FIRMWARE', '/firmware', 'Firmware must be null or a source object.'));
  else {
    allowedKeys(scene.firmware, ['manifestUrl', 'url', 'sha256'], '/firmware', errors);
    if (Boolean(scene.firmware.manifestUrl) === Boolean(scene.firmware.url)) errors.push(issue('FIRMWARE_SOURCE', '/firmware', 'Specify exactly one manifestUrl or merged image url.'));
    for (const key of ['manifestUrl', 'url']) if (scene.firmware[key] !== undefined && !remoteUrl(scene.firmware[key])) errors.push(issue('FIRMWARE_URL', `/firmware/${key}`, 'Use an absolute HTTPS URL, or loopback HTTP for local development.'));
    if (scene.firmware.sha256 !== undefined && !/^[a-f0-9]{64}$/i.test(scene.firmware.sha256)) errors.push(issue('FIRMWARE_HASH', '/firmware/sha256', 'SHA-256 must contain 64 hexadecimal digits.'));
    if (!scene.firmware.sha256) warnings.push(issue('UNPINNED_FIRMWARE', '/firmware', 'The source has no SHA-256 pin; its content may change.'));
    warnings.push(issue('FIRMWARE_CORS', '/firmware', 'Remote artifacts and manifest files must allow browser cross-origin requests.'));
  }
  const locals = scene.localAccessories ?? [];
  if (!Array.isArray(locals) || locals.length > 32) errors.push(issue('LOCAL_ACCESSORIES', '/localAccessories', 'At most 32 local accessory definitions are allowed.'));
  const definitions = new Map(catalog.accessories.map(item => [item.id, item]));
  if (Array.isArray(locals)) for (const [index, definition] of locals.entries()) {
    const validation = validateLocalAccessory(definition);
    errors.push(...validation.errors.map(item => ({...item, path: `/localAccessories/${index}${item.path}`})));
    if (validation.valid) {
      if (definitions.has(definition.id)) errors.push(issue('DUPLICATE_DEFINITION', `/localAccessories/${index}/id`, 'Accessory definition IDs must be unique.'));
      else definitions.set(definition.id, definition);
    }
  }
  if (!Array.isArray(scene.accessories) || scene.accessories.length > 32) errors.push(issue('ACCESSORIES', '/accessories', 'At most 32 accessory instances are allowed.'));
  else {
    const ids = new Set(), usedPins = new Set();
    for (const [index, accessory] of scene.accessories.entries()) {
      const path = `/accessories/${index}`;
      if (!record(accessory)) { errors.push(issue('ACCESSORY', path, 'Accessory must be an object.')); continue; }
      allowedKeys(accessory, ['id', 'catalogId', 'parameters', 'wiring'], path, errors);
      if (typeof accessory.id !== 'string' || !idPattern.test(accessory.id) || ids.has(accessory.id)) errors.push(issue('ACCESSORY_ID', `${path}/id`, 'Instance IDs must be valid and unique.'));
      ids.add(accessory.id);
      const definition = definitions.get(accessory.catalogId);
      if (!definition) errors.push(issue('UNKNOWN_ACCESSORY', `${path}/catalogId`, `Unknown accessory: ${accessory.catalogId}`));
      if (definition?.model.status === 'unavailable') warnings.push(issue('ACCESSORY_MODEL_UNAVAILABLE', path, 'Accessory is metadata-only; no peripheral behavior is implemented.'));
      validateParameters(accessory.parameters, `${path}/parameters`, errors);
      if (!record(accessory.wiring)) { errors.push(issue('WIRING', `${path}/wiring`, 'Declare logical or physical wiring.')); continue; }
      const wiring = accessory.wiring;
      allowedKeys(wiring, ['mode', 'port', 'connectorId', 'rx', 'tx', 'interfaceId'], `${path}/wiring`, errors);
      if (wiring.mode === 'logical') {
        if (wiring.port !== 'uart1') errors.push(issue('UART_PORT', `${path}/wiring/port`, 'Logical accessory input currently supports UART1 only.'));
        if (wiring.interfaceId !== undefined || wiring.connectorId !== undefined || wiring.rx !== undefined || wiring.tx !== undefined) errors.push(issue('LOGICAL_WIRING', `${path}/wiring`, 'Logical UART input does not declare physical pins.'));
        warnings.push(issue('LOGICAL_UART', `${path}/wiring`, 'Logical UART injection bypasses GPIO routing, baud timing and external power. It does not verify physical wiring.'));
      } else if (wiring.mode === 'visual') {
        if (Object.keys(wiring).some(key => key !== 'mode')) errors.push(issue('VISUAL_WIRING', `${path}/wiring`, 'Visual placement does not declare electrical routing.'));
        warnings.push(issue('VISUAL_ACCESSORY', `${path}/wiring`, 'The accessory is shown in 3D; no electrical or peripheral behavior is connected.'));
      } else if (wiring.mode === 'physical' && (wiring.interfaceId !== undefined || definition?.model.id === 'gpio-button-u027')) {
        const route=unitButtonRoute(product,accessory,definition);
        if(Object.keys(wiring).some(key=>!['mode','interfaceId'].includes(key)) || !route) errors.push(issue('ACCESSORY_ROUTE',`${path}/wiring`,'Unit Button currently supports only StopWatch Port A, with interfaceId grove.'));
        if(route){if(usedPins.has(route.pin))errors.push(issue('PIN_CONFLICT',`${path}/wiring`,'A physical GPIO is assigned more than once.'));usedPins.add(route.pin);}
      } else if (wiring.mode === 'physical') {
        const connector = product?.interfaces.find(item => item.id === wiring.connectorId && item.external === true);
        if (!connector) errors.push(issue('CONNECTOR', `${path}/wiring/connectorId`, 'Connector is not in this product’s verified pin map.'));
        for (const signal of ['rx', 'tx']) {
          const pin = wiring[signal];
          if (!Number.isInteger(pin) || !connector?.pins.some(item => item.gpio === pin)) errors.push(issue('PIN', `${path}/wiring/${signal}`, 'Choose a verified GPIO on the declared connector.'));
          if (usedPins.has(pin)) errors.push(issue('PIN_CONFLICT', `${path}/wiring/${signal}`, 'A physical GPIO is assigned more than once.'));
          usedPins.add(pin);
        }
        if (!['uart0', 'uart1', 'uart2'].includes(wiring.port)) errors.push(issue('UART_PORT', `${path}/wiring/port`, 'Declare the firmware UART controller.'));
        warnings.push(issue('PHYSICAL_WIRING_UNSUPPORTED', `${path}/wiring`, 'Physical UART routing is recorded for review but is not executed by this platform adapter.'));
      } else errors.push(issue('WIRING_MODE', `${path}/wiring/mode`, 'Wiring mode must be logical, physical or visual.'));
    }
  }
  if (!record(scene.keyMapping) || Object.keys(scene.keyMapping).length > 64) errors.push(issue('KEY_MAPPING', '/keyMapping', 'Key mapping must be an object with at most 64 bindings.'));
  else for (const [key, control] of Object.entries(scene.keyMapping)) {
    if (!/^[A-Za-z][A-Za-z0-9]{0,39}$/.test(key)) errors.push(issue('KEY_CODE', `/keyMapping/${key}`, 'Use a KeyboardEvent.code such as KeyA or Space.'));
    if (product && !product.controls.some(item => item.id === control && isPressControl(item))) errors.push(issue('UNKNOWN_CONTROL', `/keyMapping/${key}`, `Unknown button: ${control}`));
  }
  if (!record(scene.view)) errors.push(issue('VIEW', '/view', 'View must be an object.'));
  else {
    allowedKeys(scene.view, ['mode', 'deviceView', 'appearance'], '/view', errors);
    if (!['studio', 'embed'].includes(scene.view.mode)) errors.push(issue('VIEW_MODE', '/view/mode', 'Choose studio or embed.'));
    if (!['2d', '3d'].includes(scene.view.deviceView)) errors.push(issue('DEVICE_VIEW', '/view/deviceView', 'Choose 2d or 3d.'));
    if (scene.view.appearance !== undefined) {
      const appearance = scene.view.appearance, path = '/view/appearance';
      if (!record(appearance)) errors.push(issue('APPEARANCE', path, 'Appearance must be an object.'));
      else {
        allowedKeys(appearance, ['shell'], path, errors);
        if (!record(appearance.shell)) errors.push(issue('APPEARANCE', `${path}/shell`, 'Shell appearance is required.'));
        else {
          const shell = appearance.shell;
          allowedKeys(shell, ['color', 'opacity', 'finish'], `${path}/shell`, errors);
          if (typeof shell.color !== 'string' || !/^#[0-9a-fA-F]{6}$/.test(shell.color)) errors.push(issue('SHELL_COLOR', `${path}/shell/color`, 'Use a six-digit hex color.'));
          if (typeof shell.opacity !== 'number' || !Number.isFinite(shell.opacity) || shell.opacity < 0 || shell.opacity > 1) errors.push(issue('SHELL_OPACITY', `${path}/shell/opacity`, 'Opacity must be between 0 and 1.'));
          if (!isShellFinish(shell.finish)) errors.push(issue('SHELL_FINISH', `${path}/shell/finish`, 'Choose a supported shell finish.'));
        }
      }
    }
  }
  return {valid: errors.length === 0, errors, warnings};
}

export function encodeScene(scene) {
  if (!record(scene) || scene.schemaVersion !== SCHEMA_VERSION) fail('SCHEMA_VERSION', 'Only scene schema version 1 is supported.');
  const bytes = new TextEncoder().encode(checkedJson(scene));
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
}

export function decodeScene(encoded) {
  if (typeof encoded !== 'string' || encoded.length > Math.ceil(MAX_SCENE_BYTES * 4 / 3) || !/^[A-Za-z0-9_-]+$/.test(encoded) || encoded.length % 4 === 1) fail('SCENE_ENCODING', 'Invalid or oversized base64url scene.');
  let json;
  try {
    const binary = atob(encoded.replaceAll('-', '+').replaceAll('_', '/'));
    json = new TextDecoder('utf-8', {fatal: true}).decode(Uint8Array.from(binary, char => char.charCodeAt(0)));
  } catch { fail('SCENE_ENCODING', 'Scene is not valid UTF-8 base64url.'); }
  const scene = parseJson(json);
  if (!record(scene) || scene.schemaVersion !== SCHEMA_VERSION) fail('SCHEMA_VERSION', 'Only scene schema version 1 is supported.');
  return scene;
}

export function createLocalAccessory(input) {
  if (!record(input)) fail('LOCAL_ACCESSORY', 'Accessory input must be an object.');
  checkedJson(input, 16 * 1024);
  const modelId = input.modelId ?? 'metadata-only';
  const slug = String(input.name ?? '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 50);
  const definition = {
    schemaVersion: SCHEMA_VERSION, id: input.id ?? `local:${slug || 'accessory'}`, name: input.name,
    type: modelId === 'uart-nmea' ? 'gps' : 'custom',
    model: {id: modelId, status: modelId === 'uart-nmea' ? 'logical-input' : 'unavailable'},
    parameters: input.parameters ?? {}, connector: input.connector ?? {type: 'custom', voltage: 3.3},
    sources: [], limitations: modelId === 'uart-nmea' ? ['Logical UART1 injection only; no physical timing or routing.'] : ['Metadata only; no executable peripheral model.'],
  };
  if (input.description !== undefined) definition.description = input.description;
  const validation = validateLocalAccessory(definition);
  if (!validation.valid) { const error = new Error(validation.errors.map(item => item.message).join(' ')); error.code = 'LOCAL_ACCESSORY'; error.details = validation.errors; throw error; }
  return definition;
}

export function validateLocalAccessory(definition) {
  const errors = [];
  try { checkedJson(definition, 16 * 1024); } catch (error) { return {valid: false, errors: [issue(error.code, '/', error.message)], warnings: []}; }
  if (!record(definition)) return {valid: false, errors: [issue('LOCAL_ACCESSORY', '/', 'Accessory must be an object.')], warnings: []};
  allowedKeys(definition, ['schemaVersion', 'id', 'name', 'description', 'type', 'model', 'parameters', 'connector', 'sources', 'limitations'], '', errors);
  if (definition.schemaVersion !== SCHEMA_VERSION) errors.push(issue('SCHEMA_VERSION', '/schemaVersion', 'Only schema version 1 is supported.'));
  if (typeof definition.id !== 'string' || !/^local:[a-z0-9][a-z0-9-]{0,69}$/.test(definition.id)) errors.push(issue('LOCAL_ID', '/id', 'Local accessory IDs must use local: followed by a lowercase slug.'));
  if (typeof definition.name !== 'string' || !definition.name.trim() || definition.name.length > 120) errors.push(issue('NAME', '/name', 'Name must contain 1–120 characters.'));
  if (definition.description !== undefined && (typeof definition.description !== 'string' || definition.description.length > 2000)) errors.push(issue('DESCRIPTION', '/description', 'Description must contain at most 2000 characters.'));
  if (!['gps', 'custom'].includes(definition.type)) errors.push(issue('TYPE', '/type', 'Choose gps or custom.'));
  if (!record(definition.model)) errors.push(issue('MODEL', '/model', 'Model must be an object.'));
  else {
    allowedKeys(definition.model, ['id', 'status'], '/model', errors);
    const expected = {'uart-nmea': 'logical-input', 'metadata-only': 'unavailable'}[definition.model.id];
    if (!expected || definition.model.status !== expected) errors.push(issue('MODEL', '/model', 'Choose uart-nmea/logical-input or metadata-only/unavailable.'));
    if (definition.model.id === 'uart-nmea' && definition.type !== 'gps') errors.push(issue('TYPE', '/type', 'The UART NMEA model must use type gps.'));
  }
  validateParameters(definition.parameters, '/parameters', errors);
  if (!record(definition.connector)) errors.push(issue('CONNECTOR', '/connector', 'Connector must be an object.'));
  else {
    allowedKeys(definition.connector, ['type', 'voltage', 'protocol', 'pins'], '/connector', errors);
    if (typeof definition.connector.type !== 'string' || !definition.connector.type.trim() || definition.connector.type.length > 80) errors.push(issue('CONNECTOR', '/connector/type', 'Provide a connector type.'));
    if (typeof definition.connector.voltage !== 'number' || definition.connector.voltage <= 0 || definition.connector.voltage > 48) errors.push(issue('VOLTAGE', '/connector/voltage', 'Voltage must be greater than 0 and at most 48.'));
    if (definition.connector.protocol !== undefined && !['uart', 'i2c', 'gpio', 'spi'].includes(definition.connector.protocol)) errors.push(issue('PROTOCOL', '/connector/protocol', 'Choose uart, i2c, gpio or spi.'));
    if (definition.connector.pins !== undefined) {
      if (!Array.isArray(definition.connector.pins) || definition.connector.pins.length > 32) errors.push(issue('PINS', '/connector/pins', 'Declare at most 32 pins.'));
      else {
        const names = new Set();
        for (const [index, pin] of definition.connector.pins.entries()) {
          const path = `/connector/pins/${index}`;
          if (!record(pin)) { errors.push(issue('PIN', path, 'Pin must be an object.')); continue; }
          allowedKeys(pin, ['name', 'direction'], path, errors);
          if (typeof pin.name !== 'string' || !pin.name.trim() || pin.name.length > 40 || names.has(pin.name)) errors.push(issue('PIN_NAME', `${path}/name`, 'Pin names must be unique and contain 1–40 characters.'));
          names.add(pin.name);
          if (!['input', 'output', 'bidirectional', 'power', 'ground'].includes(pin.direction)) errors.push(issue('PIN_DIRECTION', `${path}/direction`, 'Choose input, output, bidirectional, power or ground.'));
        }
      }
    }
  }
  if (!Array.isArray(definition.sources) || definition.sources.some(source => !record(source) || typeof source.title !== 'string' || !remoteUrl(source.url))) errors.push(issue('SOURCES', '/sources', 'Sources must be objects with title and HTTPS url.'));
  if (!Array.isArray(definition.limitations) || definition.limitations.some(item => typeof item !== 'string')) errors.push(issue('LIMITATIONS', '/limitations', 'Limitations must be strings.'));
  return {valid: errors.length === 0, errors, warnings: []};
}

export function importLocalAccessory(json) {
  const definition = parseJson(json, 16 * 1024);
  const validation = validateLocalAccessory(definition);
  if (!validation.valid) fail('LOCAL_ACCESSORY', validation.errors.map(item => item.message).join(' '));
  return definition;
}

export function exportLocalAccessory(definition) {
  return JSON.stringify(importLocalAccessory(checkedJson(definition, 16 * 1024)), null, 2);
}

export function listLocalAccessories(storage = globalThis.localStorage) {
  if (!storage) fail('STORAGE_UNAVAILABLE', 'Browser local storage is unavailable.');
  const raw = storage.getItem(STORAGE_KEY);
  if (raw === null) return [];
  const definitions = parseJson(raw, 512 * 1024);
  if (!Array.isArray(definitions) || definitions.length > 32) fail('STORAGE_INVALID', 'Local accessory store is invalid.');
  const ids = new Set();
  for (const definition of definitions) {
    if (!validateLocalAccessory(definition).valid || ids.has(definition.id)) fail('STORAGE_INVALID', 'Local accessory store contains an invalid or duplicate definition.');
    ids.add(definition.id);
  }
  return definitions;
}

export function saveLocalAccessory(definition, storage = globalThis.localStorage) {
  const clean = importLocalAccessory(checkedJson(definition, 16 * 1024));
  const definitions = listLocalAccessories(storage).filter(item => item.id !== clean.id);
  if (definitions.length >= 32) fail('STORAGE_LIMIT', 'At most 32 local accessories can be stored.');
  definitions.push(clean);
  storage.setItem(STORAGE_KEY, checkedJson(definitions, 512 * 1024));
  return clean;
}

export function removeLocalAccessory(id, storage = globalThis.localStorage) {
  const definitions = listLocalAccessories(storage);
  const remaining = definitions.filter(item => item.id !== id);
  storage.setItem(STORAGE_KEY, checkedJson(remaining, 512 * 1024));
  return remaining.length !== definitions.length;
}
