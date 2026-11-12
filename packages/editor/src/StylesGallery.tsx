import type { DocStyleEntry } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import { Popover } from '@blueoffice/ui';
import { ChevronDown, Ellipsis } from 'lucide-react';
import { useEffect, useState } from 'react';
import { visibleStyles } from './StylesPane';
import styles from './StylesGallery.module.css';

/** Tiles shown in the ribbon (the rest are in the styles pane). */
const TILES = 3;

/** Built-in styles: their command (`style.heading1`, on the tile as `data-style-command`) and
 * their name in the UI language. */
const BUILTIN: Record<string, [string, string]> = {
  Normal: ['style.normal', 'commands.format.styleNormal'],
  Title: ['style.title', 'commands.format.styleTitle'],
  Heading1: ['style.heading1', 'commands.format.styleHeading1'],
  Heading2: ['style.heading2', 'commands.format.styleHeading2'],
  Heading3: ['style.heading3', 'commands.format.styleHeading3'],
};

export interface StylesGalleryProps {
  /** Paragraph and character styles of the document (`styleList`). */
  load: () => Promise<DocStyleEntry[] | null>;
  /** Style of the paragraph at the caret (`null`: Normal). */
  current: string | null;
  /** Changes when the list may have changed (document opened, style applied). */
  refreshKey: string;
  disabled?: boolean;
  apply: (id: string) => void;
  /** The ▾ button: every style (the styles pane). */
  onMore: () => void;
  /** Style commands in the ⋯ menu under ▾ (new style, inspector, reveal, organizer). */
  tools?: { id: string; label: string; run: () => void }[];
}

/**
 * Home › Styles: the quick styles of the document (§17.7.4.14 qFormat, ordered by
 * §17.7.4.19 uiPriority) as a row of tiles drawn in their own formatting, the style at the
 * caret pressed — the gallery word processors show — and a button for all the styles.
 */
export function StylesGallery({
  load,
  current,
  refreshKey,
  disabled,
  apply,
  onMore,
  tools,
}: StylesGalleryProps) {
  const { t } = useTranslation();
  const [list, setList] = useState<DocStyleEntry[]>([]);
  useEffect(() => {
    if (disabled) return;
    let live = true;
    void load().then((l) => {
      if (live && l) setList(l);
    });
    return () => {
      live = false;
    };
  }, [load, refreshKey, disabled]);
  const quick = visibleStyles(
    list.filter((s) => s.quick && s.kind === 'paragraph'),
    'recommended',
    '',
  ).slice(0, TILES);
  const at = current ?? 'Normal';
  return (
    <div
      className={styles.gallery}
      role="group"
      aria-label={t('ribbon.groups.styles')}
      data-testid="styles-gallery"
    >
      {quick.map((s) => {
        const p = s.preview;
        const builtin = BUILTIN[s.id];
        const name = builtin ? t(builtin[1]) : s.name;
        return (
          <button
            key={s.id}
            type="button"
            className={styles.tile}
            data-style-id={s.id}
            data-style-command={builtin?.[0]}
            aria-pressed={s.id === at}
            title={name}
            disabled={disabled}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => apply(s.id)}
          >
            <span
              className={styles.sample}
              style={{
                fontFamily: `"${p.font}", serif`,
                fontSize: `${Math.min(15, Math.max(9, p.size))}px`,
                fontWeight: p.bold ? 700 : 400,
                fontStyle: p.italic ? 'italic' : 'normal',
                textDecoration: p.underline ? 'underline' : 'none',
                color: `#${p.color}`,
              }}
            >
              AaBbCc
            </span>
            <span className={styles.name}>{name}</span>
          </button>
        );
      })}
      <div className={styles.side}>
        <button
          type="button"
          className={styles.more}
          data-testid="styles-gallery-more"
          aria-label={t('commands.view.stylesPane')}
          title={t('commands.view.stylesPane')}
          disabled={disabled}
          onClick={onMore}
        >
          <ChevronDown size={14} />
        </button>
        {tools && tools.length > 0 && (
          // The style commands, under ▾ in the same narrow column (no extra ribbon width).
          <Popover
            label={t('ribbon.groups.styleTools')}
            disabled={disabled}
            testId="styles-gallery-tools"
            className={styles.more}
            trigger={<Ellipsis size={14} />}
          >
            {(close) => (
              <div className={styles.menu} role="menu" aria-label={t('ribbon.groups.styleTools')}>
                {tools.map((x) => (
                  <button
                    key={x.id}
                    type="button"
                    role="menuitem"
                    data-command={x.id}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => {
                      close();
                      x.run();
                    }}
                  >
                    {x.label}
                  </button>
                ))}
              </div>
            )}
          </Popover>
        )}
      </div>
    </div>
  );
}
