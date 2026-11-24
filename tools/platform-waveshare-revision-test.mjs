import test from 'node:test';
import assert from 'node:assert/strict';
import {validateBoardRevision,isS31Board,chipIdForBoard} from '../web/platform/app-image.js';
import {WAVESHARE_BOARD_CAPABILITIES} from '../web/platform/waveshare-board-families.js';

function header(minimum,maximum=0){const bytes=new Uint8Array(24),view=new DataView(bytes.buffer);view.setUint16(15,minimum,true);view.setUint16(17,maximum,true);return bytes;}
test('delivered P4 firmware headers cannot cross the selected wafer boundary',()=>{
  for(const [board,cap] of Object.entries(WAVESHARE_BOARD_CAPABILITIES)){
    if(cap.chip!=='esp32p4'||!cap.virtualRevision)continue;
    const revision=cap.virtualRevision;
    assert.doesNotThrow(()=>validateBoardRevision(header(revision),board),board);
    const excluded=revision===100?header(300):header(100);
    assert.throws(()=>validateBoardRevision(excluded,board),{code:'CHIP_REVISION_UNSUPPORTED'},board);
    assert.throws(()=>validateBoardRevision(header(revision,revision-1),board),{code:'CHIP_REVISION_UNSUPPORTED'},board);
  }
});
test('Waveshare S31 headers retain the S31 chip ID and rev0 boundary',()=>{
  const board='waveshare-s31-touch-lcd43c';
  assert.equal(isS31Board(board),true);assert.equal(chipIdForBoard(board),32);
  assert.doesNotThrow(()=>validateBoardRevision(header(0),board));
  assert.throws(()=>validateBoardRevision(header(1),board),{code:'CHIP_REVISION_UNSUPPORTED'});
});
