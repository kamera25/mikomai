import { describe, it as test, expect } from 'vitest';
import { parsePingCommand } from './commandParser.js';

describe('parsePingCommand', () => {
  test('基本的なpingコマンドからホスト名を抽出できること', () => {
    const actual = parsePingCommand('ping 192.168.1.1');
    expect(actual).toEqual({ host: '192.168.1.1' });
  });

  test('日本語の「ピン」からホスト名を抽出できること', () => {
    const actual = parsePingCommand('10.0.0.1へピン');
    expect(actual).toEqual({ host: '10.0.0.1' });
  });

  test('sizeオプションを抽出できること', () => {
    const actual = parsePingCommand('ping localhost size 100');
    expect(actual).toEqual({ host: 'localhost', size: 100 });
  });

  test('countオプションを抽出できること', () => {
    const actual = parsePingCommand('ping 8.8.8.8 5回実行');
    expect(actual).toEqual({ host: '8.8.8.8', count: 5 });
  });

  test('df（フラグメント禁止）オプションを抽出できること', () => {
    const actual = parsePingCommand('ping 1.1.1.1 フラグメント禁止');
    expect(actual).toEqual({ host: '1.1.1.1', df: true });
  });

  test('関係ない文字列の場合はnullを返すこと', () => {
    const actual = parsePingCommand('hello world');
    expect(actual).toBe(null);
  });
});
