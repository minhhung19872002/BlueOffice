import { useTranslation } from '@blueoffice/i18n';
import type { ControlInfo, SelRect } from '@blueoffice/platform';
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import styles from './Editor.module.css';

/** Screen rectangle (client pixels) of a rectangle of the document (points on a page). */
export type ToClient = (
  r: SelRect,
) => { left: number; top: number; width: number; height: number } | null;

/** Re-render when the document scrolls or the window resizes (overlays follow the page). */
function useFollow(scrollerOf?: () => HTMLElement | null | undefined): void {
  const [, setTick] = useState(0);
  useEffect(() => {
    const scroller = scrollerOf?.();
    const bump = () => setTick((n) => n + 1);
    scroller?.addEventListener('scroll', bump, { passive: true });
    window.addEventListener('resize', bump);
    return () => {
      scroller?.removeEventListener('scroll', bump);
      window.removeEventListener('resize', bump);
    };
  }, [scrollerOf]);
}

/**
 * Ctrl+F9 in place: the code of a new (or shown) field typed between its braces where the field
 * is (ECMA-376 Part 1, §17.16.1). Enter / Esc / leaving keep the code shown; F9 updates the field
 * so it shows its result, as a word processor does.
 */
export function FieldCodeEditor({
  rect,
  initial,
  toClient,
  scroller,
  onCommit,
}: {
  rect: SelRect;
  initial: string;
  toClient: ToClient;
  scroller?: () => HTMLElement | null | undefined;
  /** `update`: F9 was pressed (update the field so it shows its result). */
  onCommit: (code: string, update: boolean) => void;
}) {
  const { t } = useTranslation();
  useFollow(scroller);
  const [code, setCode] = useState(initial);
  const done = useRef(false);
  const commit = (update: boolean) => {
    if (done.current) return;
    done.current = true;
    onCommit(code, update);
  };
  const box = toClient(rect);
  if (!box) return null;
  const h = Math.max(box.height, 18);
  return (
    <div
      className={styles.fieldCodeInPlace}
      data-testid="field-code-inplace"
      style={{ left: box.left, top: box.top, height: h, fontSize: Math.max(11, h * 0.72) }}
    >
      <span aria-hidden="true">{'{ '}</span>
      <input
        data-testid="field-code-input"
        aria-label={t('fieldCode.inPlace')}
        title={t('fieldCode.inPlace')}
        value={code}
        spellCheck={false}
        autoFocus
        size={Math.max(4, code.length + 1)}
        onChange={(e) => setCode(e.target.value)}
        onBlur={() => commit(false)}
        onKeyDown={(e) => {
          if (e.key === 'F9' && !e.ctrlKey && !e.altKey) {
            e.preventDefault();
            e.stopPropagation();
            commit(!e.shiftKey);
          } else if (e.key === 'Enter' || e.key === 'Escape') {
            e.preventDefault();
            e.stopPropagation();
            commit(false);
          }
        }}
      />
      <span aria-hidden="true">{' }'}</span>
    </div>
  );
}

/** The kinds of content control that open a list on their own frame. */
export function dropKind(c: ControlInfo | undefined | null): 'list' | 'date' | 'gallery' | null {
  if (!c || c.locked) return null;
  if (c.kind === 'dropDown' || c.kind === 'comboBox') return 'list';
  if (c.kind === 'date') return 'date';
  if (c.kind === 'buildingBlockGallery') return 'gallery';
  return null;
}

/** A building block offered by a gallery control. */
export interface GalleryBlock {
  key: string;
  name: string;
}

/**
 * The drop-down arrow at the right end of a list / date / gallery content control, as a word
 * processor shows it on the control itself, and the list, calendar or gallery it opens
 * (Alt+Down opens it from the keyboard; arrows move, Enter picks, Esc closes).
 */
export function ControlDropDown({
  control,
  toClient,
  scroller,
  open,
  onOpenChange,
  onPick,
  blocks,
  lang,
}: {
  control: ControlInfo;
  toClient: ToClient;
  scroller?: () => HTMLElement | null | undefined;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** A list item's display text, a date (`yyyy-mm-dd`) or a building block key. */
  onPick: (value: string) => void;
  blocks: GalleryBlock[];
  lang: string;
}) {
  const { t } = useTranslation();
  useFollow(scroller);
  const kind = dropKind(control);
  const last = control.frame[control.frame.length - 1];
  if (!kind || !last) return null;
  const box = toClient(last);
  if (!box) return null;
  const label =
    kind === 'list'
      ? t('controlBar.openList')
      : kind === 'date'
        ? t('controlBar.openCalendar')
        : t('controlBar.openGallery');
  const size = Math.max(24, box.height);
  const left = box.left + box.width + 3;
  return (
    <>
      <button
        type="button"
        className={styles.ccDropButton}
        data-testid="control-drop"
        aria-label={label}
        title={label}
        aria-haspopup={kind === 'date' ? 'dialog' : 'listbox'}
        aria-expanded={open}
        style={{ left, top: box.top + box.height / 2 - size / 2, width: 24, height: size }}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => onOpenChange(!open)}
      >
        {kind === 'date' ? <CalendarGlyph /> : <span aria-hidden="true">▾</span>}
      </button>
      {open && (
        <div
          className={styles.ccPopup}
          style={{ left: Math.max(4, left + 24 - 240), top: box.top + box.height + 4 }}
        >
          {kind === 'list' && (
            <ListPopup items={control.items} onPick={onPick} onClose={() => onOpenChange(false)} />
          )}
          {kind === 'gallery' && (
            <ListPopup
              items={blocks.map((b) => b.name)}
              empty={t('controlBar.galleryEmpty')}
              onPick={(name) => {
                const b = blocks.find((x) => x.name === name);
                if (b) onPick(b.key);
              }}
              onClose={() => onOpenChange(false)}
            />
          )}
          {kind === 'date' && (
            <Calendar
              value={control.date?.slice(0, 10) ?? null}
              lang={lang}
              onPick={onPick}
              onClose={() => onOpenChange(false)}
            />
          )}
        </div>
      )}
    </>
  );
}

function CalendarGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
      <rect x="1.5" y="2.5" width="11" height="10" rx="1" fill="none" stroke="currentColor" />
      <path d="M1.5 5.5h11M4.5 1v3M9.5 1v3" stroke="currentColor" />
    </svg>
  );
}

function ListPopup({
  items,
  empty,
  onPick,
  onClose,
}: {
  items: string[];
  empty?: string;
  onPick: (v: string) => void;
  onClose: () => void;
}) {
  const [active, setActive] = useState(0);
  const ref = useRef<HTMLUListElement>(null);
  useLayoutEffect(() => ref.current?.focus(), []);
  const key = (e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') setActive((a) => Math.min(items.length - 1, a + 1));
    else if (e.key === 'ArrowUp') setActive((a) => Math.max(0, a - 1));
    else if (e.key === 'Home') setActive(0);
    else if (e.key === 'End') setActive(items.length - 1);
    else if (e.key === 'Enter' || e.key === ' ') {
      const v = items[active];
      if (v !== undefined) onPick(v);
    } else if (e.key === 'Escape' || e.key === 'Tab') onClose();
    else return;
    e.preventDefault();
    e.stopPropagation();
  };
  if (items.length === 0) return <div className={styles.ccEmpty}>{empty}</div>;
  return (
    <ul
      ref={ref}
      role="listbox"
      tabIndex={0}
      data-testid="control-list"
      className={styles.ccList}
      aria-activedescendant={`cc-item-${active}`}
      onKeyDown={key}
    >
      {items.map((it, i) => (
        <li
          key={`${i}-${it}`}
          id={`cc-item-${i}`}
          role="option"
          aria-selected={i === active}
          className={i === active ? styles.ccActive : undefined}
          onMouseDown={(e) => e.preventDefault()}
          onMouseEnter={() => setActive(i)}
          onClick={() => onPick(it)}
        >
          {it}
        </li>
      ))}
    </ul>
  );
}

const pad = (n: number) => String(n).padStart(2, '0');
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

/** A month calendar: arrows move by a day / week, PageUp / PageDown by a month, Enter picks. */
function Calendar({
  value,
  lang,
  onPick,
  onClose,
}: {
  value: string | null;
  lang: string;
  onPick: (v: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const start = value ? new Date(`${value}T00:00:00`) : new Date();
  const [focus, setFocus] = useState(() => (Number.isNaN(start.getTime()) ? new Date() : start));
  const grid = useRef<HTMLTableElement>(null);
  useLayoutEffect(() => grid.current?.focus(), []);
  const month = new Date(focus.getFullYear(), focus.getMonth(), 1);
  // Weeks start on Monday (ISO 8601).
  const lead = (month.getDay() + 6) % 7;
  const first = new Date(month);
  first.setDate(1 - lead);
  const days = Array.from({ length: 42 }, (_, i) => {
    const d = new Date(first);
    d.setDate(first.getDate() + i);
    return d;
  });
  const fmtMonth = new Intl.DateTimeFormat(lang, { month: 'long', year: 'numeric' });
  const fmtDay = new Intl.DateTimeFormat(lang, { weekday: 'narrow' });
  const fmtFull = new Intl.DateTimeFormat(lang, { dateStyle: 'full' });
  const move = (days: number, months = 0) => {
    const d = new Date(focus);
    if (months) d.setMonth(d.getMonth() + months);
    d.setDate(d.getDate() + days);
    setFocus(d);
  };
  const key = (e: KeyboardEvent) => {
    if (e.key === 'ArrowLeft') move(-1);
    else if (e.key === 'ArrowRight') move(1);
    else if (e.key === 'ArrowUp') move(-7);
    else if (e.key === 'ArrowDown') move(7);
    else if (e.key === 'PageUp') move(0, -1);
    else if (e.key === 'PageDown') move(0, 1);
    else if (e.key === 'Enter' || e.key === ' ') onPick(iso(focus));
    else if (e.key === 'Escape') onClose();
    else return;
    e.preventDefault();
    e.stopPropagation();
  };
  const today = iso(new Date());
  return (
    <div
      role="dialog"
      aria-label={t('calendar.label')}
      data-testid="control-calendar"
      className={styles.ccCalendar}
    >
      <div className={styles.ccCalHead}>
        <button
          type="button"
          aria-label={t('calendar.prevMonth')}
          title={t('calendar.prevMonth')}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => move(0, -1)}
        >
          ‹
        </button>
        <span data-testid="control-calendar-month" aria-live="polite">
          {fmtMonth.format(month)}
        </span>
        <button
          type="button"
          aria-label={t('calendar.nextMonth')}
          title={t('calendar.nextMonth')}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => move(0, 1)}
        >
          ›
        </button>
      </div>
      <table
        ref={grid}
        role="grid"
        tabIndex={0}
        aria-label={fmtMonth.format(month)}
        aria-activedescendant={`cc-day-${iso(focus)}`}
        onKeyDown={key}
      >
        <thead>
          <tr>
            {days.slice(0, 7).map((d) => (
              <th key={d.getDay()} scope="col">
                {fmtDay.format(d)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {Array.from({ length: 6 }, (_, w) => (
            <tr key={w}>
              {days.slice(w * 7, w * 7 + 7).map((d) => {
                const id = iso(d);
                const cls = [
                  d.getMonth() !== month.getMonth() ? styles.ccOther : '',
                  id === iso(focus) ? styles.ccActive : '',
                  id === value ? styles.ccChosen : '',
                  id === today ? styles.ccToday : '',
                ]
                  .filter(Boolean)
                  .join(' ');
                return (
                  <td
                    key={id}
                    id={`cc-day-${id}`}
                    role="gridcell"
                    aria-selected={id === value}
                    aria-label={fmtFull.format(d)}
                    data-date={id}
                    className={cls || undefined}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => onPick(id)}
                  >
                    {d.getDate()}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      <button
        type="button"
        className={styles.ccTodayButton}
        data-testid="control-calendar-today"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => onPick(today)}
      >
        {t('calendar.today')}
      </button>
    </div>
  );
}
