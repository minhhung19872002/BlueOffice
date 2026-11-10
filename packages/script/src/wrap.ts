/** How user code is wrapped before evaluation (browser sandbox and headless runtime alike). */

/** Wraps the user code: top-level `await` and `return` work, strict mode. */
export function wrapScript(code: string): string {
  return `(async () => {"use strict";\n${code}\n})()`;
}

/** Number of lines the wrapper adds before the user code (for error line numbers). */
export const WRAP_LINES = 1;
