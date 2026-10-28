// Same flash layout / screen / storage, but Lite physically lacks NFC and LoRa.
export const isPaperMonoBoard = board => board === 'papermono' || board === 'papermono-lite';
export const firmwareBoardMatches = (manifestBoard,board) => manifestBoard === board || isPaperMonoBoard(manifestBoard) && isPaperMonoBoard(board);
// Lite used the papermono execution board before variant support. Keep its
// original storage namespace forever: productId still prevents full/Lite mixing,
// and existing epoch/revision protection plus explicit clear remain unchanged.
export const persistentBoard = (productId,board) => productId === 'm5stack-c153-lite' && board === 'papermono-lite' ? 'papermono' : board;

// Physical card transports are board-specific; this only exposes shared storage UI.
export const hasSdCard = board => isPaperMonoBoard(board) || board === 'papers3';
