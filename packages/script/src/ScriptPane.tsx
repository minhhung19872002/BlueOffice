/**
 * BlueScript pane (CORE-SCRIPT-001): code editor with completion from the API
 * typings, Run / Stop, an output console and the user's saved scripts.
 * CORE-SCRIPT-002: Record / Stop recording (the recorded script opens here), a
 * keyboard shortcut and Quick Access Toolbar button for a saved script, and
 * server schedules of the open hosted file.
 * CORE-SCRIPT-003: "Convert macro VBA" — pasted VBA text becomes a BlueScript
 * draft (a new unsaved script) with the list of what could not be converted;
 * VBA is never run and the draft is never run automatically.
 */
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import type { ScriptSchedule, ScriptSchedules } from '@blueoffice/platform';
import { complete, insertion, type Completion } from './completion';
import type { ScriptHost } from './host';
import type { ScriptRecorder } from './recorder';
import { defaultRunner, type ScriptRun, type ScriptRunner } from './runner';
import type { LogLevel, RunResult, ScriptApp, ScriptLimits } from './sandbox';
import { scriptStore, type SavedScript, type ScriptStore } from './store';
import { convertVba, type VbaWarning } from './vbaConvert';
import styles from './ScriptPane.module.css';

export interface ScriptPaneProps {
  app: ScriptApp;
  /** Host for a run; null while no file is open. */
  host: () => ScriptHost | null;
  runner?: ScriptRunner;
  store?: ScriptStore;
  limits?: Partial<ScriptLimits>;
  /** Macro recorder of the editor (CORE-SCRIPT-002). */
  recorder?: ScriptRecorder;
  /**
   * Checks a shortcut for a saved script: the name of the command (or script)
   * already bound to it, `invalid` when it cannot be used, null when free.
   */
  checkShortcut?: (shortcut: string, scriptId: string) => string | null;
  /** Server schedules of the open file (hosted sessions). */
  schedules?: ScriptSchedules;
  /**
   * Counter of the "Convert macro VBA to BlueScript" command (CORE-SCRIPT-003):
   * each new value above 0 opens the converter of the pane.
   */
  vbaRequest?: number;
  /** The request was taken (the host resets its counter). */
  onVbaRequestHandled?: () => void;
  /**
   * A module of the open file's VBA project sent by the read-only macro viewer
   * (CORE-VBA-002): each new `id` is converted to a draft at once. The draft is
   * shown for review and never run from here.
   */
  vbaDraft?: VbaDraftRequest | null;
  /** The draft request was taken (the host clears it). */
  onVbaDraftHandled?: () => void;
}

/** VBA source to convert into a BlueScript draft; `id` changes with every request. */
export interface VbaDraftRequest {
  id: number;
  source: string;
  /** Name of the VBA module the source comes from. */
  module?: string;
}

/** `Ctrl+Shift+K` form of a key press; null for a lone modifier. */
export function shortcutFromEvent(
  e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>,
): string | null {
  if (['Control', 'Shift', 'Alt', 'Meta', 'OS', 'AltGraph'].includes(e.key)) return null;
  const mods: string[] = [];
  if (e.ctrlKey || e.metaKey) mods.push('Ctrl');
  if (e.altKey) mods.push('Alt');
  if (e.shiftKey) mods.push('Shift');
  const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  return [...mods, key].join('+');
}

interface Line {
  id: number;
  level: LogLevel | 'status';
  text: string;
}

const MAX_LINES = 2_000;

export function ScriptPane({
  app,
  host,
  runner,
  store,
  limits,
  recorder,
  checkShortcut,
  schedules,
  vbaRequest,
  onVbaRequestHandled,
  vbaDraft,
  onVbaDraftHandled,
}: ScriptPaneProps) {
  const { t } = useTranslation();
  const scripts = useMemo(() => store ?? scriptStore(app), [store, app]);
  const run = useMemo(() => runner ?? defaultRunner(), [runner]);
  const [saved, setSaved] = useState<SavedScript[]>(() => scripts.list());
  const [current, setCurrent] = useState<string | null>(null);
  // A recording stopped from the ribbon while the pane was closed opens here.
  const [pendingRecording] = useState(() => recorder?.takeResult() ?? null);
  const recordedCode = useCallback(
    (source: string) =>
      `// ${t('script.recordedHeader')}\n${source || `// ${t('script.recordedEmpty')}`}\n`,
    [t],
  );
  const [name, setName] = useState(() =>
    pendingRecording !== null ? t('script.recordedName') : '',
  );
  const [code, setCode] = useState(() =>
    pendingRecording !== null ? recordedCode(pendingRecording) : t(`script.sample.${app}`),
  );
  const [lines, setLines] = useState<Line[]>([]);
  const [running, setRunning] = useState<ScriptRun | null>(null);
  const [hint, setHint] = useState<Completion | null>(null);
  const [active, setActive] = useState(0);
  const area = useRef<HTMLTextAreaElement>(null);
  const seq = useRef(0);
  const [recording, setRecording] = useState(() => !!recorder?.recording);
  const [bindError, setBindError] = useState<string | null>(null);
  const [cron, setCron] = useState('0 7 * * 1-5');
  const [planned, setPlanned] = useState<ScriptSchedule[]>([]);
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const [vbaOpen, setVbaOpen] = useState(false);
  const [vbaSource, setVbaSource] = useState('');
  /** Unconverted constructs of the draft in the editor; null: the code is not a draft. */
  const [vbaWarnings, setVbaWarnings] = useState<VbaWarning[] | null>(null);

  const push = useCallback((level: Line['level'], text: string) => {
    setLines((l) => {
      const next = [...l, { id: ++seq.current, level, text }];
      return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
    });
  }, []);

  useEffect(() => () => running?.stop(), [running]);

  // Saved scripts changed elsewhere (bindings, another pane).
  useEffect(() => scripts.subscribe(() => setSaved(scripts.list())), [scripts]);

  const showRecorded = useCallback(
    (source: string) => {
      setCurrent(null);
      setVbaWarnings(null);
      setName(t('script.recordedName'));
      setCode(recordedCode(source));
      push('status', t('script.recordStopped'));
    },
    [t, push, recordedCode],
  );

  useEffect(() => {
    if (!recorder) return undefined;
    return recorder.subscribe((e) => {
      setRecording(e.type === 'start');
      if (e.type === 'stop') {
        recorder.takeResult();
        showRecorded(e.source);
      }
    });
  }, [recorder, showRecorded]);

  // A new request of the editor command opens the converter (state adjusted while rendering).
  const request = vbaRequest ?? 0;
  const [seenRequest, setSeenRequest] = useState(0);
  if (request !== seenRequest) {
    setSeenRequest(request);
    if (request > 0) setVbaOpen(true);
  }
  useEffect(() => {
    if (request > 0) onVbaRequestHandled?.();
  }, [request, onVbaRequestHandled]);

  /** The draft opens as a new unsaved script; it is never run from here. */
  const draftFrom = (source: string, module?: string) => {
    if (!source.trim()) return;
    const r = convertVba(source, { app });
    const header = [
      `// ${t('script.vba.header')}`,
      ...(r.warnings.length
        ? [`// ${t('script.vba.headerTodo', { count: r.warnings.length })}`]
        : []),
    ];
    setCurrent(null);
    setName(module ? t('script.vba.draftNameModule', { module }) : t('script.vba.draftName'));
    setCode(`${header.join('\n')}\n${r.code}`);
    setHint(null);
    setVbaWarnings(r.warnings);
    setVbaOpen(false);
    push(
      'status',
      t('script.vba.converted', { procedures: r.procedures.length, warnings: r.warnings.length }),
    );
  };

  const convertMacro = () => draftFrom(vbaSource);

  // A module sent by the macro viewer becomes the draft (state adjusted while rendering).
  const draftId = vbaDraft?.id ?? 0;
  const [seenDraft, setSeenDraft] = useState(0);
  if (vbaDraft && draftId !== seenDraft) {
    setSeenDraft(draftId);
    draftFrom(vbaDraft.source, vbaDraft.module);
  }
  useEffect(() => {
    if (draftId > 0) onVbaDraftHandled?.();
  }, [draftId, onVbaDraftHandled]);

  const reloadSchedules = useCallback(() => {
    if (!schedules) return;
    schedules.list().then(setPlanned, () => setPlanned([]));
  }, [schedules]);
  useEffect(reloadSchedules, [reloadSchedules]);

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
    // What the script does is not recorded (CORE-SCRIPT-002).
    const done = recorder ? recorder.pause(() => r.done) : r.done;
    void done.then((res) => {
      describe(res);
      setRunning(null);
    });
  };

  const currentScript = saved.find((x) => x.id === current) ?? null;

  const save = () => {
    const s = scripts.save({
      ...(currentScript ?? {}),
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

  const toggleRecording = () => {
    if (!recorder) return;
    if (recorder.recording) void recorder.stop();
    else {
      if (!host()) {
        push('error', t('script.noDocument'));
        return;
      }
      void recorder.start();
      push('status', t('script.recordStarted'));
    }
  };

  /** Shortcut / Quick Access Toolbar binding of the current saved script. */
  const bind = (patch: { shortcut?: string | null; quickAccess?: boolean }) => {
    if (!currentScript) return;
    const shortcut =
      patch.shortcut === undefined ? currentScript.shortcut : (patch.shortcut ?? undefined);
    if (patch.shortcut) {
      const conflict = checkShortcut?.(patch.shortcut, currentScript.id) ?? null;
      if (conflict) {
        setBindError(
          conflict === 'invalid'
            ? t('script.bind.invalid', { shortcut: patch.shortcut })
            : t('script.bind.conflict', { shortcut: patch.shortcut, command: conflict }),
        );
        return;
      }
    }
    setBindError(null);
    const rest: Partial<SavedScript> & Pick<SavedScript, 'name' | 'code'> = { ...currentScript };
    delete rest.shortcut;
    scripts.save({
      ...rest,
      ...(shortcut ? { shortcut } : {}),
      quickAccess: patch.quickAccess ?? !!currentScript.quickAccess,
    });
    setSaved(scripts.list());
  };

  const schedule = () => {
    if (!schedules || !currentScript) return;
    setScheduleError(null);
    schedules
      .create({ name: currentScript.name, code: currentScript.code, cron: cron.trim() })
      .then(
        () => {
          push('status', t('script.schedule.created', { name: currentScript.name }));
          reloadSchedules();
        },
        (e: unknown) =>
          setScheduleError(
            e instanceof Error && e.message === 'schedule.invalidCron'
              ? t('script.schedule.invalidCron')
              : t('script.schedule.failed'),
          ),
      );
  };

  const pick = (id: string) => {
    setVbaWarnings(null);
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
        {recorder && (
          <button
            type="button"
            onClick={toggleRecording}
            disabled={!!running}
            aria-pressed={recording}
            className={recording ? styles.recording : undefined}
            data-testid="script-record"
          >
            {recording ? t('script.recordStop') : t('script.record')}
          </button>
        )}
        <button
          type="button"
          onClick={() => setVbaOpen((o) => !o)}
          aria-expanded={vbaOpen}
          title={t('script.vba.hint')}
          data-testid="script-vba-open"
        >
          {t('script.vba.open')}
        </button>
      </div>
      {vbaOpen && (
        <div className={styles.vba} role="group" aria-label={t('script.vba.title')}>
          <div className={styles.vbaHint}>{t('script.vba.hint')}</div>
          <textarea
            className={styles.vbaSource}
            aria-label={t('script.vba.source')}
            placeholder={t('script.vba.placeholder')}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            wrap="off"
            value={vbaSource}
            onChange={(e) => setVbaSource(e.target.value)}
            data-testid="script-vba-source"
          />
          <div className={styles.row}>
            <button
              type="button"
              onClick={convertMacro}
              disabled={!vbaSource.trim()}
              data-testid="script-vba-convert"
            >
              {t('script.vba.convert')}
            </button>
            <button type="button" onClick={() => setVbaOpen(false)}>
              {t('script.vba.cancel')}
            </button>
          </div>
        </div>
      )}
      {vbaWarnings && (
        <div className={styles.vbaReport} data-testid="script-vba-report">
          <div className={styles.consoleHead}>
            <strong>
              {vbaWarnings.length
                ? t('script.vba.warningsTitle', { count: vbaWarnings.length })
                : t('script.vba.noWarnings')}
            </strong>
            <button type="button" onClick={() => setVbaWarnings(null)}>
              {t('script.vba.dismiss')}
            </button>
          </div>
          <div className={styles.vbaHint}>{t('script.vba.review')}</div>
          {vbaWarnings.length > 0 && (
            <ul className={styles.vbaWarnings} data-testid="script-vba-warnings">
              {vbaWarnings.map((w, i) => (
                <li key={`${w.line}-${i}`} title={w.source}>
                  {t('script.vba.warning', {
                    line: w.line,
                    construct: w.construct,
                    reason: t(`script.vba.reason.${w.reason}`),
                  })}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {recording && (
        <div className={styles.recordingNote} role="status" data-testid="script-recording">
          {t('script.recordingNote')}
        </div>
      )}
      {currentScript && (
        <div className={styles.row}>
          <input
            aria-label={t('script.bind.shortcut')}
            placeholder={t('script.bind.shortcutHint')}
            value={currentScript.shortcut ?? ''}
            readOnly
            onKeyDown={(e) => {
              if (e.key === 'Tab') return;
              e.preventDefault();
              e.stopPropagation();
              if ((e.key === 'Backspace' || e.key === 'Delete') && !e.ctrlKey && !e.altKey) {
                bind({ shortcut: null });
                return;
              }
              const s = shortcutFromEvent(e);
              if (s) bind({ shortcut: s });
            }}
            data-testid="script-shortcut"
          />
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={!!currentScript.quickAccess}
              onChange={(e) => bind({ quickAccess: e.target.checked })}
              data-testid="script-qat"
            />
            {t('script.bind.quickAccess')}
          </label>
        </div>
      )}
      {bindError && (
        <div className={styles.errorNote} role="alert" data-testid="script-bind-error">
          {bindError}
        </div>
      )}
      {schedules && currentScript && (
        <div className={styles.row}>
          <input
            aria-label={t('script.schedule.cron')}
            title={t('script.schedule.cronHint')}
            value={cron}
            onChange={(e) => setCron(e.target.value)}
            data-testid="script-cron"
          />
          <button type="button" onClick={schedule} data-testid="script-schedule">
            {t('script.schedule.add')}
          </button>
        </div>
      )}
      {scheduleError && (
        <div className={styles.errorNote} role="alert">
          {scheduleError}
        </div>
      )}
      {schedules && planned.length > 0 && (
        <ul className={styles.schedules} aria-label={t('script.schedule.list')}>
          {planned.map((p) => (
            <li key={p.id}>
              <span>
                {p.name} · <code>{p.cron}</code>
                {p.lastStatus ? ` · ${t(`script.schedule.status.${p.lastStatus}`)}` : ''}
              </span>
              <button
                type="button"
                onClick={() => void schedules.remove(p.id).then(reloadSchedules, reloadSchedules)}
              >
                {t('script.schedule.remove')}
              </button>
            </li>
          ))}
        </ul>
      )}
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
