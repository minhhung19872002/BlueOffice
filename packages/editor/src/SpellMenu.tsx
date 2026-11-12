import { ContextMenu, type MenuLine } from '@blueoffice/ui';
import { useTranslation } from '@blueoffice/i18n';
import type { SpellInfo } from '@blueoffice/platform';

const USER_WORDS_KEY = 'bo.spell.userWords';

/** Words the user added to the dictionary (kept by this browser / app; a per-user convenience). */
export function loadUserWords(): string[] {
  return loadWords(USER_WORDS_KEY);
}

export function saveUserWord(word: string): void {
  saveWord(USER_WORDS_KEY, word);
}

const IGNORED_WORDS_KEY = 'bo.spell.ignoredWords';

/** Words the user chose to ignore everywhere ("ignore all"), kept with the user dictionary. */
export function loadIgnoredWords(): string[] {
  return loadWords(IGNORED_WORDS_KEY);
}

export function saveIgnoredWord(word: string): void {
  saveWord(IGNORED_WORDS_KEY, word);
}

function loadWords(key: string): string[] {
  try {
    const raw = localStorage.getItem(key);
    const list: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(list) ? list.filter((w): w is string => typeof w === 'string') : [];
  } catch {
    return [];
  }
}

function saveWord(key: string, word: string): void {
  try {
    const words = loadWords(key);
    if (!words.includes(word)) localStorage.setItem(key, JSON.stringify([...words, word]));
  } catch {
    // Storage unavailable (private mode): the word stays for this session only.
  }
}

/** What the correction lines of a context menu do. */
export interface SpellMenuActions {
  onPick: (text: string) => void;
  onIgnore: (word: string) => void;
  onAdd: (word: string) => void;
}

/**
 * The spelling / grammar lines that head the shared context menu of a marked word
 * (CORE-UI-003, CORE-SPELL-006): the grammar rule, the suggestions (bold), then
 * "ignore all" and "add to dictionary" (spelling only), closed by a separator.
 */
export function spellMenuLines(
  info: SpellInfo,
  t: (key: string) => string,
  { onPick, onIgnore, onAdd }: SpellMenuActions,
): MenuLine[] {
  const lines: MenuLine[] = [];
  if (info.rule) lines.push({ heading: t(`grammar.${info.rule}`), testId: 'grammar-rule' });
  if (info.suggestions.length === 0) lines.push({ heading: t('spell.noSuggestions') });
  for (const word of info.suggestions)
    lines.push({
      id: `spell.pick.${word}`,
      label: word,
      emphasis: true,
      onSelect: () => onPick(word),
    });
  lines.push({ separator: true });
  lines.push({
    id: 'spell.ignoreAll',
    label: t('spell.ignoreAll'),
    onSelect: () => onIgnore(info.word),
  });
  if (!info.rule)
    lines.push({
      id: 'spell.addToDictionary',
      label: t('spell.addToDictionary'),
      onSelect: () => onAdd(info.word),
    });
  lines.push({ separator: true });
  return lines;
}

interface Props extends SpellMenuActions {
  menu: { x: number; y: number; info: SpellInfo };
  onClose: () => void;
  /** Commands of the app's own context menu shown under the corrections. */
  tail?: MenuLine[];
}

/**
 * Corrections for a misspelt word, opened by a right click on it: the shared context menu
 * (`role=menu`, arrows / Home / End / Enter / Esc) with the corrections first and then the
 * app's commands (`tail`).
 */
export function SpellMenu({ menu, onPick, onIgnore, onAdd, onClose, tail = [] }: Props) {
  const { t } = useTranslation();
  const lines = spellMenuLines(menu.info, (k) => t(k), { onPick, onIgnore, onAdd });
  // No trailing separator without commands below.
  const items = tail.length ? [...lines, ...tail] : lines.slice(0, -1);
  return (
    <ContextMenu
      x={menu.x}
      y={menu.y}
      items={items}
      label={t('spell.title')}
      testId="spell-menu"
      onClose={onClose}
    />
  );
}
