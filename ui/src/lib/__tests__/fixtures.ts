import type {
  LogLine,
  RuleMatch,
  PatternMatch,
  StateChange,
  Source,
  LogRule,
  Pattern,
  Ruleset,
  StateValue,
  AnalysisResult,
  SuggestRuleResponse,
} from '../api';

export interface TimelineEvent {
  id: number;
  type: 'rule' | 'pattern';
  timestamp: number;
  sourceId: number | null;
  ruleId?: number;
  patternId?: number;
  ruleMatch?: RuleMatch;
  patternMatch?: PatternMatch;
  colorIndex: number;
}

export function makeLogLine(overrides: Partial<LogLine> = {}): LogLine {
  return {
    timestamp: '2024-01-15T10:30:00.000',
    source_id: 1,
    raw: 'ERROR something failed',
    content: 'something failed',
    ...overrides,
  };
}

export function makeRuleMatch(overrides: Partial<RuleMatch> = {}): RuleMatch {
  return {
    rule_id: 1,
    source_id: 1,
    log_line: makeLogLine(),
    extracted_state: {},
    ...overrides,
  };
}

export function makePatternMatch(overrides: Partial<PatternMatch> = {}): PatternMatch {
  return {
    pattern_id: 1,
    timestamp: '2024-01-15T10:30:05.000',
    state_snapshot: {},
    ...overrides,
  };
}

export function makeSource(overrides: Partial<Source> = {}): Source {
  return {
    id: 1,
    name: 'app.log',
    template_id: 1,
    file_path: '/var/log/app.log',
    ...overrides,
  };
}

export function makeRule(overrides: Partial<LogRule> = {}): LogRule {
  return {
    id: 1,
    name: 'Error Rule',
    match_mode: 'Any',
    match_rules: [{ id: 1, pattern: 'ERROR' }],
    extraction_rules: [],
    ...overrides,
  };
}

export function makeRuleWithExtractions(overrides: Partial<LogRule> = {}): LogRule {
  return {
    id: 1,
    name: 'Extraction Rule',
    match_mode: 'Any',
    match_rules: [{ id: 1, pattern: 'ERROR (?P<message>.+)' }],
    extraction_rules: [
      {
        id: 1,
        extraction_type: 'Parsed',
        state_key: 'message',
        pattern: 'ERROR (?P<message>.+)',
        static_value: null,
        mode: 'Replace',
      },
      {
        id: 2,
        extraction_type: 'Static',
        state_key: 'level',
        pattern: null,
        static_value: 'error',
        mode: 'Replace',
      },
    ],
    ...overrides,
  };
}

export function makePattern(overrides: Partial<Pattern> = {}): Pattern {
  return {
    id: 1,
    name: 'Failure Pattern',
    predicates: [],
    ...overrides,
  };
}

export function makeRuleTimelineEvent(overrides: Partial<TimelineEvent> = {}): TimelineEvent {
  return {
    id: 0,
    type: 'rule',
    timestamp: Date.parse('2024-01-15T10:30:00.000Z'),
    sourceId: 1,
    ruleId: 1,
    ruleMatch: makeRuleMatch(),
    colorIndex: 1,
    ...overrides,
  };
}

export function makePatternTimelineEvent(overrides: Partial<TimelineEvent> = {}): TimelineEvent {
  return {
    id: 100,
    type: 'pattern',
    timestamp: Date.parse('2024-01-15T10:30:05.000Z'),
    sourceId: null,
    patternId: 1,
    patternMatch: makePatternMatch(),
    colorIndex: -1,
    ...overrides,
  };
}

export function makeRuleset(overrides: Partial<Ruleset> = {}): Ruleset {
  return {
    id: 1,
    name: 'Default Ruleset',
    template_id: 1,
    rule_ids: [],
    ...overrides,
  };
}

export function makeSuggestRuleResponse(
  overrides: Partial<SuggestRuleResponse> = {},
): SuggestRuleResponse {
  return {
    pattern: 'ERROR (?P<message>.+)',
    capture_groups: ['message'],
    ...overrides,
  };
}

export function makeStateChange(overrides: Partial<StateChange> = {}): StateChange {
  return {
    timestamp: '2024-01-15T10:30:00.000',
    source_id: 1,
    source_name: 'app.log',
    state_key: 'status',
    old_value: null,
    new_value: { String: 'error_detected' },
    rule_id: 1,
    ...overrides,
  };
}

export function makeAnalysisResult(overrides: Partial<AnalysisResult> = {}): AnalysisResult {
  return {
    rule_matches: [makeRuleMatch()],
    pattern_matches: [makePatternMatch()],
    state_changes: [],
    ...overrides,
  };
}
