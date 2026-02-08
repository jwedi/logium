import { describe, it, expect } from 'vitest';
import { detectGroups, toJsRegex, testPattern } from '../regexUtils';

describe('detectGroups', () => {
  it('finds JS-style named groups', () => {
    expect(detectGroups('(?<name>\\w+) (?<age>\\d+)')).toEqual(['name', 'age']);
  });

  it('finds Rust/Python-style named groups', () => {
    expect(detectGroups('(?P<host>[^ ]+) (?P<code>\\d+)')).toEqual(['host', 'code']);
  });

  it('returns indexed names for unnamed groups', () => {
    expect(detectGroups('(\\w+) (\\d+)')).toEqual(['group_0', 'group_1']);
  });

  it('handles mixed named and unnamed groups', () => {
    expect(detectGroups('(?<name>\\w+) (\\d+) (?P<level>\\w+)')).toEqual([
      'name',
      'group_1',
      'level',
    ]);
  });

  it('returns empty array for no groups', () => {
    expect(detectGroups('hello world')).toEqual([]);
  });
});

describe('toJsRegex', () => {
  it('converts (?P<name>) to (?<name>)', () => {
    expect(toJsRegex('(?P<host>[^ ]+)')).toBe('(?<host>[^ ]+)');
  });

  it('passes through JS-style syntax unchanged', () => {
    expect(toJsRegex('(?<name>\\w+)')).toBe('(?<name>\\w+)');
  });

  it('converts multiple Rust-style groups', () => {
    expect(toJsRegex('(?P<a>\\d+)-(?P<b>\\d+)')).toBe('(?<a>\\d+)-(?<b>\\d+)');
  });
});

describe('testPattern', () => {
  it('returns match with named groups', () => {
    const result = testPattern('ERROR (?P<message>.+)', 'ERROR something failed');
    expect(result.status).toBe('match');
    expect(result.message).toBe('Match: "ERROR something failed"');
    expect(result.groups).toEqual({ message: 'something failed' });
  });

  it('returns no-match when pattern does not match', () => {
    const result = testPattern('ERROR', 'INFO all good');
    expect(result.status).toBe('no-match');
    expect(result.message).toBe('No match');
    expect(result.groups).toEqual({});
  });

  it('returns error for invalid regex', () => {
    const result = testPattern('(unclosed', 'test');
    expect(result.status).toBe('error');
    expect(result.message).toMatch(/Invalid regex/);
    expect(result.groups).toEqual({});
  });

  it('handles JS-style named groups', () => {
    const result = testPattern('(?<level>\\w+): (?<msg>.+)', 'ERROR: bad');
    expect(result.status).toBe('match');
    expect(result.groups).toEqual({ level: 'ERROR', msg: 'bad' });
  });

  it('returns empty groups when pattern has no groups', () => {
    const result = testPattern('ERROR', 'ERROR happened');
    expect(result.status).toBe('match');
    expect(result.groups).toEqual({});
  });

  it('handles pattern matching empty string with groups', () => {
    const result = testPattern('(?<opt>\\d*)', 'hello');
    expect(result.status).toBe('match');
    // The group captures an empty string
    expect(result.groups).toEqual({ opt: '' });
  });
});
