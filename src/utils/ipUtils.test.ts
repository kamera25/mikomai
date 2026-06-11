import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import { isGlobalIP } from './ipUtils.js';

describe('isGlobalIP', () => {
  test('グローバルIPv4アドレスを正しく判定できること', () => {
    assert.equal(isGlobalIP('8.8.8.8'), true);
    assert.equal(isGlobalIP('1.1.1.1'), true);
    assert.equal(isGlobalIP('198.51.100.1'), true);
  });

  test('プライベート/ローカルIPv4アドレスは偽（false）を返すこと', () => {
    assert.equal(isGlobalIP('127.0.0.1'), false);
    assert.equal(isGlobalIP('10.0.0.1'), false);
    assert.equal(isGlobalIP('172.16.0.1'), false);
    assert.equal(isGlobalIP('172.31.255.255'), false);
    assert.equal(isGlobalIP('192.168.1.1'), false);
    assert.equal(isGlobalIP('169.254.10.10'), false);
    assert.equal(isGlobalIP('0.0.0.0'), false);
    assert.equal(isGlobalIP('224.0.0.1'), false);
    assert.equal(isGlobalIP('255.255.255.255'), false);
  });

  test('グローバルIPv6アドレスを正しく判定できること', () => {
    assert.equal(isGlobalIP('2001:db8::1'), true);
    assert.equal(isGlobalIP('2001:4860:4860::8888'), true);
  });

  test('ローカル/プライベートIPv6アドレスは偽（false）を返すこと', () => {
    assert.equal(isGlobalIP('::1'), false);
    assert.equal(isGlobalIP('fe80::1'), false);
    assert.equal(isGlobalIP('fc00::1'), false);
    assert.equal(isGlobalIP('ff02::1'), false);
  });

  test('不正な文字列やIP以外の文字列は偽（false）を返すこと', () => {
    assert.equal(isGlobalIP('invalid-ip'), false);
    assert.equal(isGlobalIP('999.999.999.999'), false);
    assert.equal(isGlobalIP('256.0.0.1'), false);
  });
});
