import { describe, expect, it } from 'vitest';
import { cycleReference } from '../src/formulaRef';

const cycle = (text: string, caret: number) => cycleReference(text, caret)?.text ?? null;

describe('F4 in a formula (CORE-UI-004)', () => {
  it('cycles A1 → $A$1 → A$1 → $A1 → A1', () => {
    let t = '=A1+1';
    const seen: string[] = [];
    for (let i = 0; i < 4; i++) {
      t = cycle(t, 3)!;
      seen.push(t);
    }
    expect(seen).toEqual(['=$A$1+1', '=A$1+1', '=$A1+1', '=A1+1']);
  });

  it('changes the reference at the caret only, both ends of a range', () => {
    expect(cycle('=SUM(A1:B2)*C3', 7)).toBe('=SUM($A$1:$B$2)*C3');
    expect(cycle('=SUM(A1:B2)*C3', 14)).toBe('=SUM(A1:B2)*$C$3');
    const r = cycleReference('=A1+B22', 7);
    expect(r).toEqual({ text: '=A1+$B$22', start: 4, end: 9 });
  });

  it('leaves names, functions, sheet names, strings and plain text alone', () => {
    expect(cycle('=LOG10(5)', 4)).toBeNull();
    expect(cycle('="A1"&B1', 3)).toBeNull();
    expect(cycle('="A1"&B1', 8)).toBe('="A1"&$B$1');
    expect(cycle('=Sheet1!A1', 4)).toBeNull();
    expect(cycle('=Sheet1!A1', 10)).toBe('=Sheet1!$A$1');
    expect(cycle('A1', 2)).toBeNull();
    expect(cycle('=1+2', 2)).toBeNull();
  });
});
