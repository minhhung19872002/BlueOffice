/**
 * Building blocks (DOC-BB-001): gallery dropdowns of the Insert tab (building blocks,
 * headers, footers, page numbers, cover pages) with previews drawn by the engine, the
 * "save selection as building block" dialog and the Organizer. The blocks live in the
 * document's glossary part (ECMA-376 Part 1, §17.12), in the user's building blocks
 * file kept through the platform, and in the library shipped with BlueDoc.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  BLOCK_GALLERIES,
  type BlockBehavior,
  type BlockGalleryGroup,
  type BlockInfo,
  type BlockSpec,
  type BlockStore,
  blocksOfGroup,
  type EditCommand,
  type EditResult,
} from '@blueoffice/platform';
import { Dialog, Popover } from '@blueoffice/ui';
import { ChevronDown } from 'lucide-react';
import { type ReactNode, useCallback, useEffect, useState } from 'react';
import own from './BuildingBlocks.module.css';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

const BEHAVIORS: BlockBehavior[] = ['content', 'p', 'pg'];

/** Errors of the building block commands that have a message of their own. */
const BLOCK_ERRORS = [
  'noBlock',
  'badName',
  'badGallery',
  'emptySelection',
  'builtinBlock',
  'noAutoText',
  'bodyOnly',
  'badFile',
] as const;

/** i18n key of a building block command error (null: not one of them). */
export function blockErrorKey(error: string | null | undefined): string | null {
  return error && (BLOCK_ERRORS as readonly string[]).includes(error)
    ? `blocks.errors.${error}`
    : null;
}

/** i18n key of a gallery name (custom galleries of other applications share one label). */
export function galleryLabelKey(gallery: string): string {
  const known: readonly string[] = [...BLOCK_GALLERIES, 'pgNumT', 'pgNumB', 'pgNumMargins'];
  return known.includes(gallery) ? `blocks.galleries.${gallery}` : 'blocks.galleries.other';
}

/** Name suggested for a new block: the first words of the selection. */
export function suggestedBlockName(text: string): string {
  const line = text.trim().split(/[\r\n]/)[0] ?? '';
  // A block name holds no comma (§17.12.13 name).
  const words = line.replace(/,/g, ' ').split(/\s+/).filter(Boolean);
  return words.slice(0, 4).join(' ').slice(0, 32);
}

/** Preview pictures by block (key and content summary), kept while the editor lives. */
const previewCache = new Map<string, string>();
const cacheKey = (b: BlockInfo) => `${b.key}|${b.name}|${b.text}`;

/** Drop the cached previews (after blocks were changed or another document opened). */
export function clearBlockPreviews(): void {
  previewCache.clear();
}

/** Picture of a block drawn by the engine (`buildingBlockPreview`); its text until it arrives. */
export function BlockPreview({ block, query }: { block: BlockInfo; query: Run }) {
  const id = cacheKey(block);
  const blockKey = block.key;
  // The picture fetched for this block; the cache answers at once when it has it.
  const [loaded, setLoaded] = useState<{ id: string; png: string } | null>(null);
  useEffect(() => {
    if (previewCache.has(id)) return;
    let live = true;
    void query({ cmd: 'buildingBlockPreview', key: blockKey }).then((r) => {
      const p = r?.blocks?.preview;
      if (!p) return;
      previewCache.set(id, p);
      if (live) setLoaded({ id, png: p });
    });
    return () => {
      live = false;
    };
  }, [id, blockKey, query]);
  const png = previewCache.get(id) ?? (loaded?.id === id ? loaded.png : null);
  return png ? (
    <img
      className={own.preview}
      data-testid="block-preview"
      alt=""
      src={`data:image/png;base64,${png}`}
    />
  ) : (
    <span className={own.previewText}>{block.text || block.name}</span>
  );
}

/** The blocks of the engine (`buildingBlocks`); `null` while loading. */
export function useBlocks(query: Run): [BlockInfo[] | null, () => void] {
  const [list, setList] = useState<BlockInfo[] | null>(null);
  const reload = useCallback(() => {
    void query({ cmd: 'buildingBlocks' }).then((r) => setList(r?.blocks?.list ?? []));
  }, [query]);
  useEffect(reload, [reload]);
  return [list, reload];
}

/** Blocks of one ribbon gallery grouped by category; a click inserts the block. */
export function BlockGallery({
  group,
  query,
  onPick,
}: {
  group: BlockGalleryGroup;
  query: Run;
  onPick: (block: BlockInfo) => void;
}) {
  const { t } = useTranslation();
  const [list] = useBlocks(query);
  if (!list) return <div className={styles.hint}>…</div>;
  const blocks = blocksOfGroup(list, group);
  if (blocks.length === 0)
    return (
      <div className={styles.hint} data-testid="blocks-empty">
        {t('blocks.menu.empty')}
      </div>
    );
  const categories = [...new Set(blocks.map((b) => b.category))];
  return (
    <div className={own.gallery} data-testid={`block-gallery-${group}`}>
      {categories.map((c) => (
        <section key={c}>
          <h4 className={own.category}>{c}</h4>
          {blocks
            .filter((b) => b.category === c)
            .map((b) => (
              <button
                key={b.key}
                type="button"
                role="menuitem"
                className={own.item}
                data-block={b.name}
                data-store={b.store}
                title={b.description || b.name}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onPick(b)}
              >
                <BlockPreview block={b} query={query} />
                <span className={own.itemName}>{b.name}</span>
              </button>
            ))}
        </section>
      ))}
    </div>
  );
}

export interface BlockMenuAction {
  /** Command id (`data-command`). */
  id: string;
  label: string;
  disabled?: boolean;
  onSelect: () => void;
}

/**
 * Gallery dropdown of the ribbon: the blocks of `groups` (each under its heading when
 * there are several), then `actions` (edit header, save selection, organizer …).
 */
export function BlockGalleryMenu({
  groups,
  label,
  icon,
  testId,
  disabled,
  run,
  query,
  page,
  actions,
}: {
  groups: BlockGalleryGroup[];
  label: string;
  icon: ReactNode;
  testId: string;
  disabled: boolean;
  run: Run;
  query: Run;
  /** Page whose header / footer a header, footer or page number block replaces. */
  page: number;
  actions: BlockMenuAction[];
}) {
  const { t } = useTranslation();
  return (
    <Popover
      label={label}
      disabled={disabled}
      testId={testId}
      trigger={
        <>
          {icon}
          <span className={own.triggerLabel}>{label}</span>
          <ChevronDown size={12} />
        </>
      }
    >
      {(close) => (
        <div className={`${styles.menu} ${own.menu}`} role="menu">
          {groups.map((g) => (
            <div key={g}>
              {groups.length > 1 && <h3 className={own.group}>{t(`blocks.groups.${g}`)}</h3>}
              <BlockGallery
                group={g}
                query={query}
                onPick={(b) => {
                  close();
                  void run({ cmd: 'insertBuildingBlock', key: b.key, page });
                }}
              />
            </div>
          ))}
          <div className={own.actions}>
            {actions.map((a) => (
              <button
                key={a.id}
                type="button"
                role="menuitem"
                data-command={a.id}
                disabled={a.disabled}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => {
                  close();
                  a.onSelect();
                }}
              >
                {a.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </Popover>
  );
}

/** Fields of a block's properties (save dialog, Organizer). */
function SpecFields({
  spec,
  onChange,
  categories,
}: {
  spec: BlockSpec;
  onChange: (s: BlockSpec) => void;
  categories: string[];
}) {
  const { t } = useTranslation();
  const galleries: string[] = [...BLOCK_GALLERIES];
  if (!galleries.includes(spec.gallery)) galleries.push(spec.gallery);
  return (
    <>
      <label className={styles.field}>
        <span>{t('blocks.fields.name')}</span>
        <input
          data-testid="block-name"
          value={spec.name}
          maxLength={255}
          autoFocus
          onChange={(e) => onChange({ ...spec, name: e.target.value })}
        />
      </label>
      <label className={styles.field}>
        <span>{t('blocks.fields.gallery')}</span>
        <select
          data-testid="block-gallery"
          value={spec.gallery}
          onChange={(e) => onChange({ ...spec, gallery: e.target.value })}
        >
          {galleries.map((g) => (
            <option key={g} value={g}>
              {t(galleryLabelKey(g))}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.field}>
        <span>{t('blocks.fields.category')}</span>
        <input
          data-testid="block-category"
          list="bo-block-categories"
          value={spec.category}
          onChange={(e) => onChange({ ...spec, category: e.target.value })}
        />
        <datalist id="bo-block-categories">
          {categories.map((c) => (
            <option key={c} value={c} />
          ))}
        </datalist>
      </label>
      <label className={styles.field}>
        <span>{t('blocks.fields.description')}</span>
        <input
          data-testid="block-description"
          value={spec.description}
          onChange={(e) => onChange({ ...spec, description: e.target.value })}
        />
      </label>
      <label className={styles.field}>
        <span>{t('blocks.fields.behavior')}</span>
        <select
          data-testid="block-behavior"
          value={spec.behavior}
          onChange={(e) => onChange({ ...spec, behavior: e.target.value as BlockBehavior })}
        >
          {BEHAVIORS.map((b) => (
            <option key={b} value={b}>
              {t(`blocks.behaviors.${b}`)}
            </option>
          ))}
        </select>
      </label>
    </>
  );
}

const validName = (name: string) => name.trim() !== '' && !name.includes(',');

/** "Save selection as building block": name, gallery, category, description, behaviour, where. */
export function SaveBlockDialog({
  run,
  query,
  onClose,
}: {
  run: Run;
  query: Run;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [spec, setSpec] = useState<BlockSpec>({
    name: '',
    gallery: 'autoTxt',
    category: '',
    description: '',
    behavior: 'content',
  });
  const [store, setStore] = useState<'document' | 'user'>('user');
  const [error, setError] = useState<string | null>(null);
  const [list] = useBlocks(query);
  // The first words of the selection name the block until the user types a name.
  useEffect(() => {
    void query({ cmd: 'copy' }).then((r) => {
      const name = suggestedBlockName(r?.clip?.text ?? '');
      if (name) setSpec((s) => (s.name ? s : { ...s, name }));
    });
  }, [query]);
  const categories = [...new Set((list ?? []).map((b) => b.category))].sort();
  const save = async () => {
    const r = await run({
      cmd: 'saveBuildingBlock',
      spec: { ...spec, name: spec.name.trim() },
      store,
    });
    if (r?.error) {
      setError(r.error);
      return;
    }
    clearBlockPreviews();
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('blocks.save.title')}
      testId="save-block"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="save-block-ok"
            disabled={!validName(spec.name)}
            onClick={() => void save()}
          >
            {t('blocks.save.ok')}
          </button>
        </>
      }
    >
      <div onKeyDown={(e) => e.stopPropagation()}>
        <SpecFields spec={spec} onChange={setSpec} categories={categories} />
        <label className={styles.field}>
          <span>{t('blocks.fields.store')}</span>
          <select
            data-testid="block-store"
            value={store}
            onChange={(e) => setStore(e.target.value === 'document' ? 'document' : 'user')}
          >
            <option value="user">{t('blocks.stores.user')}</option>
            <option value="document">{t('blocks.stores.document')}</option>
          </select>
        </label>
        <p className={styles.hint}>{t('blocks.save.hint')}</p>
        {error && (
          <p className={styles.error} role="alert" data-testid="save-block-error">
            {t(blockErrorKey(error) ?? 'blocks.errors.failed')}
          </p>
        )}
      </div>
    </Dialog>
  );
}

const STORE_ORDER: Record<BlockStore, number> = { document: 0, user: 1, builtin: 2 };

/** Organizer: every block with its preview; edit properties, delete, insert. */
export function BlocksOrganizer({
  run,
  query,
  page,
  onClose,
}: {
  run: Run;
  query: Run;
  page: number;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [list, reload] = useBlocks(query);
  const [picked, setPicked] = useState<string | null>(null);
  const [editing, setEditing] = useState<BlockSpec | null>(null);
  const [error, setError] = useState<string | null>(null);
  const blocks = [...(list ?? [])].sort(
    (a, b) =>
      a.gallery.localeCompare(b.gallery) ||
      STORE_ORDER[a.store] - STORE_ORDER[b.store] ||
      a.name.localeCompare(b.name),
  );
  const current = blocks.find((b) => b.key === picked) ?? null;
  const readOnly = !current || current.store === 'builtin';
  const categories = [...new Set(blocks.map((b) => b.category))].sort();

  const change = async (c: EditCommand) => {
    const r = await run(c);
    setError(r?.error ?? null);
    if (r?.error) return false;
    clearBlockPreviews();
    setPicked(null);
    setEditing(null);
    reload();
    return true;
  };

  return (
    <Dialog
      open
      width={760}
      onOpenChange={(o) => !o && onClose()}
      title={t('blocks.organizer.title')}
      testId="blocks-organizer"
      footer={
        <>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="organizer-edit"
            disabled={readOnly}
            onClick={() =>
              current &&
              setEditing({
                name: current.name,
                gallery: current.gallery,
                category: current.category,
                description: current.description,
                behavior: current.behavior,
              })
            }
          >
            {t('blocks.organizer.edit')}
          </button>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="organizer-delete"
            disabled={readOnly}
            onClick={() => current && void change({ cmd: 'deleteBuildingBlock', key: current.key })}
          >
            {t('blocks.organizer.delete')}
          </button>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="organizer-insert"
            disabled={!current}
            onClick={() => {
              if (!current) return;
              onClose();
              void run({ cmd: 'insertBuildingBlock', key: current.key, page });
            }}
          >
            {t('blocks.organizer.insert')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="organizer-close"
            onClick={onClose}
          >
            {t('blocks.organizer.close')}
          </button>
        </>
      }
    >
      <div className={own.organizer} onKeyDown={(e) => e.stopPropagation()}>
        <div className={own.tableWrap}>
          <table className={own.table} data-testid="organizer-list">
            <thead>
              <tr>
                <th>{t('blocks.fields.name')}</th>
                <th>{t('blocks.fields.gallery')}</th>
                <th>{t('blocks.fields.category')}</th>
                <th>{t('blocks.fields.store')}</th>
              </tr>
            </thead>
            <tbody>
              {blocks.map((b) => (
                <tr
                  key={b.key}
                  data-block={b.name}
                  data-store={b.store}
                  aria-selected={b.key === picked}
                  className={b.key === picked ? own.picked : undefined}
                  onClick={() => {
                    setPicked(b.key);
                    setEditing(null);
                    setError(null);
                  }}
                >
                  <td>{b.name}</td>
                  <td>{t(galleryLabelKey(b.gallery))}</td>
                  <td>{b.category}</td>
                  <td>{t(`blocks.stores.${b.store}`)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className={own.side}>
          {!current ? (
            <p className={styles.hint}>{t('blocks.organizer.pick')}</p>
          ) : editing ? (
            <div data-testid="organizer-form">
              <SpecFields spec={editing} onChange={setEditing} categories={categories} />
              <button
                type="button"
                className={styles.dialogPrimary}
                data-testid="organizer-apply"
                disabled={!validName(editing.name)}
                onClick={() =>
                  void change({
                    cmd: 'editBuildingBlock',
                    key: current.key,
                    spec: { ...editing, name: editing.name.trim() },
                  })
                }
              >
                {t('blocks.organizer.apply')}
              </button>
            </div>
          ) : (
            <>
              <div className={own.previewBox}>
                <BlockPreview block={current} query={query} />
              </div>
              <p className={own.detail}>
                <strong>{current.name}</strong>
                <br />
                {current.description}
              </p>
              <p className={styles.hint}>{t(`blocks.behaviors.${current.behavior}`)}</p>
              {current.store === 'builtin' && (
                <p className={styles.hint}>{t('blocks.organizer.builtin')}</p>
              )}
            </>
          )}
          {error && (
            <p className={styles.error} role="alert" data-testid="organizer-error">
              {t(blockErrorKey(error) ?? 'blocks.errors.failed')}
            </p>
          )}
        </div>
      </div>
    </Dialog>
  );
}
