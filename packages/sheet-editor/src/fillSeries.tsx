/**
 * Home › Fill › Series: a series from the first cell of each column or row of the
 * selection — linear (add the step), growth (multiply), dates (day, weekday, month, year)
 * or as the fill handle would continue it — up to an optional stop value. The engine command
 * `fillSeries` writes it as one undo step.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { SheetHost, SheetRange } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import { parseNumber } from './grid';
import styles from './SheetEditor.module.css';

type Kind = 'linear' | 'growth' | 'date' | 'autoFill';
type Unit = 'day' | 'weekday' | 'month' | 'year';

export function SeriesDialog({
  host,
  sheet,
  range,
  firstIsDate,
  decimal,
  onClose,
  onDone,
  fail,
}: {
  host: SheetHost;
  sheet: number;
  range: SheetRange;
  /** The first cell holds a date: the Date type is proposed. */
  firstIsDate: boolean;
  decimal: ',' | '.';
  onClose: () => void;
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  // More rows than columns: down the columns.
  const [columns, setColumns] = useState(range.r1 - range.r0 >= range.c1 - range.c0);
  const [kind, setKind] = useState<Kind>(firstIsDate ? 'date' : 'linear');
  const [unit, setUnit] = useState<Unit>('day');
  const [step, setStep] = useState('1');
  const [stop, setStop] = useState('');
  const [error, setError] = useState('');
  const ok = async () => {
    const s = parseNumber(step, decimal);
    const e = stop.trim() === '' ? null : parseNumber(stop, decimal);
    if (s === null || (stop.trim() !== '' && e === null)) {
      setError(t('sheetEditor.series.badNumber'));
      return;
    }
    try {
      await host.exec({
        cmd: 'fillSeries',
        sheet,
        range,
        columns,
        kind,
        ...(kind === 'date' ? { unit } : {}),
        step: s,
        stop: e,
      });
      await onDone();
      onClose();
    } catch (err) {
      fail(err);
    }
  };
  const radio = <T extends string>(
    name: string,
    value: T,
    current: T,
    set: (v: T) => void,
    label: string,
    disabled = false,
  ) => (
    <label className={styles.check}>
      <input
        type="radio"
        name={name}
        data-testid={`series-${name}-${value}`}
        checked={current === value}
        disabled={disabled}
        onChange={() => set(value)}
      />
      {label}
    </label>
  );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.series.title')}
      testId="series-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="series-ok"
            onClick={() => void ok()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.row}>
        <fieldset>
          <legend>{t('sheetEditor.series.in')}</legend>
          {radio(
            'in',
            'rows',
            columns ? 'columns' : 'rows',
            (v) => setColumns(v === 'columns'),
            t('sheetEditor.series.rows'),
          )}
          {radio(
            'in',
            'columns',
            columns ? 'columns' : 'rows',
            (v) => setColumns(v === 'columns'),
            t('sheetEditor.series.columns'),
          )}
        </fieldset>
        <fieldset>
          <legend>{t('sheetEditor.series.type')}</legend>
          {radio('type', 'linear', kind, setKind, t('sheetEditor.series.linear'))}
          {radio('type', 'growth', kind, setKind, t('sheetEditor.series.growth'))}
          {radio('type', 'date', kind, setKind, t('sheetEditor.series.date'))}
          {radio('type', 'autoFill', kind, setKind, t('sheetEditor.series.autoFill'))}
        </fieldset>
        <fieldset>
          <legend>{t('sheetEditor.series.unit')}</legend>
          {radio('unit', 'day', unit, setUnit, t('sheetEditor.series.day'), kind !== 'date')}
          {radio(
            'unit',
            'weekday',
            unit,
            setUnit,
            t('sheetEditor.series.weekday'),
            kind !== 'date',
          )}
          {radio('unit', 'month', unit, setUnit, t('sheetEditor.series.month'), kind !== 'date')}
          {radio('unit', 'year', unit, setUnit, t('sheetEditor.series.year'), kind !== 'date')}
        </fieldset>
      </div>
      <div className={styles.form}>
        <label>
          {t('sheetEditor.series.step')}
          <input
            data-testid="series-step"
            value={step}
            disabled={kind === 'autoFill'}
            onChange={(e) => {
              setStep(e.target.value);
              setError('');
            }}
          />
        </label>
        <label>
          {t('sheetEditor.series.stop')}
          <input
            data-testid="series-stop"
            value={stop}
            disabled={kind === 'autoFill'}
            onChange={(e) => {
              setStop(e.target.value);
              setError('');
            }}
          />
        </label>
      </div>
      {error && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
