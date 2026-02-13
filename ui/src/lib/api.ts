// ---- Types ----

export interface Project {
  id: number;
  name: string;
  created_at: string;
}

export interface TimestampTemplate {
  id: number;
  name: string;
  format: string;
  extraction_regex: string | null;
  default_year: number | null;
}

export interface SourceTemplate {
  id: number;
  name: string;
  timestamp_template_id: number;
  line_delimiter: string;
  content_regex: string | null;
  continuation_regex: string | null;
  json_timestamp_field: string | null;
}

export interface Source {
  id: number;
  name: string;
  template_id: number;
  file_path: string;
}

export interface MatchRule {
  id: number;
  pattern: string;
}

export interface ExtractionRule {
  id: number;
  extraction_type: 'Parsed' | 'Static' | 'Clear';
  state_key: string;
  pattern: string | null;
  static_value: string | null;
  mode: 'Replace' | 'Accumulate';
}

export interface LogRule {
  id: number;
  name: string;
  match_mode: 'Any' | 'All';
  match_rules: MatchRule[];
  extraction_rules: ExtractionRule[];
}

export interface Ruleset {
  id: number;
  name: string;
  template_id: number;
  rule_ids: number[];
}

export type StateValue =
  | { String: string }
  | { Integer: number }
  | { Float: number }
  | { Bool: boolean };

export interface PatternPredicate {
  source_name: string;
  state_key: string;
  operator: string;
  operand: { Literal: StateValue } | { StateRef: { source_name: string; state_key: string } };
}

export interface Pattern {
  id: number;
  name: string;
  predicates: PatternPredicate[];
}

export interface LogLine {
  timestamp: string;
  source_id: number;
  raw: string;
  content: string;
}

export interface RuleMatch {
  rule_id: number;
  source_id: number;
  log_line: LogLine;
  extracted_state: Record<string, StateValue>;
}

export interface PatternMatch {
  pattern_id: number;
  timestamp: string;
  state_snapshot: Record<string, Record<string, StateValue>>;
}

export interface StateChange {
  timestamp: string;
  source_id: number;
  source_name: string;
  state_key: string;
  old_value: StateValue | null;
  new_value: StateValue | null;
  rule_id: number;
}

export interface AnalysisResult {
  rule_matches: RuleMatch[];
  pattern_matches: PatternMatch[];
  state_changes: StateChange[];
}

export interface SuggestRuleResponse {
  pattern: string;
  capture_groups: string[];
}

export interface LogCluster {
  template: string;
  count: number;
  source_ids: number[];
  sample_lines: string[];
}

export interface ClusterResult {
  clusters: LogCluster[];
  total_lines: number;
}

export interface TimeRange {
  start: string | null;
  end: string | null;
}

// ---- API Client ----

const BASE = '/api';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...init?.headers,
    },
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`API ${res.status}: ${text}`);
  }
  if (res.status === 204) return undefined as T;
  return res.json();
}

// Projects
export const projects = {
  list: () => request<Project[]>('/projects'),
  get: (id: number) => request<Project>(`/projects/${id}`),
  create: (data: { name: string }) =>
    request<Project>('/projects', { method: 'POST', body: JSON.stringify(data) }),
  update: (id: number, data: Partial<Project>) =>
    request<Project>(`/projects/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  delete: (id: number) => request<void>(`/projects/${id}`, { method: 'DELETE' }),
  exportConfig: async (id: number): Promise<Blob> => {
    const res = await fetch(`${BASE}/projects/${id}/export`);
    if (!res.ok) throw new Error(`Export failed: ${res.status}`);
    return res.blob();
  },
  importConfig: (
    id: number,
    data: unknown,
  ): Promise<{
    timestamp_templates: number;
    source_templates: number;
    rules: number;
    rulesets: number;
    patterns: number;
  }> =>
    request(`/projects/${id}/import`, {
      method: 'POST',
      body: JSON.stringify(data),
    }),
};

// Timestamp Templates
export const timestampTemplates = {
  list: (pid: number) => request<TimestampTemplate[]>(`/projects/${pid}/timestamp-templates`),
  get: (pid: number, id: number) =>
    request<TimestampTemplate>(`/projects/${pid}/timestamp-templates/${id}`),
  create: (pid: number, data: Omit<TimestampTemplate, 'id'>) =>
    request<TimestampTemplate>(`/projects/${pid}/timestamp-templates`, {
      method: 'POST',
      body: JSON.stringify(data),
    }),
  update: (pid: number, id: number, data: Partial<TimestampTemplate>) =>
    request<TimestampTemplate>(`/projects/${pid}/timestamp-templates/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),
  delete: (pid: number, id: number) =>
    request<void>(`/projects/${pid}/timestamp-templates/${id}`, { method: 'DELETE' }),
};

// Templates
export const templates = {
  list: (pid: number) => request<SourceTemplate[]>(`/projects/${pid}/templates`),
  get: (pid: number, id: number) => request<SourceTemplate>(`/projects/${pid}/templates/${id}`),
  create: (pid: number, data: Omit<SourceTemplate, 'id'>) =>
    request<SourceTemplate>(`/projects/${pid}/templates`, {
      method: 'POST',
      body: JSON.stringify(data),
    }),
  update: (pid: number, id: number, data: Partial<SourceTemplate>) =>
    request<SourceTemplate>(`/projects/${pid}/templates/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),
  delete: (pid: number, id: number) =>
    request<void>(`/projects/${pid}/templates/${id}`, { method: 'DELETE' }),
};

// Sources
export const sources = {
  list: (pid: number) => request<Source[]>(`/projects/${pid}/sources`),
  get: (pid: number, id: number) => request<Source>(`/projects/${pid}/sources/${id}`),
  create: (pid: number, data: Omit<Source, 'id'>) =>
    request<Source>(`/projects/${pid}/sources`, { method: 'POST', body: JSON.stringify(data) }),
  upload: async (pid: number, id: number, file: File): Promise<Source> => {
    const form = new FormData();
    form.append('file', file);
    const res = await fetch(`${BASE}/projects/${pid}/sources/${id}/upload`, {
      method: 'POST',
      body: form,
    });
    if (!res.ok) throw new Error(`Upload failed: ${res.status}`);
    return res.json();
  },
  delete: (pid: number, id: number) =>
    request<void>(`/projects/${pid}/sources/${id}`, { method: 'DELETE' }),
};

// Rules
export const rules = {
  list: (pid: number) => request<LogRule[]>(`/projects/${pid}/rules`),
  get: (pid: number, id: number) => request<LogRule>(`/projects/${pid}/rules/${id}`),
  create: (pid: number, data: Omit<LogRule, 'id'>) =>
    request<LogRule>(`/projects/${pid}/rules`, { method: 'POST', body: JSON.stringify(data) }),
  update: (pid: number, id: number, data: Partial<LogRule>) =>
    request<LogRule>(`/projects/${pid}/rules/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  delete: (pid: number, id: number) =>
    request<void>(`/projects/${pid}/rules/${id}`, { method: 'DELETE' }),
};

// Rulesets
export const rulesets = {
  list: (pid: number) => request<Ruleset[]>(`/projects/${pid}/rulesets`),
  get: (pid: number, id: number) => request<Ruleset>(`/projects/${pid}/rulesets/${id}`),
  create: (pid: number, data: Omit<Ruleset, 'id'>) =>
    request<Ruleset>(`/projects/${pid}/rulesets`, { method: 'POST', body: JSON.stringify(data) }),
  update: (pid: number, id: number, data: Partial<Ruleset>) =>
    request<Ruleset>(`/projects/${pid}/rulesets/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),
  delete: (pid: number, id: number) =>
    request<void>(`/projects/${pid}/rulesets/${id}`, { method: 'DELETE' }),
};

// Patterns
export const patterns = {
  list: (pid: number) => request<Pattern[]>(`/projects/${pid}/patterns`),
  get: (pid: number, id: number) => request<Pattern>(`/projects/${pid}/patterns/${id}`),
  create: (pid: number, data: Omit<Pattern, 'id'>) =>
    request<Pattern>(`/projects/${pid}/patterns`, { method: 'POST', body: JSON.stringify(data) }),
  update: (pid: number, id: number, data: Partial<Pattern>) =>
    request<Pattern>(`/projects/${pid}/patterns/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),
  delete: (pid: number, id: number) =>
    request<void>(`/projects/${pid}/patterns/${id}`, { method: 'DELETE' }),
};

// Analysis events (matches Rust AnalysisEvent serde output)
export type AnalysisEvent =
  | { type: 'rule_match'; data: RuleMatch }
  | { type: 'pattern_match'; data: PatternMatch }
  | { type: 'state_change'; data: StateChange }
  | { type: 'progress'; data: { lines_processed: number } }
  | {
      type: 'complete';
      data: {
        total_lines: number;
        total_rule_matches: number;
        total_pattern_matches: number;
        total_state_changes: number;
      };
    }
  | { type: 'error'; data: { message: string } };

export interface StreamingCallbacks {
  onRuleMatch: (rm: RuleMatch) => void;
  onPatternMatch: (pm: PatternMatch) => void;
  onStateChange: (sc: StateChange) => void;
  onProgress: (linesProcessed: number) => void;
  onComplete: (totals: {
    total_lines: number;
    total_rule_matches: number;
    total_pattern_matches: number;
    total_state_changes: number;
  }) => void;
  onError: (message: string) => void;
}

function buildTimeRangeParams(timeRange?: TimeRange): string {
  if (!timeRange) return '';
  const params = new URLSearchParams();
  if (timeRange.start) params.set('start', timeRange.start);
  if (timeRange.end) params.set('end', timeRange.end);
  const qs = params.toString();
  return qs ? `?${qs}` : '';
}

// Analysis
export const analysis = {
  run: (pid: number, timeRange?: TimeRange) =>
    request<AnalysisResult>(`/projects/${pid}/analyze${buildTimeRangeParams(timeRange)}`, {
      method: 'POST',
    }),
  runStreaming: (
    pid: number,
    callbacks: StreamingCallbacks,
    timeRange?: TimeRange,
  ): { close: () => void } => {
    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const qs = buildTimeRangeParams(timeRange);
    const ws = new WebSocket(
      `${proto}//${window.location.host}/api/projects/${pid}/analyze/ws${qs}`,
    );

    ws.onmessage = (ev) => {
      const event: AnalysisEvent = JSON.parse(ev.data);
      switch (event.type) {
        case 'rule_match':
          callbacks.onRuleMatch(event.data);
          break;
        case 'pattern_match':
          callbacks.onPatternMatch(event.data);
          break;
        case 'state_change':
          callbacks.onStateChange(event.data);
          break;
        case 'progress':
          callbacks.onProgress(event.data.lines_processed);
          break;
        case 'complete':
          callbacks.onComplete(event.data);
          ws.close();
          break;
        case 'error':
          callbacks.onError(event.data.message);
          ws.close();
          break;
      }
    };

    ws.onerror = () => {
      callbacks.onError('WebSocket connection failed');
    };

    return { close: () => ws.close() };
  },
  exportJson: (pid: number, timeRange?: TimeRange, include?: string[]) => {
    const params = new URLSearchParams({ format: 'json' });
    if (timeRange?.start) params.set('start', timeRange.start);
    if (timeRange?.end) params.set('end', timeRange.end);
    if (include && include.length > 0) params.set('include', include.join(','));
    window.open(`${BASE}/projects/${pid}/analyze/export?${params}`);
  },
  exportCsv: async (pid: number, section: string, timeRange?: TimeRange) => {
    const params = new URLSearchParams({ format: 'csv', section });
    if (timeRange?.start) params.set('start', timeRange.start);
    if (timeRange?.end) params.set('end', timeRange.end);
    const res = await fetch(`${BASE}/projects/${pid}/analyze/export?${params}`);
    if (!res.ok) throw new Error(`Export failed: ${res.status}`);
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `analysis-${section.replace('_', '-')}.csv`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  },
  detectTemplate: (pid: number, data: { content: string }) =>
    request<SourceTemplate>(`/projects/${pid}/detect-template`, {
      method: 'POST',
      body: JSON.stringify(data),
    }),
  suggestRule: (pid: number, data: { text: string; context_lines?: string[] }) =>
    request<SuggestRuleResponse>(`/projects/${pid}/suggest-rule`, {
      method: 'POST',
      body: JSON.stringify(data),
    }),
};

// Clustering
export const clustering = {
  run: (pid: number, timeRange?: TimeRange) =>
    request<ClusterResult>(`/projects/${pid}/cluster${buildTimeRangeParams(timeRange)}`, {
      method: 'POST',
    }),
};
