// ---- Types ----

export interface Project {
  id: number;
  name: string;
  created_at: string;
}

export interface SourceTemplate {
  id: number;
  name: string;
  timestamp_format: string;
  line_delimiter: string;
  content_regex: string | null;
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

export interface AnalysisResult {
  rule_matches: RuleMatch[];
  pattern_matches: PatternMatch[];
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
  create: (data: { name: string }) => request<Project>('/projects', { method: 'POST', body: JSON.stringify(data) }),
  update: (id: number, data: Partial<Project>) => request<Project>(`/projects/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  delete: (id: number) => request<void>(`/projects/${id}`, { method: 'DELETE' }),
};

// Templates
export const templates = {
  list: (pid: number) => request<SourceTemplate[]>(`/projects/${pid}/templates`),
  get: (pid: number, id: number) => request<SourceTemplate>(`/projects/${pid}/templates/${id}`),
  create: (pid: number, data: Omit<SourceTemplate, 'id'>) => request<SourceTemplate>(`/projects/${pid}/templates`, { method: 'POST', body: JSON.stringify(data) }),
  update: (pid: number, id: number, data: Partial<SourceTemplate>) => request<SourceTemplate>(`/projects/${pid}/templates/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  delete: (pid: number, id: number) => request<void>(`/projects/${pid}/templates/${id}`, { method: 'DELETE' }),
};

// Sources
export const sources = {
  list: (pid: number) => request<Source[]>(`/projects/${pid}/sources`),
  get: (pid: number, id: number) => request<Source>(`/projects/${pid}/sources/${id}`),
  create: (pid: number, data: Omit<Source, 'id'>) => request<Source>(`/projects/${pid}/sources`, { method: 'POST', body: JSON.stringify(data) }),
  upload: async (pid: number, id: number, file: File): Promise<Source> => {
    const form = new FormData();
    form.append('file', file);
    const res = await fetch(`${BASE}/projects/${pid}/sources/${id}/upload`, { method: 'POST', body: form });
    if (!res.ok) throw new Error(`Upload failed: ${res.status}`);
    return res.json();
  },
  delete: (pid: number, id: number) => request<void>(`/projects/${pid}/sources/${id}`, { method: 'DELETE' }),
};

// Rules
export const rules = {
  list: (pid: number) => request<LogRule[]>(`/projects/${pid}/rules`),
  get: (pid: number, id: number) => request<LogRule>(`/projects/${pid}/rules/${id}`),
  create: (pid: number, data: Omit<LogRule, 'id'>) => request<LogRule>(`/projects/${pid}/rules`, { method: 'POST', body: JSON.stringify(data) }),
  update: (pid: number, id: number, data: Partial<LogRule>) => request<LogRule>(`/projects/${pid}/rules/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  delete: (pid: number, id: number) => request<void>(`/projects/${pid}/rules/${id}`, { method: 'DELETE' }),
};

// Rulesets
export const rulesets = {
  list: (pid: number) => request<Ruleset[]>(`/projects/${pid}/rulesets`),
  get: (pid: number, id: number) => request<Ruleset>(`/projects/${pid}/rulesets/${id}`),
  create: (pid: number, data: Omit<Ruleset, 'id'>) => request<Ruleset>(`/projects/${pid}/rulesets`, { method: 'POST', body: JSON.stringify(data) }),
  update: (pid: number, id: number, data: Partial<Ruleset>) => request<Ruleset>(`/projects/${pid}/rulesets/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  delete: (pid: number, id: number) => request<void>(`/projects/${pid}/rulesets/${id}`, { method: 'DELETE' }),
};

// Patterns
export const patterns = {
  list: (pid: number) => request<Pattern[]>(`/projects/${pid}/patterns`),
  get: (pid: number, id: number) => request<Pattern>(`/projects/${pid}/patterns/${id}`),
  create: (pid: number, data: Omit<Pattern, 'id'>) => request<Pattern>(`/projects/${pid}/patterns`, { method: 'POST', body: JSON.stringify(data) }),
  update: (pid: number, id: number, data: Partial<Pattern>) => request<Pattern>(`/projects/${pid}/patterns/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  delete: (pid: number, id: number) => request<void>(`/projects/${pid}/patterns/${id}`, { method: 'DELETE' }),
};

// Analysis
export const analysis = {
  run: (pid: number) => request<AnalysisResult>(`/projects/${pid}/analyze`, { method: 'POST' }),
  detectTemplate: (pid: number, data: { content: string }) => request<SourceTemplate>(`/projects/${pid}/detect-template`, { method: 'POST', body: JSON.stringify(data) }),
  suggestRule: (pid: number, data: { text: string }) => request<LogRule>(`/projects/${pid}/suggest-rule`, { method: 'POST', body: JSON.stringify(data) }),
};
