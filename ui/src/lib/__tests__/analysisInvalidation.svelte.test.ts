import { describe, it, expect } from 'vitest';
import { getInvalidationStamp, invalidateAnalysis } from '../analysisInvalidation.svelte';

describe('analysisInvalidation', () => {
  it('getInvalidationStamp() starts at 0', () => {
    expect(getInvalidationStamp()).toBe(0);
  });

  it('invalidateAnalysis() increments the stamp', () => {
    const before = getInvalidationStamp();
    invalidateAnalysis();
    expect(getInvalidationStamp()).toBe(before + 1);
  });

  it('multiple calls increment sequentially', () => {
    const before = getInvalidationStamp();
    invalidateAnalysis();
    invalidateAnalysis();
    invalidateAnalysis();
    expect(getInvalidationStamp()).toBe(before + 3);
  });
});
