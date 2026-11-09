import type { EditCommand, EditResult, FindResult, OutlineItem } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import { useEffect, useMemo, useRef, useState, type DragEvent } from 'react';
import { headingAt, headingsOf, hasChildren, visibleItems } from './outlineModel';
import { outlineDragPara, startOutlineDrag } from './OutlineView';
import styles from './Views.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

export type NavTab = 'headings' | 'pages' | 'results';

export interface NavigationPaneProps {
  tab: NavTab;
  onTab: (tab: NavTab) => void;
  /** Document outline (headings tab). */
  outline: OutlineItem[] | null;
  caretPara: number | undefined;
  /** Current page (0-based) and page count (pages tab). */
  page: number;
  pageCount: number;
  /** Bumped when the layout changes (thumbnails are drawn again). */
  layoutVersion: number;
  renderThumbnail: (index: number, canvas: HTMLCanvasElement, width: number) => Promise<void>;
  goToPage: (index: number) => void;
  run: Run;
  /** Clear the "highlight all" boxes of the results tab. */
  onClearHighlights: () => void;
  onClose: () => void;
}

const THUMB_WIDTH = 132;

/**
 * Navigation pane (docked left): heading tree (click to go, drag to reorganise,
 * collapse), page thumbnails and find results with their context.
 */
export function NavigationPane(p: NavigationPaneProps) {
  const { t } = useTranslation();
  const tabs: NavTab[] = ['headings', 'pages', 'results'];
  return (
    <aside className={styles.nav} aria-label={t('views.nav.title')} data-testid="navigation-pane">
      <div className={styles.navHeader}>
        <strong>{t('views.nav.title')}</strong>
        <button
          type="button"
          aria-label={t('views.nav.close')}
          title={t('views.nav.close')}
          onClick={p.onClose}
          data-testid="navigation-close"
        >
          ×
        </button>
      </div>
      <div className={styles.navTabs} role="tablist">
        {tabs.map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={p.tab === id}
            data-testid={`nav-tab-${id}`}
            className={p.tab === id ? styles.active : ''}
            onClick={() => p.onTab(id)}
          >
            {t(`views.nav.${id}`)}
          </button>
        ))}
      </div>
      <div className={styles.navBody} role="tabpanel">
        {p.tab === 'headings' && <HeadingTree {...p} />}
        {p.tab === 'pages' && <PageThumbnails {...p} />}
        {p.tab === 'results' && <Results {...p} />}
      </div>
    </aside>
  );
}

function HeadingTree({ outline, caretPara, run }: NavigationPaneProps) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set());
  const [dropOn, setDropOn] = useState<number | 'end' | null>(null);
  const heads = useMemo(() => headingsOf(outline ?? []), [outline]);
  const shown = useMemo(() => visibleItems(heads, collapsed, null), [heads, collapsed]);
  const current = headingAt(outline ?? [], caretPara)?.para;
  if (heads.length === 0) return <p className={styles.empty}>{t('views.nav.noHeadings')}</p>;
  const drop = (e: DragEvent, before: number | null) => {
    e.preventDefault();
    setDropOn(null);
    const para = outlineDragPara(e);
    if (para === null || para === before) return;
    void run({ cmd: 'moveOutlineTo', para, before });
  };
  return (
    <div role="tree" aria-label={t('views.nav.headings')} className={styles.tree}>
      {shown.map((h) => {
        const parent = hasChildren(heads, heads.indexOf(h));
        const open = !collapsed.has(h.para);
        return (
          <div
            key={h.para}
            role="treeitem"
            aria-level={(h.level ?? 0) + 1}
            aria-selected={h.para === current}
            aria-expanded={parent ? open : undefined}
            data-testid="nav-heading"
            data-para={h.para}
            className={`${styles.treeItem} ${h.para === current ? styles.selected : ''} ${
              dropOn === h.para ? styles.dropBefore : ''
            }`}
            style={{ paddingLeft: 4 + (h.level ?? 0) * 14 }}
            draggable
            onDragStart={(e) => startOutlineDrag(e, h.para)}
            onDragOver={(e) => {
              e.preventDefault();
              setDropOn(h.para);
            }}
            onDragLeave={() => setDropOn((d) => (d === h.para ? null : d))}
            onDrop={(e) => drop(e, h.para)}
          >
            <button
              type="button"
              className={styles.twisty}
              aria-label={parent ? t(open ? 'views.nav.collapse' : 'views.nav.expand') : undefined}
              aria-hidden={!parent}
              tabIndex={parent ? 0 : -1}
              onClick={() =>
                parent &&
                setCollapsed((c) => {
                  const n = new Set(c);
                  if (n.has(h.para)) n.delete(h.para);
                  else n.add(h.para);
                  return n;
                })
              }
            >
              {parent ? (open ? '▾' : '▸') : ''}
            </button>
            <button
              type="button"
              className={styles.treeLabel}
              title={h.text}
              onClick={() =>
                void run({
                  cmd: 'setSelection',
                  selection: { anchor: { p: h.para, o: 0 }, focus: { p: h.para, o: 0 } },
                })
              }
            >
              {h.text || ' '}
            </button>
          </div>
        );
      })}
      <div
        className={`${styles.dropEnd} ${dropOn === 'end' ? styles.dropBefore : ''}`}
        onDragOver={(e) => {
          e.preventDefault();
          setDropOn('end');
        }}
        onDragLeave={() => setDropOn(null)}
        onDrop={(e) => drop(e, null)}
      >
        {t('views.nav.dropEnd')}
      </div>
    </div>
  );
}

function Thumbnail({
  index,
  current,
  version,
  render,
  onGo,
}: {
  index: number;
  current: boolean;
  version: number;
  render: NavigationPaneProps['renderThumbnail'];
  onGo: (i: number) => void;
}) {
  const { t } = useTranslation();
  const ref = useRef<HTMLCanvasElement>(null);
  const [visible, setVisible] = useState(typeof IntersectionObserver === 'undefined');
  useEffect(() => {
    const c = ref.current;
    if (!c || visible) return;
    const io = new IntersectionObserver((es) => {
      if (es.some((e) => e.isIntersecting)) setVisible(true);
    });
    io.observe(c);
    return () => io.disconnect();
  }, [visible]);
  useEffect(() => {
    const c = ref.current;
    if (c && visible) void render(index, c, THUMB_WIDTH).catch(() => {});
  }, [index, version, visible, render]);
  const label = t('views.nav.page', { n: index + 1 });
  return (
    <button
      type="button"
      className={`${styles.thumb} ${current ? styles.selected : ''}`}
      aria-label={label}
      aria-current={current ? 'page' : undefined}
      data-testid="nav-thumbnail"
      onClick={() => onGo(index)}
    >
      <canvas ref={ref} width={THUMB_WIDTH} height={Math.round(THUMB_WIDTH * 1.414)} />
      <span>{index + 1}</span>
    </button>
  );
}

function PageThumbnails({
  page,
  pageCount,
  layoutVersion,
  renderThumbnail,
  goToPage,
}: NavigationPaneProps) {
  return (
    <div className={styles.thumbs}>
      {Array.from({ length: pageCount }, (_, i) => (
        <Thumbnail
          key={i}
          index={i}
          current={i === page}
          version={layoutVersion}
          render={renderThumbnail}
          onGo={goToPage}
        />
      ))}
    </div>
  );
}

function Results({ run, onClearHighlights, goToPage }: NavigationPaneProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<FindResult[] | null>(null);
  const [active, setActive] = useState(-1);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const search = (q: string) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      if (!q) {
        setResults(null);
        onClearHighlights();
        return;
      }
      void run({ cmd: 'findResults', query: q }).then((r) => {
        setResults(r?.results ?? []);
        setActive(-1);
      });
    }, 200);
  };
  useEffect(() => () => clearTimeout(timer.current), []);
  const pick = (i: number) => {
    const r = results?.[i];
    if (!r) return;
    setActive(i);
    goToPage(r.page);
    void run({ cmd: 'selectMatch', match: r.match });
  };
  return (
    <div className={styles.results}>
      <input
        type="search"
        value={query}
        placeholder={t('views.nav.search')}
        aria-label={t('views.nav.search')}
        data-testid="nav-search"
        onChange={(e) => {
          setQuery(e.target.value);
          search(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && results?.length) {
            e.preventDefault();
            pick((active + (e.shiftKey ? results.length - 1 : 1)) % results.length);
          }
        }}
      />
      {results && (
        <p className={styles.count} data-testid="nav-result-count">
          {results.length
            ? t('views.nav.resultCount', { count: results.length })
            : t('views.nav.noResults')}
        </p>
      )}
      <ol className={styles.resultList}>
        {results?.map((r, i) => (
          <li key={`${r.match.p}:${r.match.start}`}>
            <button
              type="button"
              data-testid="nav-result"
              className={i === active ? styles.selected : ''}
              onClick={() => pick(i)}
            >
              <span>{r.before}</span>
              <mark>{r.text}</mark>
              <span>{r.after}</span>
              <small>{t('views.nav.page', { n: r.page + 1 })}</small>
            </button>
          </li>
        ))}
      </ol>
    </div>
  );
}
