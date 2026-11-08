import { useEffect, useRef } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import type { SpellInfo } from '@blueoffice/platform';

const USER_WORDS_KEY = 'bo.spell.userWords';

/** Words the user added to the dictionary (kept by this browser / app; a per-user convenience). */
export function loadUserWords(): string[] {
  try {
    const raw = localStorage.getItem(USER_WORDS_KEY);
    const list: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(list) ? list.filter((w): w is string => typeof w === 'string') : [];
  } catch {
    return [];
  }
}

export function saveUserWord(word: string): void {
  try {
    const words = loadUserWords();
    if (!words.includes(word))
      localStorage.setItem(USER_WORDS_KEY, JSON.stringify([...words, word]));
  } catch {
    // Storage unavailable (private mode): the word stays for this session only.
  }
}

interface Props {
  menu: { x: number; y: number; info: SpellInfo };
  onPick: (text: string) => void;
  onIgnore: (word: string) => void;
  onAdd: (word: string) => void;
  onClose: () => void;
}

/** Corrections for a misspelt word, opened by a right click on it. */
export function SpellMenu({ menu, onPick, onIgnore, onAdd, onClose }: Props) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.querySelector<HTMLButtonElement>('button')?.focus();
    const close = (e: MouseEvent | KeyboardEvent) => {
      if (
        e instanceof KeyboardEvent ? e.key === 'Escape' : !ref.current?.contains(e.target as Node)
      )
        onClose();
    };
    window.addEventListener('mousedown', close);
    window.addEventListener('keydown', close);
    return () => {
      window.removeEventListener('mousedown', close);
      window.removeEventListener('keydown', close);
    };
  }, [onClose]);
  const item: React.CSSProperties = {
    display: 'block',
    width: '100%',
    textAlign: 'left',
    padding: '4px 12px',
    border: 0,
    background: 'transparent',
    font: 'inherit',
    cursor: 'pointer',
  };
  return (
    <div
      ref={ref}
      role="menu"
      aria-label={t('spell.title')}
      data-testid="spell-menu"
      style={{
        position: 'fixed',
        left: menu.x,
        top: menu.y,
        zIndex: 50,
        minWidth: 180,
        padding: '4px 0',
        background: 'var(--bo-surface, #fff)',
        color: 'var(--bo-text, #1f1f1f)',
        border: '1px solid var(--bo-border, #c8c8c8)',
        borderRadius: 6,
        boxShadow: '0 4px 16px rgba(0,0,0,.18)',
        font: '13px system-ui, sans-serif',
      }}
    >
      {menu.info.suggestions.length === 0 && (
        <div style={{ ...item, cursor: 'default', opacity: 0.7 }}>{t('spell.noSuggestions')}</div>
      )}
      {menu.info.suggestions.map((s) => (
        <button
          key={s}
          type="button"
          role="menuitem"
          style={{ ...item, fontWeight: 600 }}
          onClick={() => onPick(s)}
        >
          {s}
        </button>
      ))}
      <hr style={{ margin: '4px 0', border: 0, borderTop: '1px solid var(--bo-border, #ddd)' }} />
      <button type="button" role="menuitem" style={item} onClick={() => onIgnore(menu.info.word)}>
        {t('spell.ignoreAll')}
      </button>
      <button type="button" role="menuitem" style={item} onClick={() => onAdd(menu.info.word)}>
        {t('spell.addToDictionary')}
      </button>
    </div>
  );
}
