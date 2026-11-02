import test from 'node:test';
import assert from 'node:assert/strict';
import {prepareConsoleCopy,stripAnsi} from '../web/platform/console-log.js';

test('copy keeps firmware timestamps and joins transport chunks into logical records',()=>{
 const chunks=['\x1b[0;32mI (123) boot: hel','lo\x1b[0m\r','\n\n[12:34:56.789] 中文','\npartial'];
 assert.deepEqual(prepareConsoleCopy(chunks.join('')), {count:3,text:'I (123) boot: hello\n\n[12:34:56.789] 中文\npartial'});
});

test('all, last 50 and last 100 count nonempty lines and retain separators inside the selection',()=>{
 const lines=Array.from({length:123},(_,i)=>`[${i}] message ${i}`),text='\n'+lines.join('\n\n')+'\n';
 for(const limit of ['all',50,100]){
  const expected=limit==='all'?lines:lines.slice(-limit),result=prepareConsoleCopy(text,limit);
  assert.equal(result.count,expected.length);assert.equal(result.text,expected.join('\n\n'));
 }
 assert.deepEqual(prepareConsoleCopy('one\r\ntwo\rthree',50),{count:3,text:'one\ntwo\nthree'});
 assert.deepEqual(prepareConsoleCopy('single very long line '.repeat(1000),100),{count:1,text:'single very long line '.repeat(1000)});
});

test('ANSI colors, cursor codes, OSC hyperlinks and control strings are removed without leaking payloads',()=>{
 assert.equal(stripAnsi('\x1b[2J\x1b[1;1H\x1b[?25l\x1b(Bvisible\x1b7\x1b8\x1bc'),'visible');
 assert.equal(stripAnsi('\x1b]0;private title\x07\x1b]8;;https://example.org\x1b\\link\x1b]8;;\x1b\\'),'link');
 assert.equal(stripAnsi('\x1bPprivate\x1b\\a\x1b_hidden\x1b\\b\x9dtitle\x9cc\x9b31md\x9b0m'),'abcd');
 assert.equal(stripAnsi('prefix\x1b]unfinished title'),'prefix');
 assert.equal(stripAnsi('prefix\x1b[31'),'prefix');
 assert.equal(stripAnsi('a\0b\x07c\td'),'abc\td');
 assert.deepEqual(prepareConsoleCopy(' \n\x1b[0m\r\n\t'),{count:0,text:''});
});

test('copies are independent snapshots and never combine other ports or mutate retained logs',()=>{
 const buffers={usb:'USB only\n',uart0:'I (1) uart0\n',uart1:'UART1 only\n'};
 const snapshot=prepareConsoleCopy(buffers.uart0,50);buffers.uart0+='I (2) later\n';
 assert.deepEqual(snapshot,{count:1,text:'I (1) uart0'});
 assert.equal(prepareConsoleCopy(buffers.uart1).text,'UART1 only');
 assert.equal(prepareConsoleCopy(buffers.uart0).count,2);
 buffers.uart0='';assert.deepEqual(prepareConsoleCopy(buffers.uart0),{count:0,text:''});
 assert.throws(()=>prepareConsoleCopy('one',-1),RangeError);
});
