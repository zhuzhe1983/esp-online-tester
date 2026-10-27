import test from 'node:test';
import assert from 'node:assert/strict';
import {firmwareRunPolicy} from '../web/platform/firmware-policy.js';

const board = {runtime: {board: 'papermono', status: 'partial'}};
const source = {url: 'https://example.com/merged.bin'};
test('an unverified artifact can run on an implemented board regardless of publisher', () => {
  for (const publisher of ['M5Stack', 'Community', undefined]) {
    const policy = firmwareRunPolicy(board, {status: 'trial', source, publisher, simulationVerified: false});
    assert.equal(policy.canRun, true); assert.equal(policy.trial, true);
    assert.equal(policy.label, '可尝试运行');
  }
});
test('missing board and missing startup artifacts remain distinct', () => {
  assert.equal(firmwareRunPolicy({runtime:{board:null}}, {source}).reason, 'board-unavailable');
  assert.equal(firmwareRunPolicy(board, {source:null}).reason, 'artifact-missing');
  assert.equal(firmwareRunPolicy(board, {source:null}).canRun, false);
  assert.equal(firmwareRunPolicy({runtime:{board:null}}, {source}).canRun, false);
});
test('available never implies verification; evidence is explicit', () => {
  assert.equal(firmwareRunPolicy(board, {status:'available',source}).trial,true);
  assert.equal(firmwareRunPolicy(board, {source,simulationVerified:true}).trial,false);
});
