import { describe, it as test, expect } from 'vitest';
import { isGlobalIP } from './ipUtils.js';

describe('isGlobalIP', () => {
  test('グローバルIPv4アドレスを正しく判定できること', () => {
    expect(isGlobalIP('8.8.8.8')).toBe(true);
    expect(isGlobalIP('1.1.1.1')).toBe(true);
    expect(isGlobalIP('198.51.100.1')).toBe(true);
  });

  test('プライベート/ローカルIPv4アドレスは偽（false）を返すこと', () => {
    expect(isGlobalIP('127.0.0.1')).toBe(false);
    expect(isGlobalIP('10.0.0.1')).toBe(false);
    expect(isGlobalIP('172.16.0.1')).toBe(false);
    expect(isGlobalIP('172.31.255.255')).toBe(false);
    expect(isGlobalIP('192.168.1.1')).toBe(false);
    expect(isGlobalIP('169.254.10.10')).toBe(false);
    expect(isGlobalIP('0.0.0.0')).toBe(false);
    expect(isGlobalIP('224.0.0.1')).toBe(false);
    expect(isGlobalIP('255.255.255.255')).toBe(false);
  });

  test('グローバルIPv6アドレスを正しく判定できること', () => {
    expect(isGlobalIP('2001:db8::1')).toBe(true);
    expect(isGlobalIP('2001:4860:4860::8888')).toBe(true);
  });

  test('ローカル/プライベートIPv6アドレスは偽（false）を返すこと', () => {
    expect(isGlobalIP('::1')).toBe(false);
    expect(isGlobalIP('fe80::1')).toBe(false);
    expect(isGlobalIP('fc00::1')).toBe(false);
    expect(isGlobalIP('ff02::1')).toBe(false);
  });

  test('不正な文字列やIP以外の文字列は偽（false）を返すこと', () => {
    expect(isGlobalIP('invalid-ip')).toBe(false);
    expect(isGlobalIP('999.999.999.999')).toBe(false);
    expect(isGlobalIP('256.0.0.1')).toBe(false);
  });
});
