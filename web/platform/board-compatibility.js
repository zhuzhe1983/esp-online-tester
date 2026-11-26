import {LILYGO_BOARD_CAPABILITIES} from './lilygo-board-families.js';
import {WAVESHARE_BOARD_CAPABILITIES} from './waveshare-board-families.js';
// Same flash layout / screen / storage, but Lite physically lacks NFC and LoRa.
export const isPaperMonoBoard = board => board === 'papermono' || board === 'papermono-lite';
export const firmwareBoardMatches = (manifestBoard,board) => manifestBoard === board || isPaperMonoBoard(manifestBoard) && isPaperMonoBoard(board);
// Lite used the papermono execution board before variant support. Keep its
// original storage namespace forever: productId still prevents full/Lite mixing,
// and existing epoch/revision protection plus explicit clear remain unchanged.
export const persistentBoard = (productId,board) => productId === 'm5stack-c153-lite' && board === 'papermono-lite' ? 'papermono' : board;

export const isCore2Board = board => ['core2','core2-v11','core2-v13','core2-aws','core2-aws-v13'].includes(board);
export const isStickPlusBoard = board => board === 'stickc-plus' || board === 'm5stack-k016-p' || board === 'stickc-plus-se' || board === 'tlite';
export const isClassicStickBoard = board => board === 'stickc' || isStickPlusBoard(board) || ['stickt2','stickt-original','stickc-plus2','m5stick-oled'].includes(board);

// Physical card transports are board-specific; this only exposes shared storage UI.
export const hasSdCard = board => WAVESHARE_BOARD_CAPABILITIES[board]?.sdCard === true || LILYGO_BOARD_CAPABILITIES[board]?.sdCard === true || isPaperMonoBoard(board) || isCore2Board(board) || board === 'papers3' || board === 'paperdiy' || board === 'corematrix' || board === 'metalio-ink4' || board === 'metalio-ink4-plus' || ['unit-cams3','unit-cams3-5mp','m5stack-tab5','bala-fire-original','m5paper-v11','m5paper-original','m5paper-comm','m5stack-basic','cores3','cores3-se','cores3-lite','cores3-thread-br'].includes(board) || ['seeed-reterminal-sticky','seeed-reterminal-e1001','seeed-reterminal-e1002','seeed-reterminal-e1003','seeed-reterminal-e1004'].includes(board);
