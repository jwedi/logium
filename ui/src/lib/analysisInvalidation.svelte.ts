let stamp = $state(0);

export function invalidateAnalysis() {
  stamp++;
}

export function getInvalidationStamp(): number {
  return stamp;
}
