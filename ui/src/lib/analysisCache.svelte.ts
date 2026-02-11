import type { AnalysisResult } from './api';

let cachedResult: AnalysisResult | null = $state(null);
let cachedProjectId: number | null = $state(null);

export function getCachedAnalysis(projectId: number): AnalysisResult | null {
  return cachedProjectId === projectId ? cachedResult : null;
}

export function setCachedAnalysis(projectId: number, result: AnalysisResult) {
  cachedProjectId = projectId;
  cachedResult = result;
}

export function clearCachedAnalysis() {
  cachedResult = null;
  cachedProjectId = null;
}
