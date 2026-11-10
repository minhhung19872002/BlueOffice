/**
 * Citations and bibliography of BlueDoc (CORE-CITE-001, References tab): the reference
 * style picker, "Insert citation" (search the lists, new source, placeholder, pages and
 * what to leave out), "Manage sources" (master list of the user ↔ list of the document,
 * BibTeX / RIS import, BibTeX export) and "Bibliography" (with a heading). Sources live in
 * the document's bibliography part (ECMA-376 Part 1, §22.6); citations are `CITATION`
 * fields (§17.16.5.8) and the list a `BIBLIOGRAPHY` field (§17.16.5.5). Each action is one
 * engine command (one undo step).
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  CITE_SOURCE_TYPES,
  CITE_STYLES,
  type CiteInfo,
  type CiteItem,
  type CiteSource,
  type CiteSourceInfo,
  type CiteSpan,
  type CiteStyleId,
  type EditCommand,
  type EditResult,
  type FileAccess,
} from '@blueoffice/platform';
import { Dialog, type SettingStore } from '@blueoffice/ui';
import { useCallback, useEffect, useState, type ReactNode } from 'react';
import styles from './CitationDialogs.module.css';
import {
  contributor,
  fieldsFor,
  lcidOf,
  loadMasterList,
  matchesSource,
  mergeSources,
  parsePeople,
  peopleText,
  placeholderTag,
  plainSource,
  saveMasterList,
  sourceLine,
  withContributor,
  withField,
} from './citations';
import editorStyles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Dialogs of this file (values of the editor's `dialog` state). */
export const CITATION_DIALOGS = ['insertCitation', 'manageSources', 'insertBibliography'] as const;
export type CitationDialogId = (typeof CITATION_DIALOGS)[number];

export function isCitationDialog(d: string | null): d is CitationDialogId {
  return (CITATION_DIALOGS as readonly string[]).includes(d ?? '');
}

/** Message key of an engine refusal. */
const ERRORS = [
  'cited',
  'noSource',
  'noCitation',
  'tagTaken',
  'inTable',
  'badFormat',
  'protected',
  'readOnly',
];
const errorKey = (e: string) => `cite.errors.${ERRORS.includes(e) ? e : 'failed'}`;

function Cancel({ onClick, label }: { onClick: () => void; label?: string }) {
  const { t } = useTranslation();
  return (
    <button type="button" className={editorStyles.dialogButton} onClick={onClick}>
      {label ?? t('cite.cancel')}
    </button>
  );
}

function Primary({
  testId,
  onClick,
  label,
  disabled,
}: {
  testId: string;
  onClick: () => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className={editorStyles.dialogPrimary}
      data-testid={testId}
      disabled={disabled}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

/** A formatted entry: italic stretches as `<em>`. */
export function EntryText({ spans }: { spans: CiteSpan[] }) {
  return (
    <>
      {spans.map((s, i) => (s.italic ? <em key={i}>{s.text}</em> : <span key={i}>{s.text}</span>))}
    </>
  );
}

// ---------------------------------------------------------------------------
// Style picker (ribbon)
// ---------------------------------------------------------------------------

/**
 * The reference style of the document. Choosing a style rewrites every citation and the
 * bibliography in one undo step. `refreshKey` changes when the picker should read the
 * style again (another document, a dialog closed).
 */
export function CitationStylePicker({
  run,
  disabled,
  refreshKey,
}: {
  run: Run;
  disabled?: boolean;
  refreshKey?: unknown;
}) {
  const { t } = useTranslation();
  const [style, setStyle] = useState<CiteStyleId>('APA');
  const refresh = useCallback(async () => {
    const r = await run({ cmd: 'citeState' });
    if (r?.cite) setStyle(r.cite.style);
  }, [run]);
  useEffect(() => {
    if (disabled) return;
    let alive = true;
    void run({ cmd: 'citeState' }).then((r) => {
      if (alive && r?.cite) setStyle(r.cite.style);
    });
    return () => {
      alive = false;
    };
  }, [run, disabled, refreshKey]);
  const choose = async (next: CiteStyleId) => {
    setStyle(next);
    const r = await run({ cmd: 'citeSetStyle', style: next });
    if (r?.cite) setStyle(r.cite.style);
  };
  return (
    <label className={styles.stylePicker}>
      <span>{t('cite.style')}</span>
      <select
        data-testid="cite-style"
        data-command="references.citationStyle"
        aria-label={t('cite.style')}
        value={style}
        disabled={disabled}
        onFocus={() => void refresh()}
        onChange={(e) => void choose(e.target.value as CiteStyleId)}
      >
        {CITE_STYLES.map((s) => (
          <option key={s} value={s}>
            {t(`cite.styles.${s}`)}
          </option>
        ))}
      </select>
    </label>
  );
}

// ---------------------------------------------------------------------------
// Source form
// ---------------------------------------------------------------------------

const isVietnamese = (s: CiteSource) => s.fields?.LCID === '1066';

/** The form of one source: type, language, contributors and the elements of its type. */
export function SourceForm({
  value,
  onChange,
}: {
  value: CiteSource;
  onChange: (s: CiteSource) => void;
}) {
  const { t } = useTranslation();
  const [all, setAll] = useState(false);
  const author = contributor(value, 'Author');
  const [corporate, setCorporate] = useState(!!author?.corporate);
  // Names are kept as typed while the form is open (parsing on every key would move the caret).
  const [names, setNames] = useState<Record<string, string>>(() => ({
    Author: peopleText(author?.people),
    Editor: peopleText(contributor(value, 'Editor')?.people),
    Translator: peopleText(contributor(value, 'Translator')?.people),
  }));
  const setPeople = (role: string, text: string, source: CiteSource = value) => {
    setNames((n) => ({ ...n, [role]: text }));
    onChange(withContributor(source, role, { people: parsePeople(text, isVietnamese(source)) }));
  };
  const roles = all || value.kind === 'BookSection' ? ['Editor'] : [];
  if (all) roles.push('Translator');
  const label = (name: string) => t(`cite.fields.${name}`, { defaultValue: name });
  return (
    <div className={styles.form} data-testid="cite-source-form">
      <label htmlFor="cite-kind">{t('cite.sourceType')}</label>
      <select
        id="cite-kind"
        data-testid="cite-kind"
        value={value.kind}
        onChange={(e) => onChange({ ...value, kind: e.target.value })}
      >
        {CITE_SOURCE_TYPES.map((k) => (
          <option key={k} value={k}>
            {t(`cite.types.${k}`)}
          </option>
        ))}
      </select>
      <label htmlFor="cite-language">{t('cite.language')}</label>
      <select
        id="cite-language"
        data-testid="cite-language"
        value={value.fields?.LCID ?? ''}
        onChange={(e) => {
          const next = withField(value, 'LCID', e.target.value);
          // The order of typed names follows the language.
          if (!corporate && names.Author?.trim()) {
            onChange(
              withContributor(next, 'Author', {
                people: parsePeople(names.Author, isVietnamese(next)),
              }),
            );
          } else onChange(next);
        }}
      >
        <option value="">{t('cite.languageDefault')}</option>
        <option value="1066">{t('cite.languageVi')}</option>
        <option value="1033">{t('cite.languageEn')}</option>
      </select>
      <label htmlFor="cite-author">{t('cite.author')}</label>
      {corporate ? (
        <input
          id="cite-author"
          data-testid="cite-corporate"
          value={author?.corporate ?? ''}
          onChange={(e) =>
            onChange(withContributor(value, 'Author', { corporate: e.target.value }))
          }
        />
      ) : (
        <textarea
          id="cite-author"
          data-testid="cite-author"
          rows={2}
          placeholder={t('cite.authorHint')}
          value={names.Author ?? ''}
          onChange={(e) => setPeople('Author', e.target.value)}
        />
      )}
      <label className={styles.wide}>
        <input
          type="checkbox"
          data-testid="cite-is-corporate"
          checked={corporate}
          onChange={(e) => {
            setCorporate(e.target.checked);
            if (e.target.checked) onChange(withContributor(value, 'Author', { corporate: '' }));
            else setPeople('Author', names.Author ?? '');
          }}
        />
        {t('cite.corporate')}
      </label>
      {roles.map((role) => (
        <FormRow key={role} id={`cite-role-${role}`} label={t(`cite.roles.${role}`)}>
          <textarea
            id={`cite-role-${role}`}
            data-testid={`cite-role-${role}`}
            rows={2}
            placeholder={t('cite.authorHint')}
            value={names[role] ?? ''}
            onChange={(e) => setPeople(role, e.target.value)}
          />
        </FormRow>
      ))}
      {fieldsFor(value, all).map((name) => (
        <FormRow key={name} id={`cite-field-${name}`} label={label(name)}>
          <input
            id={`cite-field-${name}`}
            data-testid={`cite-field-${name}`}
            value={value.fields?.[name] ?? ''}
            onChange={(e) => onChange(withField(value, name, e.target.value))}
          />
        </FormRow>
      ))}
      <FormRow id="cite-tag" label={t('cite.tag')}>
        <input
          id="cite-tag"
          data-testid="cite-tag"
          placeholder={t('cite.tagHint')}
          value={value.tag}
          onChange={(e) => onChange({ ...value, tag: e.target.value.replace(/[^\w.:-]/g, '') })}
        />
      </FormRow>
      <label className={styles.wide}>
        <input
          type="checkbox"
          data-testid="cite-all-fields"
          checked={all}
          onChange={(e) => setAll(e.target.checked)}
        />
        {t('cite.allFields')}
      </label>
    </div>
  );
}

function FormRow({ id, label, children }: { id: string; label: string; children: ReactNode }) {
  return (
    <>
      <label htmlFor={id}>{label}</label>
      {children}
    </>
  );
}

/** A source being made or changed: `replace` is the tag of the source it replaces. */
interface Editing {
  source: CiteSource;
  replace?: string;
  /** The list the source belongs to. */
  target: 'document' | 'master';
}

const emptySource = (language: string): CiteSource => ({
  tag: '',
  kind: 'Book',
  fields: language.toLowerCase().startsWith('vi') ? { LCID: '1066' } : {},
});

/** A source with a title or a contributor (worth saving). */
const hasData = (s: CiteSource) =>
  !!s.fields?.Title?.trim() ||
  (s.contributors ?? []).some((c) => !!c.corporate?.trim() || !!c.people?.length);

// ---------------------------------------------------------------------------
// Shared state: document sources + master list
// ---------------------------------------------------------------------------

function useSources(run: Run, settings: SettingStore) {
  const [info, setInfo] = useState<CiteInfo | null>(null);
  const [master, setMaster] = useState<CiteSource[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    void run({ cmd: 'citeState' }).then((r) => {
      if (alive && r?.cite) setInfo(r.cite);
    });
    void loadMasterList(settings).then((m) => {
      if (alive) setMaster(m);
    });
    return () => {
      alive = false;
    };
  }, [run, settings]);
  /** Run a command; keeps the new state and reports a refusal. */
  const send = useCallback(
    async (cmd: EditCommand): Promise<EditResult | null> => {
      const r = await run(cmd);
      if (r?.cite) setInfo(r.cite);
      setError(r?.error ? r.error : null);
      return r;
    },
    [run],
  );
  const storeMaster = useCallback(
    async (next: CiteSource[]) => {
      setMaster(next);
      await saveMasterList(settings, next);
    },
    [settings],
  );
  return { info, master, error, setError, send, storeMaster };
}

function SourceList({
  title,
  items,
  selected,
  onSelect,
  testId,
  empty,
}: {
  title?: string;
  items: (CiteSource & { cited?: boolean; placeholder?: boolean; badge?: string })[];
  selected: string[];
  onSelect: (tag: string) => void;
  testId: string;
  empty: string;
}) {
  const { t } = useTranslation();
  return (
    <div>
      {title && <h3>{title}</h3>}
      <ul className={styles.list} role="listbox" aria-label={title} data-testid={testId}>
        {items.length === 0 && <li className={styles.empty}>{empty}</li>}
        {items.map((s) => (
          <li
            key={s.tag}
            role="option"
            aria-selected={selected.includes(s.tag)}
            data-tag={s.tag}
            onClick={() => onSelect(s.tag)}
          >
            <span className={styles.line}>
              {s.placeholder ? t('cite.placeholderLine', { tag: s.tag }) : sourceLine(s)}
            </span>
            {s.badge && <span className={styles.badge}>{s.badge}</span>}
            {s.cited && <span className={styles.badge}>{t('cite.cited')}</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Insert / edit citation
// ---------------------------------------------------------------------------

/** Insert a citation at the caret, or change the citation the caret touches. */
export function InsertCitationDialog({
  run,
  settings,
  language,
  onClose,
}: {
  run: Run;
  settings: SettingStore;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const { info, master, error, send, storeMaster } = useSources(run, settings);
  const [query, setQuery] = useState('');
  // `null` until the user changes the choice: an existing citation's sources stand.
  const [chosen, setChosen] = useState<CiteItem[] | null>(null);
  const [placeholders, setPlaceholders] = useState<string[]>([]);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [toMaster, setToMaster] = useState(true);
  // Editing the citation at the caret: start from its sources.
  const [existing, setExisting] = useState<CiteInfo['citation']>();
  if (info?.citation && !existing) setExisting(info.citation);
  const items = chosen ?? existing?.items ?? [];
  const setItems = (f: (cur: CiteItem[]) => CiteItem[]) =>
    setChosen((cur) => f(cur ?? existing?.items ?? []));
  const docTags = (info?.sources ?? []).map((s) => s.tag.toLowerCase());
  const masterOnly = master.filter((m) => !docTags.includes(m.tag.toLowerCase()));
  const listed = [
    ...(info?.sources ?? []),
    ...placeholders.map((tag) => ({ tag, kind: 'Book', placeholder: true })),
    ...masterOnly.map((m) => ({ ...m, badge: t('cite.fromMaster') })),
  ].filter((s) => matchesSource(s, query));
  const toggle = (tag: string) =>
    setItems((cur) =>
      cur.some((i) => i.tag === tag) ? cur.filter((i) => i.tag !== tag) : [...cur, { tag }],
    );
  const patch = (tag: string, p: Partial<CiteItem>) =>
    setItems((cur) => cur.map((i) => (i.tag === tag ? { ...i, ...p } : i)));
  const name = (tag: string) => {
    const inDoc = info?.sources.find((x) => x.tag === tag);
    const s = inDoc ?? master.find((x) => x.tag === tag);
    return s && !inDoc?.placeholder ? sourceLine(s) : t('cite.placeholderLine', { tag });
  };
  const saveNew = async () => {
    if (!editing || !hasData(editing.source)) return;
    const before = (info?.sources ?? []).map((s) => s.tag);
    const r = await send({ cmd: 'citeSaveSources', sources: [editing.source] });
    if (!r || r.error) return;
    const added = r.cite?.sources.find((s) => !before.includes(s.tag));
    if (added) {
      setItems((cur) => [...cur, { tag: added.tag }]);
      if (toMaster) await storeMaster(mergeSources(master, [plainSource(added)]));
    }
    setEditing(null);
  };
  const addPlaceholder = () => {
    const tag = placeholderTag([
      ...(info?.sources ?? []).map((s) => s.tag),
      ...master.map((s) => s.tag),
      ...placeholders,
    ]);
    setPlaceholders((p) => [...p, tag]);
    setItems((cur) => [...cur, { tag }]);
  };
  const apply = async () => {
    if (!items.length) return;
    const chosen = items.map((i) => i.tag);
    // Sources taken from the master list and new placeholders join the document.
    const extra: CiteSource[] = [
      ...masterOnly.filter((m) => chosen.includes(m.tag)),
      ...placeholders.filter((p) => chosen.includes(p)).map((tag) => ({ tag, kind: 'Book' })),
    ];
    const citation = { items, lcid: existing?.lcid ?? lcidOf(language) };
    let r: EditResult | null;
    if (existing) {
      if (extra.length) await send({ cmd: 'citeSaveSources', sources: extra });
      r = await send({ cmd: 'citeEdit', citation });
    } else {
      r = await send({ cmd: 'citeInsert', citation, sources: extra });
    }
    if (r && !r.error) onClose();
  };
  if (editing) {
    return (
      <Dialog
        open
        onOpenChange={(v) => !v && onClose()}
        title={t('cite.newSource')}
        width={560}
        testId="cite-source-dialog"
        footer={
          <>
            <Cancel onClick={() => setEditing(null)} label={t('cite.back')} />
            <Primary
              testId="cite-source-save"
              label={t('cite.save')}
              disabled={!hasData(editing.source)}
              onClick={() => void saveNew()}
            />
          </>
        }
      >
        <div className={styles.column}>
          <SourceForm
            value={editing.source}
            onChange={(source) => setEditing({ ...editing, source })}
          />
          <label className={styles.toolbar}>
            <input
              type="checkbox"
              data-testid="cite-to-master"
              checked={toMaster}
              onChange={(e) => setToMaster(e.target.checked)}
            />
            {t('cite.alsoMaster')}
          </label>
          {error && (
            <div className={styles.error} role="alert">
              {t(errorKey(error))}
            </div>
          )}
        </div>
      </Dialog>
    );
  }
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={existing ? t('cite.editCitationTitle') : t('cite.insertCitationTitle')}
      width={600}
      testId="cite-insert-dialog"
      footer={
        <>
          <Cancel onClick={onClose} />
          <Primary
            testId="cite-insert-ok"
            label={existing ? t('cite.update') : t('cite.insert')}
            disabled={!items.length}
            onClick={() => void apply()}
          />
        </>
      }
    >
      <div className={styles.column}>
        <div className={styles.toolbar}>
          <input
            type="search"
            data-testid="cite-search"
            placeholder={t('cite.search')}
            aria-label={t('cite.search')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <button
            type="button"
            data-testid="cite-new-source"
            onClick={() => setEditing({ source: emptySource(language), target: 'document' })}
          >
            {t('cite.newSource')}
          </button>
          <button type="button" data-testid="cite-new-placeholder" onClick={addPlaceholder}>
            {t('cite.newPlaceholder')}
          </button>
        </div>
        <SourceList
          items={listed}
          selected={items.map((i) => i.tag)}
          onSelect={toggle}
          testId="cite-source-list"
          empty={t('cite.noSources')}
        />
        <div className={styles.items} data-testid="cite-items">
          {items.map((it) => (
            <div key={it.tag} className={styles.item} data-tag={it.tag}>
              <strong>{name(it.tag)}</strong>
              <label>
                {t('cite.pages')}
                <input
                  type="text"
                  data-testid="cite-item-pages"
                  value={it.pages ?? ''}
                  onChange={(e) => patch(it.tag, { pages: e.target.value })}
                />
              </label>
              <label>
                {t('cite.prefix')}
                <input
                  type="text"
                  data-testid="cite-item-prefix"
                  value={it.prefix ?? ''}
                  onChange={(e) => patch(it.tag, { prefix: e.target.value })}
                />
              </label>
              <label>
                {t('cite.suffix')}
                <input
                  type="text"
                  data-testid="cite-item-suffix"
                  value={it.suffix ?? ''}
                  onChange={(e) => patch(it.tag, { suffix: e.target.value })}
                />
              </label>
              {(['noAuthor', 'noYear', 'noTitle'] as const).map((k) => (
                <label key={k}>
                  <input
                    type="checkbox"
                    data-testid={`cite-item-${k}`}
                    checked={!!it[k]}
                    onChange={(e) => patch(it.tag, { [k]: e.target.checked })}
                  />
                  {t(`cite.${k}`)}
                </label>
              ))}
            </div>
          ))}
          {!items.length && <div className={styles.empty}>{t('cite.pickSource')}</div>}
        </div>
        {error && (
          <div className={styles.error} role="alert" data-testid="cite-error">
            {t(errorKey(error))}
          </div>
        )}
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Manage sources
// ---------------------------------------------------------------------------

const isInfo = (s: CiteSource): s is CiteSourceInfo => 'preview' in s;
const isPlaceholder = (s: CiteSource) => isInfo(s) && s.placeholder;

/** `ris` for a `.ris` file or text that starts with a RIS type tag, else `bibtex`. */
export function importFormat(name: string, text: string): 'bibtex' | 'ris' {
  if (/\.ris$/i.test(name)) return 'ris';
  if (/\.bib(tex)?$/i.test(name)) return 'bibtex';
  // A byte order mark may come first.
  const head = text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
  return /^\s*TY {2}-/.test(head) ? 'ris' : 'bibtex';
}

/** The master list of the user and the list of the document, side by side. */
export function SourceManagerDialog({
  run,
  settings,
  files,
  language,
  onClose,
}: {
  run: Run;
  settings: SettingStore;
  files: FileAccess;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const { info, master, error, setError, send, storeMaster } = useSources(run, settings);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<{ list: 'master' | 'document'; tag: string } | null>(
    null,
  );
  const [editing, setEditing] = useState<Editing | null>(null);
  const [masterInfo, setMasterInfo] = useState<CiteSourceInfo[]>([]);
  const [note, setNote] = useState<string | null>(null);
  const style = info?.style;
  // Entries of the master list in the document's style (for the preview).
  useEffect(() => {
    let alive = true;
    if (!master.length) return;
    void run({ cmd: 'citePreview', style, sources: master }).then((r) => {
      if (alive) setMasterInfo(r?.cite?.preview ?? []);
    });
    return () => {
      alive = false;
    };
  }, [run, master, style]);
  const docSources = info?.sources ?? [];
  const current: CiteSourceInfo | CiteSource | undefined =
    selected?.list === 'document'
      ? docSources.find((s) => s.tag === selected.tag)
      : selected
        ? (masterInfo.find((s) => s.tag === selected.tag) ??
          master.find((s) => s.tag === selected.tag))
        : undefined;
  const copyToDocument = async () => {
    const s = master.find((x) => x.tag === selected?.tag);
    if (s) await send({ cmd: 'citeSaveSources', sources: [s] });
  };
  const copyToMaster = async () => {
    const s = docSources.find((x) => x.tag === selected?.tag);
    if (s && !s.placeholder) await storeMaster(mergeSources(master, [plainSource(s)]));
  };
  const remove = async () => {
    if (!selected) return;
    if (selected.list === 'master') {
      await storeMaster(master.filter((s) => s.tag !== selected.tag));
      setSelected(null);
      return;
    }
    const r = await send({ cmd: 'citeDeleteSource', tag: selected.tag });
    if (r && !r.error) setSelected(null);
  };
  const edit = () => {
    if (!current || !selected) return;
    setError(null);
    setEditing({ source: plainSource(current), replace: current.tag, target: selected.list });
  };
  const save = async () => {
    if (!editing) return;
    const { source, replace, target } = editing;
    if (target === 'master') {
      const tag = source.tag || replace || placeholderTag(master.map((s) => s.tag));
      const rest = master.filter((s) => s.tag !== replace);
      await storeMaster(mergeSources(rest, [{ ...source, tag }]));
      setEditing(null);
      return;
    }
    const before = docSources.map((s) => s.tag);
    const r = await send({ cmd: 'citeSaveSources', sources: [source], replace });
    if (!r || r.error) return;
    // A new source is kept in the master list too.
    const added = r.cite?.sources.find((s) => !before.includes(s.tag));
    if (added && !replace) await storeMaster(mergeSources(master, [plainSource(added)]));
    setEditing(null);
  };
  const importFile = async () => {
    setNote(null);
    const file = await files.openFile(['.bib', '.ris', '.txt']);
    if (!file) return;
    const text = new TextDecoder('utf-8').decode(file.bytes);
    const r = await send({
      cmd: 'citeParse',
      format: importFormat(file.name, text),
      text,
      taken: master.map((s) => s.tag),
    });
    const parsed = r?.cite?.parsed ?? [];
    if (!parsed.length) {
      setNote(t('cite.importNone'));
      return;
    }
    await send({ cmd: 'citeSaveSources', sources: parsed });
    await storeMaster(mergeSources(master, parsed));
    setNote(t('cite.imported', { count: parsed.length }));
  };
  const exportFile = async () => {
    const r = await send({ cmd: 'citeExport' });
    const text = r?.cite?.exported;
    if (text) {
      await files.saveFile(new TextEncoder().encode(text), 'sources.bib', 'application/x-bibtex');
    }
  };
  if (editing) {
    return (
      <Dialog
        open
        onOpenChange={(v) => !v && onClose()}
        title={editing.replace ? t('cite.editSource') : t('cite.newSource')}
        width={560}
        testId="cite-source-dialog"
        footer={
          <>
            <Cancel onClick={() => setEditing(null)} label={t('cite.back')} />
            <Primary
              testId="cite-source-save"
              label={t('cite.save')}
              disabled={!hasData(editing.source)}
              onClick={() => void save()}
            />
          </>
        }
      >
        <div className={styles.column}>
          <SourceForm
            value={editing.source}
            onChange={(source) => setEditing({ ...editing, source })}
          />
          {error && (
            <div className={styles.error} role="alert" data-testid="cite-error">
              {t(errorKey(error))}
            </div>
          )}
        </div>
      </Dialog>
    );
  }
  const pick = (list: 'master' | 'document') => (tag: string) => {
    setError(null);
    setSelected({ list, tag });
  };
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('cite.manageTitle')}
      width={760}
      testId="cite-manager-dialog"
      footer={<Cancel onClick={onClose} label={t('cite.close')} />}
    >
      <div className={styles.column}>
        <div className={styles.toolbar}>
          <input
            type="search"
            data-testid="cite-search"
            placeholder={t('cite.search')}
            aria-label={t('cite.search')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <button
            type="button"
            data-testid="cite-new-source"
            onClick={() => {
              setError(null);
              setEditing({ source: emptySource(language), target: 'document' });
            }}
          >
            {t('cite.newSource')}
          </button>
          <button type="button" data-testid="cite-edit-source" disabled={!current} onClick={edit}>
            {t('cite.edit')}
          </button>
          <button
            type="button"
            data-testid="cite-delete-source"
            disabled={!selected}
            onClick={() => void remove()}
          >
            {t('cite.delete')}
          </button>
          <button type="button" data-testid="cite-import" onClick={() => void importFile()}>
            {t('cite.import')}
          </button>
          <button
            type="button"
            data-testid="cite-export"
            disabled={!docSources.some((s) => !s.placeholder)}
            onClick={() => void exportFile()}
          >
            {t('cite.export')}
          </button>
        </div>
        <div className={styles.lists}>
          <SourceList
            title={t('cite.masterList')}
            items={master.filter((s) => matchesSource(s, query))}
            selected={selected?.list === 'master' ? [selected.tag] : []}
            onSelect={pick('master')}
            testId="cite-master-list"
            empty={t('cite.noSources')}
          />
          <div className={styles.transfer}>
            <button
              type="button"
              data-testid="cite-copy-to-document"
              disabled={selected?.list !== 'master'}
              onClick={() => void copyToDocument()}
            >
              {t('cite.copyToDocument')}
            </button>
            <button
              type="button"
              data-testid="cite-copy-to-master"
              disabled={selected?.list !== 'document' || !current || isPlaceholder(current)}
              onClick={() => void copyToMaster()}
            >
              {t('cite.copyToMaster')}
            </button>
          </div>
          <SourceList
            title={t('cite.documentList')}
            items={docSources.filter((s) => matchesSource(s, query))}
            selected={selected?.list === 'document' ? [selected.tag] : []}
            onSelect={pick('document')}
            testId="cite-document-list"
            empty={t('cite.noSources')}
          />
        </div>
        <div className={styles.preview} data-testid="cite-preview" aria-live="polite">
          {current && isInfo(current) && (
            <>
              <div className={styles.cite}>{current.citation}</div>
              <div>
                <EntryText spans={current.preview} />
              </div>
            </>
          )}
        </div>
        {note && <div data-testid="cite-note">{note}</div>}
        {error && (
          <div className={styles.error} role="alert" data-testid="cite-error">
            {t(errorKey(error))}
          </div>
        )}
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Bibliography
// ---------------------------------------------------------------------------

/** Heading presets of a new bibliography. */
export const BIBLIOGRAPHY_HEADINGS = ['bibliography', 'references', 'worksCited', 'none'] as const;
export type BibliographyHeading = (typeof BIBLIOGRAPHY_HEADINGS)[number] | 'custom';

/** Insert the bibliography at the caret, with a heading. */
export function BibliographyDialog({
  run,
  language,
  onClose,
}: {
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [preset, setPreset] = useState<BibliographyHeading>('references');
  const [custom, setCustom] = useState('');
  const [error, setError] = useState<string | null>(null);
  const heading =
    preset === 'none' ? '' : preset === 'custom' ? custom.trim() : t(`cite.headings.${preset}`);
  const apply = async () => {
    const r = await run({
      cmd: 'citeBibliography',
      heading: heading || undefined,
      lcid: lcidOf(language),
    });
    if (r?.error) setError(r.error);
    else onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('cite.bibliographyTitle')}
      width={420}
      testId="cite-bibliography-dialog"
      footer={
        <>
          <Cancel onClick={onClose} />
          <Primary
            testId="cite-bibliography-ok"
            label={t('cite.insert')}
            onClick={() => void apply()}
          />
        </>
      }
    >
      <div className={styles.form}>
        <label htmlFor="cite-heading">{t('cite.heading')}</label>
        <select
          id="cite-heading"
          data-testid="cite-heading"
          value={preset}
          onChange={(e) => setPreset(e.target.value as BibliographyHeading)}
        >
          {BIBLIOGRAPHY_HEADINGS.map((h) => (
            <option key={h} value={h}>
              {h === 'none' ? t('cite.headingNone') : t(`cite.headings.${h}`)}
            </option>
          ))}
          <option value="custom">{t('cite.headingCustom')}</option>
        </select>
        {preset === 'custom' && (
          <FormRow id="cite-heading-text" label={t('cite.headingText')}>
            <input
              id="cite-heading-text"
              data-testid="cite-heading-text"
              value={custom}
              onChange={(e) => setCustom(e.target.value)}
            />
          </FormRow>
        )}
        {error && (
          <div className={`${styles.wide} ${styles.error}`} role="alert" data-testid="cite-error">
            {t(errorKey(error))}
          </div>
        )}
      </div>
    </Dialog>
  );
}

/** The dialog `which` of this file. */
export function CitationDialog({
  which,
  run,
  settings,
  files,
  language,
  onClose,
}: {
  which: CitationDialogId;
  run: Run;
  settings: SettingStore;
  files: FileAccess;
  language: string;
  onClose: () => void;
}) {
  switch (which) {
    case 'insertCitation':
      return (
        <InsertCitationDialog run={run} settings={settings} language={language} onClose={onClose} />
      );
    case 'manageSources':
      return (
        <SourceManagerDialog
          run={run}
          settings={settings}
          files={files}
          language={language}
          onClose={onClose}
        />
      );
    case 'insertBibliography':
      return <BibliographyDialog run={run} language={language} onClose={onClose} />;
  }
}
