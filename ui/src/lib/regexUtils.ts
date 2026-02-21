/**
 * Shared regex helpers for rule creation and editing.
 * Handles both JS (?<name>) and Rust/Python (?P<name>) named groups.
 */

/** Finds named and unnamed capture groups in a regex pattern. */
export function detectGroups(pattern: string): string[] {
  const groups: string[] = [];
  const namedGroupRegex = /\((?:\?(?:P?<([^>]+)>))?/g;
  let match;
  let idx = 0;
  while ((match = namedGroupRegex.exec(pattern)) !== null) {
    if (match[1]) {
      groups.push(match[1]);
    } else {
      groups.push(`group_${idx}`);
    }
    idx++;
  }
  return groups;
}

/** Converts Rust/Python (?P<name>) syntax to JS (?<name>). */
export function toJsRegex(pattern: string): string {
  return pattern.replace(/\(\?P</g, '(?<');
}

export interface RegexTestResult {
  status: 'match' | 'no-match' | 'error';
  message: string;
  groups: Record<string, string>;
}

/** Compiles a regex, runs it against text, returns a structured result with named groups. */
export function testPattern(pattern: string, text: string): RegexTestResult {
  try {
    const jsPattern = toJsRegex(pattern);
    const re = new RegExp(jsPattern);
    const m = re.exec(text);
    if (m) {
      const groups: Record<string, string> = {};
      if (m.groups) {
        for (const [k, v] of Object.entries(m.groups)) {
          if (v !== undefined) groups[k] = v;
        }
      }
      return {
        status: 'match',
        message: `Match: "${m[0]}"`,
        groups,
      };
    }
    return { status: 'no-match', message: 'No match', groups: {} };
  } catch (e: any) {
    return { status: 'error', message: `Invalid regex: ${e.message}`, groups: {} };
  }
}

/**
 * Validates whether a regex pattern compiles successfully.
 * Handles Rust/Python (?P<name>) syntax by converting to JS (?<name>).
 * Returns null if valid, or an error message string if invalid.
 * Empty/whitespace-only patterns are considered valid (fields are optional).
 */
export function validateRegex(pattern: string): string | null {
  if (!pattern.trim()) return null;
  try {
    new RegExp(toJsRegex(pattern));
    return null;
  } catch (e: any) {
    return e.message;
  }
}
