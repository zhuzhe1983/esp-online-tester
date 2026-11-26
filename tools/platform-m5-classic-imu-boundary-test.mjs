import test from 'node:test';
import assert from 'node:assert/strict';
import {imuModelForBoard} from '../web/platform/imu.js';
test('classic IMU availability follows fitted board devices, independently of boot or power families',()=>{
 for(const board of ['stickc-plus-se','m5stick-oled','m5paper-original','m5paper-comm','bala-fire-original'])assert.equal(imuModelForBoard(board),null,board);
 for(const board of ['stickc','stickc-plus','stickc-plus2','stickt2','stickt-original','tlite'])assert.equal(imuModelForBoard(board),'mpu6886-register-subset',board);
});
