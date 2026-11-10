import { describe, expect, it } from 'vitest';
import { cellWord } from '../src/thesaurus';

describe('BlueSheet thesaurus', () => {
  it('takes the word of a cell that holds exactly one word', () => {
    expect(cellWord('happy', 'happy')).toBe('happy');
    expect(cellWord(' Quick ', ' Quick ')).toBe('Quick');
    expect(cellWord('two words', 'two words')).toBeNull();
    expect(cellWord('=A1', 'happy')).toBeNull();
    expect(cellWord('42', '42')).toBeNull();
    expect(cellWord('', '')).toBeNull();
  });
});
