export interface RegexPreset {
  label: string;
  pattern: string;
  description: string;
}

export type RegexFieldType =
  | 'continuation_regex'
  | 'extraction_regex'
  | 'content_regex'
  | 'file_name_regex'
  | 'log_content_regex';

export const REGEX_PRESETS: Record<RegexFieldType, RegexPreset[]> = {
  continuation_regex: [
    { label: 'Indented', pattern: '^\\s+', description: 'Lines starting with whitespace' },
    { label: 'Tab-indented', pattern: '^\\t', description: 'Lines starting with a tab' },
    { label: 'Caused by', pattern: '^Caused by:', description: 'Java/Python chained exceptions' },
    {
      label: 'Java stack trace',
      pattern: '^\\s+at\\s+',
      description: 'Java stack trace frames',
    },
  ],
  extraction_regex: [
    {
      label: 'Bracketed',
      pattern: '\\[([^\\]]+)\\]',
      description: 'Content inside square brackets',
    },
    { label: 'First token', pattern: '^(\\S+)', description: 'First non-whitespace token' },
  ],
  content_regex: [
    {
      label: 'Skip first token',
      pattern: '^\\S+\\s+(.*)',
      description: 'Everything after the first token',
    },
    {
      label: 'Skip bracketed prefix',
      pattern: '^\\[.*?\\]\\s+(.*)',
      description: 'Everything after a [bracketed] prefix',
    },
  ],
  file_name_regex: [
    { label: 'Nginx logs', pattern: 'nginx.*\\.log$', description: 'Nginx log files' },
    { label: 'Syslog', pattern: 'syslog.*', description: 'Syslog files' },
    { label: 'JSON files', pattern: '\\.json$', description: 'JSON log files' },
  ],
  log_content_regex: [
    {
      label: 'Syslog format',
      pattern: '^\\w+\\s+\\d+\\s+[\\d:]+',
      description: 'Month day time (e.g. Jan 15 10:30:00)',
    },
    {
      label: 'ISO date prefix',
      pattern: '^\\d{4}-\\d{2}-\\d{2}',
      description: 'Lines starting with YYYY-MM-DD',
    },
    {
      label: 'IP prefix',
      pattern: '^\\d+\\.\\d+\\.\\d+\\.\\d+',
      description: 'Lines starting with an IP address',
    },
  ],
};
