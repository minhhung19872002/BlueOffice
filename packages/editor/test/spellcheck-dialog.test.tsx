// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, EditResult, ProofIssue, ProofStory } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  LanguageDialog,
  SpellCheckDialog,
  loadProofOptions,
  nextIssue,
  utf8Length,
} from '../src/SpellCheckDialog';
import { loadIgnoredWords, loadUserWords } from '../src/SpellMenu';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const body: ProofStory = { kind: 'body' };
const header: ProofStory = { kind: 'header', section: 0, variant: 'default' };

const issue = (
  order: number,
  story: ProofStory,
  para: number,
  start: number,
  word: string,
  extra: Partial<ProofIssue> = {},
): ProofIssue => ({
  story,
  order,
  para,
  start,
  end: start + utf8Length(word),
  word,
  suggestions: [],
  before: 'Some ',
  after: ' text.',
  ...extra,
});

/** A fake engine holding a list of issues; corrections and ignores remove the issues they cover. */
function setup(initial: ProofIssue[]) {
  let issues = [...initial];
  const commands: EditCommand[] = [];
  let options = { ignoreCaps: true, ignoreDigits: true, autoLanguage: false };
  const run = vi.fn(async (c: EditCommand): Promise<EditResult | null> => {
    commands.push(c);
    switch (c.cmd) {
      case 'proofCheck':
        return {
          proof: {
            issues,
            unchecked: ['fr-FR'],
            languages: ['vi', 'en'],
            stats: {
              words: 12,
              characters: 50,
              sentences: 3,
              paragraphs: 2,
              wordsPerSentence: 4,
              sentencesPerParagraph: 1.5,
              charactersPerWord: 50 / 12,
            },
          },
        } as EditResult;
      case 'proofOptions':
        options = {
          ignoreCaps: c.ignoreCaps ?? options.ignoreCaps,
          ignoreDigits: c.ignoreDigits ?? options.ignoreDigits,
          autoLanguage: c.autoLanguage ?? options.autoLanguage,
        };
        return { proofOptions: options } as EditResult;
      case 'proofEdit':
        if (c.edits.some((e) => e.text !== undefined))
          issues = issues.filter(
            (i) =>
              !c.edits.some(
                (e) => e.para === i.para && e.start === i.start && e.story.kind === i.story.kind,
              ),
          );
        return {} as EditResult;
      case 'spellIgnore':
      case 'spellAddWord':
        issues = issues.filter((i) => i.grammar || i.word !== c.word);
        return {} as EditResult;
      case 'grammarIgnore':
        issues = issues.filter((i) => i.rule !== c.rule || i.word !== c.text);
        return {} as EditResult;
      default:
        return null;
    }
  });
  const onClose = vi.fn();
  const onShow = vi.fn();
  wrap(<SpellCheckDialog run={run} onClose={onClose} onShow={onShow} />);
  return { commands, onClose, onShow };
}

const word = () => screen.getByTestId('spellcheck-word').textContent;

describe('nextIssue', () => {
  it('finds the first issue at or after the cursor that was not skipped', () => {
    const list = [
      issue(0, body, 0, 4, 'a'),
      issue(0, body, 2, 0, 'b'),
      issue(2, header, 0, 0, 'c'),
    ];
    expect(nextIssue(list, { order: 0, para: 0, start: 0 }, new Set())?.word).toBe('a');
    expect(nextIssue(list, { order: 0, para: 0, start: 5 }, new Set())?.word).toBe('b');
    expect(nextIssue(list, { order: 0, para: 2, start: 0 }, new Set(['0:2:0::b']))?.word).toBe('c');
    expect(nextIssue(list, { order: 3, para: 0, start: 0 }, new Set())).toBeNull();
  });
});

describe('SpellCheckDialog', () => {
  it('walks the issues of every story in order and ends with a summary', async () => {
    const { onShow } = setup([
      issue(0, body, 0, 4, 'documnet', { suggestions: ['document'], lang: 'en-US' }),
      issue(0, body, 1, 0, 'xin', {
        grammar: true,
        rule: 'capitalize',
        suggestions: ['Xin'],
      }),
      issue(1, header, 0, 0, 'trangg', { suggestions: ['trang'] }),
    ]);
    await screen.findByTestId('spellcheck-issue');
    expect(word()).toBe('documnet');
    expect(screen.getByTestId('spellcheck-context').textContent).toBe('Some documnet text.');
    expect((screen.getByTestId('spellcheck-replacement') as HTMLInputElement).value).toBe(
      'document',
    );
    expect((screen.getByTestId('spellcheck-language') as HTMLSelectElement).value).toBe('en-US');
    expect(onShow).toHaveBeenCalledWith(expect.objectContaining({ word: 'documnet' }));
    fireEvent.click(screen.getByTestId('spellcheck-ignore-once'));
    // A grammar issue: rule text and explanation, no dictionary actions.
    await waitFor(() => expect(word()).toBe('xin'));
    expect(screen.getByTestId('spellcheck-kind').textContent).toBe('Grammar or style');
    expect(screen.queryByTestId('spellcheck-add')).toBeNull();
    fireEvent.click(screen.getByTestId('spellcheck-explain-button'));
    expect(screen.getByTestId('spellcheck-explain').textContent).toBe(
      'A new sentence begins with a capital letter.',
    );
    fireEvent.click(screen.getByTestId('spellcheck-ignore-once'));
    await waitFor(() => expect(word()).toBe('trangg'));
    expect(screen.getByTestId('spellcheck-where').textContent).toBe('In: header');
    fireEvent.click(screen.getByTestId('spellcheck-ignore-once'));
    await screen.findByTestId('spellcheck-done');
    expect(screen.getByTestId('spellcheck-summary').textContent).toBe(
      'Replaced: 0 · skipped: 3 · added to the dictionary: 0',
    );
    expect(screen.getByTestId('spellcheck-unchecked').textContent).toBe(
      'Not checked, no dictionary for: French',
    );
    expect(screen.getByTestId('spellcheck-statWords').textContent).toBe('12');
    expect(screen.getByTestId('spellcheck-statWordsPerSentence').textContent).toBe('4.0');
    expect(screen.getByTestId('spellcheck-statCharactersPerWord').textContent).toBe('4.2');
  });

  it('replaces one word or every occurrence, in one command each', async () => {
    const { commands } = setup([
      issue(0, body, 0, 4, 'lỗii', { suggestions: ['lỗi'] }),
      issue(0, body, 0, 20, 'lỗii', { suggestions: ['lỗi'] }),
      issue(0, body, 3, 0, 'sai', { suggestions: ['sải'] }),
      issue(2, { kind: 'comment', index: 0 }, 0, 6, 'lỗii', { suggestions: ['lỗi'] }),
    ]);
    await screen.findByTestId('spellcheck-issue');
    fireEvent.click(screen.getByTestId('spellcheck-change-all'));
    await waitFor(() => expect(word()).toBe('sai'));
    const all = commands.find((c) => c.cmd === 'proofEdit');
    expect(all).toEqual({
      cmd: 'proofEdit',
      edits: [
        { story: body, para: 0, start: 4, end: 10, text: 'lỗi' },
        { story: body, para: 0, start: 20, end: 26, text: 'lỗi' },
        { story: { kind: 'comment', index: 0 }, para: 0, start: 6, end: 12, text: 'lỗi' },
      ],
    });
    // Type another replacement and change this one only.
    fireEvent.change(screen.getByTestId('spellcheck-replacement'), { target: { value: 'say' } });
    fireEvent.click(screen.getByTestId('spellcheck-change'));
    await screen.findByTestId('spellcheck-done');
    expect(commands.filter((c) => c.cmd === 'proofEdit').at(-1)).toEqual({
      cmd: 'proofEdit',
      edits: [{ story: body, para: 3, start: 0, end: 3, text: 'say' }],
    });
    expect(screen.getByTestId('spellcheck-summary').textContent).toBe(
      'Replaced: 4 · skipped: 0 · added to the dictionary: 0',
    );
  });

  it('keeps ignored and added words, and sets the language of a word', async () => {
    const { commands } = setup([
      issue(0, body, 0, 0, 'BlueOffise'),
      issue(0, body, 0, 12, 'colour', { lang: 'vi-VN' }),
      issue(0, body, 1, 0, 'Zalo'),
    ]);
    await screen.findByTestId('spellcheck-issue');
    fireEvent.click(screen.getByTestId('spellcheck-add'));
    await waitFor(() => expect(word()).toBe('colour'));
    fireEvent.change(screen.getByTestId('spellcheck-language'), { target: { value: 'en-GB' } });
    await waitFor(() =>
      expect(commands).toContainEqual({
        cmd: 'proofEdit',
        edits: [{ story: body, para: 0, start: 12, end: 18, lang: 'en-GB' }],
      }),
    );
    await waitFor(() => expect(commands.at(-1)?.cmd).toBe('proofCheck'));
    // The fake engine still reports it: ignore it everywhere.
    fireEvent.click(screen.getByTestId('spellcheck-ignore-all'));
    await waitFor(() => expect(word()).toBe('Zalo'));
    fireEvent.click(screen.getByTestId('spellcheck-ignore-all'));
    await screen.findByTestId('spellcheck-done');
    expect(loadUserWords()).toEqual(['BlueOffise']);
    expect(loadIgnoredWords()).toEqual(['colour', 'Zalo']);
    expect(commands).toContainEqual({ cmd: 'spellAddWord', word: 'BlueOffise' });
    expect(commands).toContainEqual({ cmd: 'spellIgnore', word: 'Zalo' });
  });

  it('changes the proofing options and keeps them', async () => {
    const { commands } = setup([]);
    await screen.findByTestId('spellcheck-done');
    expect(screen.getByTestId('spellcheck-summary').textContent).toBe(
      'Nothing to correct was found.',
    );
    fireEvent.click(screen.getByTestId('spellcheck-ignore-digits'));
    await waitFor(() =>
      expect(commands).toContainEqual({ cmd: 'proofOptions', ignoreDigits: false }),
    );
    expect(loadProofOptions()).toEqual({ ignoreDigits: false });
  });
});

describe('LanguageDialog', () => {
  it('sets the language, the do-not-check flag and automatic detection', async () => {
    const commands: EditCommand[] = [];
    const run = vi.fn(async (c: EditCommand) => {
      commands.push(c);
      return c.cmd === 'proofOptions'
        ? ({
            proofOptions: { ignoreCaps: true, ignoreDigits: true, autoLanguage: false },
          } as EditResult)
        : null;
    });
    const onClose = vi.fn();
    wrap(<LanguageDialog run={run} lang="en-US" noProof={false} onClose={onClose} />);
    await waitFor(() =>
      expect((screen.getByTestId('language-auto') as HTMLInputElement).disabled).toBe(false),
    );
    expect((screen.getByTestId('language-select') as HTMLSelectElement).value).toBe('en-US');
    fireEvent.change(screen.getByTestId('language-select'), { target: { value: 'vi-VN' } });
    fireEvent.click(screen.getByTestId('language-noproof'));
    fireEvent.click(screen.getByTestId('language-auto'));
    fireEvent.click(screen.getByTestId('language-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(commands.slice(1)).toEqual([
      { cmd: 'formatText', format: { kind: 'language', lang: 'vi-VN' } },
      { cmd: 'formatText', format: { kind: 'noProof', on: true } },
      { cmd: 'proofOptions', autoLanguage: true },
    ]);
    expect(loadProofOptions()).toEqual({ autoLanguage: true });
  });
});
