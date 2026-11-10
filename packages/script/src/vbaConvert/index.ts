/**
 * VBA → BlueScript draft converter (CORE-SCRIPT-003).
 *
 * Reads the text of a VBA macro and writes a draft of an equivalent BlueScript
 * for the user to review. Best effort: what has no BlueScript equivalent stays
 * in the draft as `// TODO(VBA): <original line>` and is listed in `warnings`.
 * VBA is never executed, and the draft is never run by the converter.
 *
 * Clean-room: written from the public language specification [MS-VBAL] only.
 */
import type { ScriptApp } from '../sandbox';
import { Emitter, TODO_MARK, type VbaWarning } from './emit';
import { parseVba } from './parser';

export type { VbaWarning } from './emit';
export type { VbaWarningReason } from './ast';
export { TODO_MARK } from './emit';

export interface VbaConversion {
  /** BlueScript source; always parses as JavaScript. */
  code: string;
  /** What could not be converted, in source order. */
  warnings: VbaWarning[];
  /** Names of the procedures found. */
  procedures: string[];
}

/** Longest VBA source converted (characters); the rest is reported and left out. */
export const MAX_VBA_CHARS = 400_000;

export function convertVba(source: string, options: { app: ScriptApp }): VbaConversion {
  const text = source.length > MAX_VBA_CHARS ? source.slice(0, MAX_VBA_CHARS) : source;
  let result: VbaConversion;
  try {
    const emitter = new Emitter(options.app);
    const { code, procedures } = emitter.module(parseVba(text));
    result = { code, procedures, warnings: emitter.warnings };
  } catch {
    // Defensive (e.g. nesting deeper than the stack): keep every line as a TODO.
    const warnings: VbaWarning[] = [];
    const lines = text.split(/\r\n|[\r\n\u2028\u2029]/).map((l, i) => {
      const source = l.trim();
      if (!source) return '';
      warnings.push({ line: i + 1, construct: source.slice(0, 40), reason: 'syntax', source });
      return `// ${TODO_MARK} ${source}`;
    });
    result = { code: `${lines.join('\n')}\n`, procedures: [], warnings };
  }
  if (text.length < source.length) {
    const line = text.split('\n').length;
    result.code += `// ${TODO_MARK} source truncated after ${MAX_VBA_CHARS} characters\n`;
    result.warnings.push({
      line,
      construct: 'source',
      reason: 'syntax',
      source: 'source truncated',
    });
  }
  result.warnings.sort((a, b) => a.line - b.line);
  return result;
}
