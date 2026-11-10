/**
 * BlueScript pane (CORE-SCRIPT-001): code editor with completion from the API
 * typings, Run / Stop, an output console and the user's saved scripts.
 */
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import { complete, insertion, type Completion } from './completion';
import type { ScriptHost } from './host';
import { defaultRunner, type ScriptRun, type ScriptRunner } from './runner';
import type { LogLevel, RunResult, ScriptApp, ScriptLimits } from './sandbox';
import { scriptStore, type SavedScript, type ScriptStore } from './store';
import styles from './ScriptPane.module.css';

export interface ScriptPaneProps {
  app: ScriptApp;
  /** Host for a run; null while no file is open. */
  host: () => ScriptHost | null;
  runner?: ScriptRunner;
  store?: ScriptStore;
  limits?: Partial<ScriptLimits>;
}

interface Line {
  id: number;
  level: LogLevel | 'status';
  text: string;
}

const MAX_LINES = 2_000;

export function ScriptPane({ app, host, runner, store, limits }: ScriptPaneProps) {
  const { t } = useTranslation();
  const scripts = useMemo(() => store ?? scriptStore(app), [store, app]);
  const run = useMemo(() => runner ?? defaultRunner(), [runner]);
  const [saved, setSaved] = useState<SavedScript[]>(() => scripts.list());
  const [current, setCurrent] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [code, setCode] = useState(() => t(`script.sample.${app}`));
  const [lines, setLines] = useState<Line[]>([]);
  const [running, setRunning] = useState<ScriptRun | null>(null);
  const [hint, setHint] = useState<Completion | null>(null);
  const [active, setActive] = useState(0);
  const area = useRef<HTMLTextAreaElement>(null);
  const seq = useRef(0);

  const push = useCallback((level: Line['level'], text: string) => {
    setLines((l) => {
      const next = [...l, { id: ++seq.current, level, text }];
      return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
    });
  }, []);

  useEffect(() => () => running?.stop(), [running]);

  const describe = (r: RunResult) => {
    if (r.ok) {
      if (r.value !== undefined) push('log', JSON.stringify(r.value));
      push('status', t('script.status.done', { ms: r.ms, calls: r.calls }));
      return;
    }
    const e = r.error!;
    push(
      'error',
      `${t(`script.error.${e.kind}`)}${e.kind === 'error' || e.kind === 'syntax' ? `: ${e.message}` : ''}`,
    );
    if (e.stack) push('error', e.stack.trim());
  };

  const start = () => {
    if (running) return;
    const h = host();
    if (!h) {
      push('error', t('script.noDocument'));
      return;
    }
    setHint(null);
    push('status', t('script.status.running'));
    const r = run.run(code, h, { ...limits, log: (level, text) => push(level, text) });
    setRunning(r);
    void r.done.then((res) => {
      describe(res);
      setRunning(null);
    });
  };

  const save = () => {
    const s = scripts.save({
      ...(current ? { id: current } : {}),
      name: name || t('script.untitled'),
      code,
    });
    setCurrent(s.id);
    setName(s.name);
    setSaved(scripts.list());
    push('status', t('script.savedAs', { name: s.name }));
  };

  const remove = () => {
    if (!current) return;
    scripts.remove(current);
    setSaved(scripts.list());
    setCurrent(null);
    setName('');
  };

  const pick = (id: string) => {
    if (!id) {
      setCurrent(null);
      setName('');
      setCode(t(`script.sample.${app}`));
      return;
    }
    const s = saved.find((x) => x.id === id);
    if (!s) return;
    setCurrent(s.id);
    setName(s.name);
    setCode(s.code);
  };

  const refreshHint = (text: string, caret: number, force = false) => {
    const c = complete(app, text, caret, force);
    setHint(c);
    setActive(0);
  };

  const accept = (index: number) => {
    const el = area.current;
    const c = hint;
    const m = c?.items[index];
    if (!el || !c || !m) return;
    const caret = el.selectionStart;
    const ins = insertion(m);
    const next = code.slice(0, c.from) + ins.text + code.slice(caret);
    const at = c.from + ins.text.length - ins.caretBack;
    setCode(next);
    setHint(null);
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(at, at);
    });
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      start();
      return;
    }
    if (e.ctrlKey && e.key === ' ') {
      e.preventDefault();
      refreshHint(code, e.currentTarget.selectionStart, true);
      return;
    }
    if (hint) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const n = hint.items.length;
        setActive((a) => (a + (e.key === 'ArrowDown' ? 1 : n - 1)) % n);
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        accept(active);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setHint(null);
        return;
      }
    }
    if (e.key === 'Tab' && !e.shiftKey) {
      // Indent instead of leaving the editor.
      e.preventDefault();
      const el = e.currentTarget;
      const s = el.selectionStart;
      const next = `${code.slice(0, s)}  ${code.slice(el.selectionEnd)}`;
      setCode(next);
      requestAnimationFrame(() => el.setSelectionRange(s + 2, s + 2));
    }
  };

  const shown = hint?.items.slice(0, 12) ?? [];
  return (
    <div className={styles.pane} data-testid="script-pane">
      <div className={styles.row}>
        <select
          aria-label={t('script.savedList')}
          value={current ?? ''}
          onChange={(e) => pick(e.target.value)}
          data-testid="script-list"
        >
          <option value="">{t('script.newScript')}</option>
          {saved.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
        <input
          aria-label={t('script.name')}
          placeholder={t('script.name')}
          value={name}
          onChange={(e) => setName(e.target.value)}
          data-testid="script-name"
        />
      </div>
      <div className={styles.row}>
        <button
          type="button"
          onClick={start}
          disabled={!!running}
          title={t('script.runHint')}
          data-testid="script-run"
        >
          {t('script.run')}
        </button>
        <button
          type="button"
          onClick={() => running?.stop()}
          disabled={!running}
          data-testid="script-stop"
        >
          {t('script.stop')}
        </button>
        <button type="button" onClick={save} data-testid="script-save">
          {t('script.save')}
        </button>
        <button type="button" onClick={remove} disabled={!current} data-testid="script-delete">
          {t('script.delete')}
        </button>
      </div>
      <textarea
        ref={area}
        className={styles.code}
        aria-label={t('script.code')}
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        wrap="off"
        value={code}
        onChange={(e) => {
          setCode(e.target.value);
          const el = e.target;
          // Suggest after a dot or while a word is typed.
          refreshHint(el.value, el.selectionStart);
        }}
        onKeyDown={onKeyDown}
        onBlur={() => setTimeout(() => setHint(null), 150)}
        data-testid="script-code"
      />
      {shown.length > 0 && (
        <ul className={styles.hints} role="listbox" aria-label={t('script.suggestions')}>
          {shown.map((m, i) => (
            <li
              key={m.name}
              role="option"
              aria-selected={i === active}
              className={i === active ? styles.activeHint : undefined}
              onMouseDown={(e) => {
                e.preventDefault();
                accept(i);
              }}
              title={m.signature}
            >
              <span>{m.name}</span>
              <small>{m.signature}</small>
            </li>
          ))}
        </ul>
      )}
      <div className={styles.consoleHead}>
        <strong>{t('script.output')}</strong>
        <button type="button" onClick={() => setLines([])}>
          {t('script.clear')}
        </button>
      </div>
      <ol className={styles.console} aria-live="polite" data-testid="script-console">
        {lines.map((l) => (
          <li key={l.id} className={styles[l.level]}>
            {l.text}
          </li>
        ))}
      </ol>
    </div>
  );
}
