import {hasSdCard} from './board-compatibility.js';

// A physical socket is catalog evidence, not proof of an emulated transport.
// Board support is retained as a fallback for legacy scenes/catalogs.
export const hasTfSlot = product => hasSdCard(product?.runtime?.board) || product?.interfaces?.some(port => /^(microSD|micro-SD|TF|SD)$/i.test(port.type)) === true;
export const preparedTfKey = product => `v1:${product.id}:prepared:${'0'.repeat(64)}`;
