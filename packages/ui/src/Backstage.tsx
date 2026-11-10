/**
 * Full-window File screen shared by BlueDoc, BlueSheet and BlueSlide (CORE-UI-002):
 * a navigation column on the left (Back, the pages, actions such as Save or Close)
 * and the selected page on the right. Escape or Back closes it; the arrow keys,
 * Home and End move through the navigation. The building blocks below
 * (`BackstageCommands`, `BackstageProperties`, `BackstageFiles`,
 * `BackstageFormats`) lay out the pages; every string comes from the caller.
 */
import { ArrowLeft, Pin, PinOff } from 'lucide-react';
import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react';
import styles from './Backstage.module.css';

/** One entry of the navigation column. */
export interface BackstageSection {
  id: string;
  label: string;
  icon?: ReactNode;
  /** Page content. Without it the entry is an action: `onSelect` runs and the screen closes. */
  content?: ReactNode;
  onSelect?: () => void;
  enabled?: boolean;
  /** Shown after a separator, at the bottom of the column (Options, Account…). */
  bottom?: boolean;
  /** Command id for tests / KeyTips (`file.save`). */
  command?: string;
}

export interface BackstageLabels {
  /** Accessible name of the screen ("File"). */
  title: string;
  /** Back button. */
  back: string;
}

export interface BackstageProps {
  open: boolean;
  onClose: () => void;
  sections: BackstageSection[];
  /** Selected page (controlled); falls back to the first page. */
  page?: string;
  onPageChange?: (id: string) => void;
  labels: BackstageLabels;
  /** Document name shown above the page. */
  heading?: string;
}

/** The page shown: `page` when it is a page, else the first page. */
export function backstagePage(
  sections: readonly BackstageSection[],
  page: string | undefined,
): BackstageSection | undefined {
  const pages = sections.filter((s) => s.content !== undefined && s.enabled !== false);
  return pages.find((s) => s.id === page) ?? pages[0];
}

/** Index of the navigation entry the key moves to (`null`: not a navigation key). */
export function navTarget(key: string, index: number, count: number): number | null {
  if (count === 0) return null;
  switch (key) {
    case 'ArrowDown':
      return (index + 1) % count;
    case 'ArrowUp':
      return (index - 1 + count) % count;
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return null;
  }
}

/** The File screen. */
export function Backstage({
  open,
  onClose,
  sections,
  page,
  onPageChange,
  labels,
  heading,
}: BackstageProps) {
  const [own, setOwn] = useState<string | undefined>(page);
  const current = backstagePage(sections, page ?? own);
  const nav = useRef<HTMLElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const titleId = useId();
  // Focus moves into the screen on open and back where it was on close.
  useEffect(() => {
    if (!open) return undefined;
    const before = document.activeElement;
    const first = nav.current?.querySelector<HTMLButtonElement>('button[aria-current="page"]');
    (first ?? nav.current?.querySelector<HTMLButtonElement>('button'))?.focus();
    return () => {
      if (before instanceof HTMLElement && before.isConnected) before.focus();
    };
  }, [open]);
  // Escape with the focus nowhere in particular (the page background).
  useEffect(() => {
    if (!open) return undefined;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || e.target !== document.body) return;
      e.preventDefault();
      onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);
  // Escape inside the screen: handled here, before the editor's own shortcuts see it.
  const onRootKey = (e: ReactKeyboardEvent) => {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    onClose();
  };
  if (!open) return null;
  const select = (s: BackstageSection) => {
    if (s.enabled === false) return;
    if (s.content === undefined) {
      onClose();
      s.onSelect?.();
      return;
    }
    setOwn(s.id);
    onPageChange?.(s.id);
  };
  const onNavKey = (e: ReactKeyboardEvent) => {
    const buttons = Array.from(
      nav.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [],
    );
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = navTarget(e.key, i, buttons.length);
    if (next === null) return;
    e.preventDefault();
    buttons[next]?.focus();
  };
  const entry = (s: BackstageSection) => (
    <li key={s.id}>
      <button
        type="button"
        className={styles.navItem}
        data-section={s.id}
        data-command={s.command}
        aria-current={current?.id === s.id ? 'page' : undefined}
        disabled={s.enabled === false}
        onClick={() => select(s)}
      >
        {s.icon && (
          <span className={styles.navIcon} aria-hidden="true">
            {s.icon}
          </span>
        )}
        {s.label}
      </button>
    </li>
  );
  const top = sections.filter((s) => !s.bottom);
  const bottom = sections.filter((s) => s.bottom);
  return (
    <div
      ref={root}
      className={styles.backstage}
      role="dialog"
      aria-modal="true"
      aria-label={labels.title}
      data-testid="backstage"
      data-page={current?.id}
      onKeyDown={onRootKey}
    >
      <nav ref={nav} className={styles.nav} aria-label={labels.title} onKeyDown={onNavKey}>
        <button
          type="button"
          className={styles.back}
          data-testid="backstage-back"
          aria-label={labels.back}
          title={`${labels.back} (Esc)`}
          onClick={onClose}
        >
          <ArrowLeft size={18} aria-hidden="true" />
        </button>
        <ul className={styles.navList}>{top.map(entry)}</ul>
        {bottom.length > 0 && (
          <ul className={`${styles.navList} ${styles.navBottom}`}>{bottom.map(entry)}</ul>
        )}
      </nav>
      <section className={styles.page} aria-labelledby={titleId} data-testid="backstage-page">
        {heading && <div className={styles.heading}>{heading}</div>}
        <h1 id={titleId} className={styles.pageTitle}>
          {current?.label}
        </h1>
        {current?.content}
      </section>
    </div>
  );
}

/** A command shown as a large button with an optional description. */
export interface BackstageCommand {
  id: string;
  label: string;
  description?: string;
  icon?: ReactNode;
  enabled?: boolean;
  onSelect: () => void;
}

/** A titled list of commands (New, Export, Print, Share …). */
export function BackstageCommands({
  title,
  commands,
  empty,
}: {
  title?: string;
  commands: BackstageCommand[];
  /** Shown when there is no command. */
  empty?: string;
}) {
  return (
    <div className={styles.block}>
      {title && <h2 className={styles.blockTitle}>{title}</h2>}
      {commands.length === 0 && empty && <p className={styles.muted}>{empty}</p>}
      <ul className={styles.commands}>
        {commands.map((c) => (
          <li key={c.id}>
            <button
              type="button"
              className={styles.command}
              data-command={c.id}
              disabled={c.enabled === false}
              onClick={c.onSelect}
            >
              {c.icon && (
                <span className={styles.commandIcon} aria-hidden="true">
                  {c.icon}
                </span>
              )}
              <span className={styles.commandText}>
                <span className={styles.commandLabel}>{c.label}</span>
                {c.description && <span className={styles.muted}>{c.description}</span>}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** One field of the properties form. */
export interface BackstageField {
  id: string;
  label: string;
  value: string;
  /** Shown as text (dates, revision, last saved by). */
  readOnly?: boolean;
  multiline?: boolean;
}

/**
 * Document properties: a text box per editable field, committed on blur / Enter
 * (`onChange` with the field and its new value), read-only values as text.
 */
export function BackstageProperties({
  title,
  fields,
  onChange,
  disabled,
}: {
  title: string;
  fields: BackstageField[];
  onChange: (id: string, value: string) => void;
  disabled?: boolean;
}) {
  return (
    <div className={styles.block}>
      <h2 className={styles.blockTitle}>{title}</h2>
      <dl className={styles.props}>
        {fields.map((f) => (
          <PropertyRow key={f.id} field={f} onChange={onChange} disabled={disabled} />
        ))}
      </dl>
    </div>
  );
}

function PropertyRow({
  field,
  onChange,
  disabled,
}: {
  field: BackstageField;
  onChange: (id: string, value: string) => void;
  disabled?: boolean;
}) {
  const [draft, setDraft] = useState(field.value);
  // A new value from the engine replaces the draft.
  const [shown, setShown] = useState(field.value);
  if (shown !== field.value) {
    setShown(field.value);
    setDraft(field.value);
  }
  const id = useId();
  const commit = () => {
    if (draft !== field.value) onChange(field.id, draft);
  };
  if (field.readOnly) {
    return (
      <>
        <dt className={styles.propLabel}>{field.label}</dt>
        <dd className={styles.propValue} data-testid={`prop-${field.id}`}>
          {field.value || '—'}
        </dd>
      </>
    );
  }
  const common = {
    id,
    className: styles.propInput,
    value: draft,
    disabled,
    'data-testid': `prop-${field.id}`,
    onChange: (e: { target: { value: string } }) => setDraft(e.target.value),
    onBlur: commit,
  };
  return (
    <>
      <dt className={styles.propLabel}>
        <label htmlFor={id}>{field.label}</label>
      </dt>
      <dd className={styles.propValue}>
        {field.multiline ? (
          <textarea {...common} rows={2} />
        ) : (
          <input
            {...common}
            type="text"
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                commit();
              }
            }}
          />
        )}
      </dd>
    </>
  );
}

/** A file of the Open page. */
export interface BackstageFile {
  /** Key (path). */
  id: string;
  name: string;
  /** Second line (folder, date). */
  detail?: string;
  pinned: boolean;
}

/** Pinned and recent files with a pin toggle each. */
export function BackstageFiles({
  title,
  files,
  empty,
  onOpen,
  onTogglePin,
  pinLabel,
  unpinLabel,
  canOpen = true,
}: {
  title: string;
  files: BackstageFile[];
  empty: string;
  onOpen: (f: BackstageFile) => void;
  onTogglePin: (f: BackstageFile) => void;
  pinLabel: string;
  unpinLabel: string;
  /** Files can be opened from their path (desktop). */
  canOpen?: boolean;
}) {
  return (
    <div className={styles.block}>
      <h2 className={styles.blockTitle}>{title}</h2>
      {files.length === 0 ? (
        <p className={styles.muted}>{empty}</p>
      ) : (
        <ul className={styles.files} aria-label={title}>
          {files.map((f) => (
            <li key={f.id} className={styles.file} data-pinned={f.pinned ? 'true' : undefined}>
              <button
                type="button"
                className={styles.fileOpen}
                disabled={!canOpen}
                onClick={() => onOpen(f)}
                title={f.id}
              >
                <span className={styles.commandLabel}>{f.name}</span>
                {f.detail && <span className={styles.muted}>{f.detail}</span>}
              </button>
              <button
                type="button"
                className={styles.pin}
                aria-pressed={f.pinned}
                aria-label={`${f.pinned ? unpinLabel : pinLabel}: ${f.name}`}
                title={f.pinned ? unpinLabel : pinLabel}
                onClick={() => onTogglePin(f)}
              >
                {f.pinned ? <PinOff size={16} /> : <Pin size={16} />}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** A file format of the Save As page. */
export interface BackstageFormat {
  id: string;
  label: string;
  /** Extension shown beside the label (`.docx`). */
  ext: string;
  /** Current type of the file. */
  current?: boolean;
  /** Warning (macros dropped …). */
  note?: string;
  onSelect: () => void;
  enabled?: boolean;
}

/** Every output format of the app, grouped (document types, other formats). */
export function BackstageFormats({
  groups,
}: {
  groups: { title: string; formats: BackstageFormat[] }[];
}) {
  return (
    <>
      {groups
        .filter((g) => g.formats.length > 0)
        .map((g) => (
          <div key={g.title} className={styles.block}>
            <h2 className={styles.blockTitle}>{g.title}</h2>
            <ul className={styles.commands} aria-label={g.title}>
              {g.formats.map((f) => (
                <li key={f.id}>
                  <button
                    type="button"
                    className={styles.command}
                    data-format={f.ext}
                    data-current={f.current ? 'true' : undefined}
                    disabled={f.enabled === false}
                    onClick={f.onSelect}
                  >
                    <span className={styles.ext} aria-hidden="true">
                      {f.ext}
                    </span>
                    <span className={styles.commandText}>
                      <span className={styles.commandLabel}>{f.label}</span>
                      {f.note && <span className={styles.muted}>{f.note}</span>}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ))}
    </>
  );
}
