/**
 * Read-only viewer of the VBA macro source of the open file (CORE-VBA-002), shared
 * by BlueDoc, BlueSheet and BlueSlide.
 *
 * BlueOffice never runs VBA. The viewer lists the modules of the project (read by
 * the engine, `vbaProject` command) and shows the code of one module in a read-only
 * box. "Convert to BlueScript draft" hands the module's text to the converter of
 * the BlueScript pane; the draft opens in the editor for review and is never run
 * automatically.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import type { VbaModuleInfo, VbaProjectInfo } from '@blueoffice/platform';
import type { VbaDraftRequest } from '@blueoffice/script';
import { Dialog } from '@blueoffice/ui';
import { ShieldCheck } from 'lucide-react';
import editorStyles from './Editor.module.css';
import styles from './MacroViewer.module.css';

export interface MacroViewerProps {
  /** Reads the project of the open file (`{ cmd: 'vbaProject' }`). */
  load: () => Promise<VbaProjectInfo>;
  /** The user asked for a BlueScript draft of this module. */
  onConvert: (module: { name: string; source: string }) => void;
  onClose: () => void;
}

/**
 * State of the viewer in an editor: whether it is open, and the module the user
 * sent to the BlueScript pane (`ScriptPane.vbaDraft`). `openScriptPane` shows the
 * pane; the draft is converted there and never run automatically.
 */
export function useMacroViewer(openScriptPane: () => void) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<VbaDraftRequest | null>(null);
  const seq = useRef(0);
  const show = useCallback(() => setOpen(true), []);
  const close = useCallback(() => setOpen(false), []);
  const convert = useCallback(
    (m: { name: string; source: string }) => {
      seq.current += 1;
      setDraft({ id: seq.current, source: m.source, module: m.name });
      setOpen(false);
      openScriptPane();
    },
    [openScriptPane],
  );
  const draftHandled = useCallback(() => setDraft(null), []);
  return { open, show, close, convert, draft, draftHandled };
}

/** The module shown first: the first standard module with code, else the first with code. */
export function initialModule(modules: readonly VbaModuleInfo[]): number {
  const withCode = (m: VbaModuleInfo) => !m.unreadable && m.source.trim() !== '';
  const standard = modules.findIndex((m) => m.kind === 'standard' && withCode(m));
  if (standard >= 0) return standard;
  return Math.max(0, modules.findIndex(withCode));
}

/** Number of lines of a module's source (a final line break does not start a line). */
export function lineCount(source: string): number {
  if (!source) return 0;
  const n = source.split('\n').length;
  return source.endsWith('\n') ? n - 1 : n;
}

export function MacroViewer({ load, onConvert, onClose }: MacroViewerProps) {
  const { t } = useTranslation();
  const [project, setProject] = useState<VbaProjectInfo | null>(null);
  const [failed, setFailed] = useState(false);
  const [selected, setSelected] = useState(0);

  useEffect(() => {
    let alive = true;
    load().then(
      (p) => {
        if (!alive) return;
        setProject(p);
        setSelected(initialModule(p.modules));
      },
      () => alive && setFailed(true),
    );
    return () => {
      alive = false;
    };
  }, [load]);

  const modules = project?.modules ?? [];
  const current = modules[selected];
  const convertible = !!current && !current.unreadable && current.source.trim() !== '';
  const broken = failed || !!project?.error;

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('macroViewer.title')}
      testId="macro-viewer"
      width={860}
      footer={
        <>
          <span className={styles.footNote}>{t('macroViewer.convertHint')}</span>
          <button
            type="button"
            className={editorStyles.dialogPrimary}
            disabled={!convertible}
            onClick={() => current && onConvert({ name: current.name, source: current.source })}
            data-testid="macro-viewer-convert"
          >
            {t('macroViewer.convert')}
          </button>
          <button type="button" className={editorStyles.dialogButton} onClick={onClose}>
            {t('macroViewer.close')}
          </button>
        </>
      }
    >
      <div className={styles.notice} role="note" data-testid="macro-viewer-notice">
        <ShieldCheck size={18} aria-hidden />
        <span>{t('macroViewer.notice')}</span>
      </div>
      {!project && !failed && (
        <div className={styles.message} role="status">
          {t('macroViewer.loading')}
        </div>
      )}
      {broken && (
        <div className={styles.message} role="alert" data-testid="macro-viewer-error">
          {t('macroViewer.failed')}
        </div>
      )}
      {project && !broken && modules.length === 0 && (
        <div className={styles.message} role="status" data-testid="macro-viewer-empty">
          {t('macroViewer.empty')}
        </div>
      )}
      {project && !project.codePageSupported && modules.length > 0 && (
        <div className={styles.warning} role="status" data-testid="macro-viewer-codepage">
          {t('macroViewer.codePage', { codePage: project.codePage })}
        </div>
      )}
      {project?.truncated && (
        <div className={styles.warning} role="status">
          {t('macroViewer.truncated')}
        </div>
      )}
      {modules.length > 0 && (
        <div className={styles.split}>
          <ul
            className={styles.modules}
            role="listbox"
            aria-label={t('macroViewer.modules')}
            data-testid="macro-viewer-modules"
          >
            {modules.map((m, i) => (
              <li
                key={`${m.name}-${i}`}
                role="option"
                aria-selected={i === selected}
                tabIndex={i === selected ? 0 : -1}
                className={i === selected ? styles.activeModule : undefined}
                onClick={() => setSelected(i)}
                onKeyDown={(e) => {
                  const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
                  if (!step) return;
                  e.preventDefault();
                  const next = Math.min(modules.length - 1, Math.max(0, i + step));
                  setSelected(next);
                  const items = e.currentTarget.parentElement?.children;
                  (items?.[next] as HTMLElement | undefined)?.focus();
                }}
              >
                <span className={styles.moduleName}>{m.name}</span>
                <small>
                  {t(`macroViewer.kind.${m.kind}`)}
                  {!m.unreadable && ` · ${t('macroViewer.lines', { count: lineCount(m.source) })}`}
                </small>
              </li>
            ))}
          </ul>
          {current?.unreadable ? (
            <div className={styles.message} role="alert" data-testid="macro-viewer-unreadable">
              {t('macroViewer.unreadable')}
            </div>
          ) : (
            <textarea
              className={styles.code}
              readOnly
              spellCheck={false}
              wrap="off"
              aria-label={t('macroViewer.code', { name: current?.name ?? '' })}
              placeholder={t('macroViewer.emptyModule')}
              value={current?.source ?? ''}
              data-testid="macro-viewer-code"
            />
          )}
        </div>
      )}
    </Dialog>
  );
}
