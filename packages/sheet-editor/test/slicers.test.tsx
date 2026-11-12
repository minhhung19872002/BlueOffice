// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PivotInfo, SlicerInfo, TimelineInfo } from '@blueoffice/platform';
import {
  FrameOptionsDialog,
  InsertFramesDialog,
  PivotChartFilters,
  SLICER_STYLE_NAMES,
  SlicerFrame,
  TIMELINE_STYLE_NAMES,
  TimelineFrame,
  chartFilterFields,
  frameStyleLabel,
  nextFrameStyleName,
  nextSlicerChoice,
  periodAt,
  periodRange,
  previewColors,
  selectionLabel,
  slicerFields,
  slicerRangeChoice,
  timelineFields,
  timelineShowOf,
} from '../src/slicers';

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = i18n.t.bind(i18n);
const ui = (el: ReactElement) => render(<I18nextProvider i18n={i18n}>{el}</I18nextProvider>);

const field = (name: string, over: Partial<PivotInfo['fields'][number]> = {}) => ({
  name,
  items: [] as string[],
  formula: null,
  derived: false,
  date: false,
  group: null,
  ...over,
});

const options = (hidden: number[] = []) => ({
  subtotal: true,
  subtotalTop: true,
  blankRow: false,
  repeatLabels: false,
  sort: 'manual' as PivotInfo['fieldOptions'][number]['sort'],
  sortBy: null,
  hidden,
});

/** Region (row field, "North" hidden), Kind (column field), Date, Amount, a calculated field
 *  and a grouping field; Channel is a report filter showing only "Shop". */
const pivot = (over: Partial<PivotInfo> = {}): PivotInfo =>
  ({
    index: 0,
    name: 'Pivot1',
    location: { r0: 2, c0: 0, r1: 6, c1: 3 },
    area: { r0: 0, c0: 0, r1: 6, c1: 3 },
    source: 'Data!A1:F9',
    fields: [
      field('Region', { items: ['North', 'South', 'West'] }),
      field('Kind', { items: ['A', 'B'] }),
      field('Date', { date: true, items: ['1/15/2024'] }),
      field('Amount'),
      field('Margin', { formula: 'Amount*0.1', derived: true }),
      field('Years', { derived: true, date: true }),
      field('Channel', { items: ['Online', 'Shop'] }),
    ],
    rows: [0],
    cols: [1],
    values: [],
    filters: [{ field: 6, items: [1] }],
    layout: 'tabular',
    rowGrandTotals: true,
    colGrandTotals: true,
    fieldOptions: [options([0]), options(), options(), options(), options(), options(), options()],
    fieldFilters: [],
    style: null,
    dataOnRows: false,
    ...over,
  }) as PivotInfo;

const slicer = (over: Partial<SlicerInfo> = {}): SlicerInfo => ({
  index: 2,
  name: 'Region',
  caption: 'Region',
  kind: 'pivot',
  field: 'Region',
  style: 'SlicerStyleDark2',
  colors: {
    selected: '#E07B39',
    selectedText: '#FFFFFF',
    header: '#E07B39',
    headerText: '#FFFFFF',
    border: '#E07B39',
  },
  columns: 2,
  from: [1, 4],
  to: [9, 6],
  items: [
    { label: 'North', selected: true },
    { label: 'South', selected: false },
    { label: '', selected: true },
  ],
  pivots: [
    { sheet: 1, sheetName: 'Report', index: 0, name: 'Pivot1', connected: true },
    { sheet: 1, sheetName: 'Report', index: 1, name: 'Pivot2', connected: false },
  ],
  ...over,
});

const timeline = (over: Partial<TimelineInfo> = {}): TimelineInfo => ({
  index: 0,
  name: 'Date',
  caption: 'Date',
  field: 'Date',
  level: 2,
  style: 'TimeSlicerStyleLight1',
  from: [1, 4],
  to: [8, 10],
  filtered: true,
  periods: [
    { label: '01/2024', start: 45292, end: 45322, selected: false },
    { label: '02/2024', start: 45323, end: 45351, selected: true },
    { label: '03/2024', start: 45352, end: 45382, selected: true },
    { label: '04/2024', start: 45383, end: 45412, selected: false },
  ],
  pivots: [{ sheet: 1, sheetName: 'Report', index: 0, name: 'Pivot1', connected: true }],
  ...over,
});

const frame = (over: { selected?: boolean; editable?: boolean } = {}) => ({
  rect: { x: 10, y: 20, w: 180, h: 200 },
  selected: false,
  editable: true,
  onSelect: vi.fn(),
  onDragStart: vi.fn(),
  onSettings: vi.fn(),
  onDelete: vi.fn(),
  ...over,
});

describe('slicer and timeline helpers', () => {
  it('names the built-in styles by family and number', () => {
    expect(SLICER_STYLE_NAMES).toHaveLength(14);
    expect(TIMELINE_STYLE_NAMES).toHaveLength(12);
    expect(frameStyleLabel('SlicerStyleLight3', t)).toBe('Light 3');
    expect(frameStyleLabel('TimeSlicerStyleDark6', t)).toBe('Dark 6');
    expect(frameStyleLabel('SlicerStyleOther2', t)).toBe('Grey 2');
    expect(frameStyleLabel('My style', t)).toBe('My style');
  });

  it('offers source fields for slicers and date fields for timelines', () => {
    // Calculated and grouping fields take no slicer; only the date column takes a timeline.
    expect(slicerFields(pivot())).toEqual([0, 1, 2, 3, 6]);
    expect(timelineFields(pivot())).toEqual([2]);
    const grouped = pivot();
    grouped.fields[0]!.group = { by: 'discrete' } as PivotInfo['fields'][number]['group'];
    expect(slicerFields(grouped)).toEqual([1, 2, 3, 6]);
  });

  it('works out the next choice of a slicer', () => {
    const all = slicer({
      items: [
        { label: 'a', selected: true },
        { label: 'b', selected: true },
      ],
    });
    const one = slicer({
      items: [
        { label: 'a', selected: true },
        { label: 'b', selected: false },
        { label: 'c', selected: false },
      ],
    });
    // Plain click: only that item; clicking the only chosen item clears the filter.
    expect(nextSlicerChoice(all, 'b', false)).toEqual(['b']);
    expect(nextSlicerChoice(one, 'b', false)).toEqual(['b']);
    expect(nextSlicerChoice(one, 'a', false)).toEqual([]);
    // Ctrl: add, remove; with every item chosen the click takes that one out.
    expect(nextSlicerChoice(one, 'c', true)).toEqual(['a', 'c']);
    expect(nextSlicerChoice(one, 'a', true)).toEqual([]);
    expect(nextSlicerChoice(all, 'a', true)).toEqual(['b']);
  });

  it('takes the items between the last clicked one and a Shift click', () => {
    const s = slicer({
      items: ['a', 'b', 'c', 'd'].map((label) => ({ label, selected: true })),
    });
    expect(slicerRangeChoice(s, 'b', 'd')).toEqual(['b', 'c', 'd']);
    expect(slicerRangeChoice(s, 'd', 'a')).toEqual(['a', 'b', 'c', 'd']);
    expect(slicerRangeChoice(s, null, 'c')).toEqual(['c']);
    expect(slicerRangeChoice(s, 'zz', 'c')).toEqual(['c']);
    expect(slicerRangeChoice(s, 'a', 'zz')).toEqual([]);
  });

  it('finds the period a scrolled strip starts at and the Show options', () => {
    const tl = timeline();
    expect(periodAt(tl.periods, null)).toBe(0);
    expect(periodAt(tl.periods, 45383)).toBe(3);
    // A day inside March: March is shown first.
    expect(periodAt(tl.periods, 45360)).toBe(2);
    expect(periodAt(tl.periods, 99999)).toBe(3);
    expect(timelineShowOf(tl)).toEqual({
      header: true,
      selectionLabel: true,
      timeLevel: true,
      scrollbar: true,
    });
    expect(timelineShowOf(timeline({ showHeader: false, showScrollbar: false }))).toEqual({
      header: false,
      selectionLabel: true,
      timeLevel: true,
      scrollbar: false,
    });
  });

  it('names a new style after the ones in use and previews its colours', () => {
    expect(nextFrameStyleName('slicer', [], t)).toBe('Slicer style 1');
    expect(nextFrameStyleName('slicer', ['slicer style 1', 'Slicer style 2'], t)).toBe(
      'Slicer style 3',
    );
    expect(nextFrameStyleName('timeline', ['Slicer style 1'], t)).toBe('Timeline style 1');
    const base = slicer().colors!;
    const c = previewColors(base, {
      whole: { fill: '#F2F2F2', border: '#1F4E79', text: '#333333' },
      header: { text: '#002060' },
      unselected: { fill: '#FFF2CC' },
    });
    expect(c).toEqual({
      ...base,
      background: '#F2F2F2',
      border: '#1F4E79',
      headerText: '#002060',
      unselected: '#FFF2CC',
      unselectedText: '#333333',
    });
  });

  it('turns periods into a day range and a label', () => {
    const tl = timeline();
    expect(periodRange(tl, 3, 1)).toEqual({ start: 45323, end: 45412 });
    expect(periodRange(tl, 0, 0)).toEqual({ start: 45292, end: 45322 });
    expect(periodRange(tl, 0, 9)).toBeNull();
    expect(selectionLabel(tl, t)).toBe('02/2024 – 03/2024');
    expect(selectionLabel(timeline({ filtered: false }), t)).toBe('All periods');
    const single = timeline();
    single.periods[2]!.selected = false;
    expect(selectionLabel(single, t)).toBe('02/2024');
  });

  it('lists the fields of a pivot chart with their hidden items', () => {
    expect(chartFilterFields(pivot())).toEqual([
      // The report filter shows only item 1: item 0 is hidden.
      { field: 6, name: 'Channel', area: 'filter', items: ['Online', 'Shop'], hidden: [0] },
      { field: 0, name: 'Region', area: 'axis', items: ['North', 'South', 'West'], hidden: [0] },
      { field: 1, name: 'Kind', area: 'legend', items: ['A', 'B'], hidden: [] },
    ]);
    expect(chartFilterFields(pivot({ rows: [], cols: [], filters: [] }))).toEqual([]);
  });
});

describe('SlicerFrame', () => {
  it('draws the items with the colours of the style and reports clicks', () => {
    const onChoose = vi.fn();
    const onClear = vi.fn();
    // Selected: the options and delete buttons show over the frame.
    const c = frame({ selected: true });
    ui(<SlicerFrame {...c} info={slicer()} onChoose={onChoose} onClear={onClear} />);
    const box = screen.getByTestId('slicer');
    expect(box.getAttribute('data-style')).toBe('SlicerStyleDark2');
    expect(box.style.getPropertyValue('--frame-on')).toBe('#E07B39');
    expect(box.style.getPropertyValue('--frame-head-text')).toBe('#FFFFFF');
    expect(box.style.left).toBe('10px');
    expect(box.style.width).toBe('180px');
    const buttons = within(box).getAllByRole('button', { pressed: true });
    expect(buttons.map((b) => b.textContent)).toEqual(['North', '(blank)']);
    // A click keeps only that item; Ctrl adds it to the chosen ones (North and blank).
    fireEvent.click(within(box).getByRole('button', { name: 'South' }));
    expect(onChoose).toHaveBeenLastCalledWith(['South']);
    fireEvent.click(within(box).getByRole('button', { name: 'South' }), { ctrlKey: true });
    expect(onChoose).toHaveBeenLastCalledWith(['North', '', 'South']);
    fireEvent.click(screen.getByTestId('slicer-clear'));
    expect(onClear).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId('slicer-settings'));
    expect(c.onSettings).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId('slicer-delete'));
    expect(c.onDelete).toHaveBeenCalledTimes(1);
  });

  it('is selected by a click, moved by its header, sized by its handle, deleted by Delete', () => {
    const c = frame();
    const { rerender } = ui(
      <SlicerFrame {...c} info={slicer()} onChoose={vi.fn()} onClear={vi.fn()} />,
    );
    expect(screen.queryByTestId('slicer-resize')).toBeNull();
    fireEvent.mouseDown(screen.getByTestId('slicer-header'), { button: 0 });
    expect(c.onSelect).toHaveBeenCalledTimes(1);
    expect(c.onDragStart).toHaveBeenCalledTimes(1);
    expect(c.onDragStart.mock.calls[0]![1]).toBe(false);
    // Not selected: no options / delete buttons; the header buttons do not start a move.
    expect(screen.queryByTestId('slicer-settings')).toBeNull();
    fireEvent.mouseDown(screen.getByTestId('slicer-multi'), { button: 0 });
    expect(c.onDragStart).toHaveBeenCalledTimes(1);
    rerender(
      <I18nextProvider i18n={i18n}>
        <SlicerFrame {...c} selected info={slicer()} onChoose={vi.fn()} onClear={vi.fn()} />
      </I18nextProvider>,
    );
    fireEvent.mouseDown(screen.getByTestId('slicer-settings'), { button: 0 });
    expect(c.onDragStart).toHaveBeenCalledTimes(1);
    fireEvent.mouseDown(screen.getByTestId('slicer-resize'), { button: 0 });
    expect(c.onDragStart.mock.calls[1]![1]).toBe(true);
    fireEvent.keyDown(screen.getByTestId('slicer'), { key: 'Delete' });
    expect(c.onDelete).toHaveBeenCalledTimes(1);
  });

  it('selects a range with Shift, toggles Multi-Select (button, Alt+S) and clears with Alt+C', () => {
    const onChoose = vi.fn();
    const onClear = vi.fn();
    const info = slicer({
      items: ['A', 'B', 'C', 'D'].map((label, i) => ({ label, selected: i === 0 })),
    });
    ui(<SlicerFrame {...frame()} info={info} onChoose={onChoose} onClear={onClear} />);
    const box = screen.getByTestId('slicer');
    const item = (name: string) => within(box).getByRole('button', { name });
    fireEvent.click(item('B'));
    expect(onChoose).toHaveBeenLastCalledWith(['B']);
    fireEvent.click(item('D'), { shiftKey: true });
    expect(onChoose).toHaveBeenLastCalledWith(['B', 'C', 'D']);
    // Multi-Select on: a plain click adds or removes like Ctrl.
    const multi = screen.getByTestId('slicer-multi');
    expect(multi.getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(multi);
    expect(multi.getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(item('C'));
    expect(onChoose).toHaveBeenLastCalledWith(['A', 'C']);
    fireEvent.keyDown(box, { key: 's', altKey: true });
    expect(multi.getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(item('C'));
    expect(onChoose).toHaveBeenLastCalledWith(['C']);
    fireEvent.keyDown(box, { key: 'c', altKey: true });
    expect(onClear).toHaveBeenCalledTimes(1);
  });

  it('draws a custom style: frame fill and the items that are not chosen', () => {
    const colors = {
      selected: '#2E75B6',
      selectedText: '#FFFFFF',
      header: '#FFFFFF',
      headerText: '#1F4E79',
      border: '#1F4E79',
      background: '#F2F7FC',
      unselected: '#DDEBF7',
      unselectedText: '#595959',
    };
    ui(<SlicerFrame {...frame()} info={slicer({ colors })} onChoose={vi.fn()} onClear={vi.fn()} />);
    const box = screen.getByTestId('slicer');
    expect(box.style.getPropertyValue('--frame-bg')).toBe('#F2F7FC');
    expect(box.style.getPropertyValue('--frame-off')).toBe('#DDEBF7');
    expect(box.style.getPropertyValue('--frame-off-text')).toBe('#595959');
  });

  it('has no editing tools and no clear filter to press when nothing can change', () => {
    const all = slicer({ items: [{ label: 'North', selected: true }] });
    const c = frame({ editable: false, selected: true });
    ui(<SlicerFrame {...c} info={all} onChoose={vi.fn()} onClear={vi.fn()} />);
    expect(screen.queryByTestId('slicer-settings')).toBeNull();
    expect(screen.queryByTestId('slicer-delete')).toBeNull();
    expect(screen.queryByTestId('slicer-resize')).toBeNull();
    expect((screen.getByTestId('slicer-clear') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.mouseDown(screen.getByTestId('slicer-header'), { button: 0 });
    expect(c.onDragStart).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByTestId('slicer'), { key: 'Delete' });
    expect(c.onDelete).not.toHaveBeenCalled();
  });
});

describe('TimelineFrame', () => {
  it('selects a period, extends the range with Shift and shows the selection', () => {
    const onRange = vi.fn();
    ui(
      <TimelineFrame
        {...frame()}
        info={timeline()}
        onRange={onRange}
        onClear={vi.fn()}
        onLevel={vi.fn()}
      />,
    );
    expect(screen.getByTestId('timeline-range').textContent).toBe('02/2024 – 03/2024');
    const strip = screen.getByTestId('timeline-strip');
    expect(
      within(strip)
        .getAllByRole('button', { pressed: true })
        .map((b) => b.textContent),
    ).toEqual(['02/2024', '03/2024']);
    fireEvent.click(within(strip).getByRole('button', { name: '01/2024' }));
    expect(onRange).toHaveBeenLastCalledWith(45292, 45322);
    fireEvent.click(within(strip).getByRole('button', { name: '04/2024' }), { shiftKey: true });
    expect(onRange).toHaveBeenLastCalledWith(45292, 45412);
  });

  it('chooses the periods a drag goes over, committed when the mouse button goes up', () => {
    const onRange = vi.fn();
    ui(
      <TimelineFrame
        {...frame()}
        info={timeline()}
        onRange={onRange}
        onClear={vi.fn()}
        onLevel={vi.fn()}
      />,
    );
    const strip = screen.getByTestId('timeline-strip');
    const period = (name: string) => within(strip).getByRole('button', { name });
    fireEvent.mouseDown(period('01/2024'), { button: 0 });
    fireEvent.mouseEnter(period('02/2024'));
    fireEvent.mouseEnter(period('03/2024'));
    expect(onRange).not.toHaveBeenCalled();
    fireEvent.mouseUp(window);
    expect(onRange).toHaveBeenCalledTimes(1);
    expect(onRange).toHaveBeenLastCalledWith(45292, 45382);
    // Then Shift click extends from where the drag started.
    fireEvent.click(period('04/2024'), { shiftKey: true });
    expect(onRange).toHaveBeenLastCalledWith(45292, 45412);
  });

  it('moves an end of the selection by dragging its handle', () => {
    const onRange = vi.fn();
    ui(
      <TimelineFrame
        {...frame()}
        info={timeline()}
        onRange={onRange}
        onClear={vi.fn()}
        onLevel={vi.fn()}
      />,
    );
    const strip = screen.getByTestId('timeline-strip');
    const period = (name: string) => within(strip).getByRole('button', { name });
    // February … March chosen: the start handle on February, the end handle on March.
    expect(within(period('02/2024')).getByTestId('timeline-edge-start')).toBeTruthy();
    expect(within(period('03/2024')).getByTestId('timeline-edge-end')).toBeTruthy();
    // The end handle dragged to April: February … April (the start stays).
    fireEvent.mouseDown(screen.getByTestId('timeline-edge-end'), { button: 0 });
    expect(screen.queryByTestId('timeline-edge-end')).toBeNull();
    fireEvent.mouseEnter(period('04/2024'));
    fireEvent.mouseUp(window);
    expect(onRange).toHaveBeenLastCalledWith(45323, 45412);
    // The start handle dragged to January: January … March.
    fireEvent.mouseDown(screen.getByTestId('timeline-edge-start'), { button: 0 });
    fireEvent.mouseEnter(period('01/2024'));
    fireEvent.mouseUp(window);
    expect(onRange).toHaveBeenLastCalledWith(45292, 45382);
    expect(onRange).toHaveBeenCalledTimes(2);
  });

  it('has no selection handles without a filter or when it cannot change', () => {
    const { rerender } = ui(
      <TimelineFrame
        {...frame()}
        info={timeline({ filtered: false })}
        onRange={vi.fn()}
        onClear={vi.fn()}
        onLevel={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('timeline-edge-start')).toBeNull();
    rerender(
      <I18nextProvider i18n={i18n}>
        <TimelineFrame
          {...frame({ editable: false })}
          info={timeline()}
          onRange={vi.fn()}
          onClear={vi.fn()}
          onLevel={vi.fn()}
        />
      </I18nextProvider>,
    );
    expect(screen.queryByTestId('timeline-edge-end')).toBeNull();
  });

  it('hides the parts Timeline Tools › Show turns off', () => {
    const c = frame();
    const { rerender } = ui(
      <TimelineFrame
        {...c}
        info={timeline({ showTimeLevel: false, showScrollbar: false, showSelectionLabel: false })}
        onRange={vi.fn()}
        onClear={vi.fn()}
        onLevel={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('timeline-level')).toBeNull();
    expect(screen.queryByTestId('timeline-zoom-in')).toBeNull();
    expect(screen.queryByTestId('timeline-range')).toBeNull();
    expect(screen.getByTestId('timeline-strip').getAttribute('data-scrollbar')).toBe('hidden');
    expect(screen.getByTestId('timeline-header').textContent).toContain('Date');
    // No header: no caption and no Clear Filter; the frame moves by its edge.
    rerender(
      <I18nextProvider i18n={i18n}>
        <TimelineFrame
          {...c}
          info={timeline({ showHeader: false })}
          onRange={vi.fn()}
          onClear={vi.fn()}
          onLevel={vi.fn()}
        />
      </I18nextProvider>,
    );
    expect(screen.queryByTestId('timeline-header')).toBeNull();
    expect(screen.queryByTestId('timeline-clear')).toBeNull();
    expect(screen.getByTestId('timeline-level')).toBeTruthy();
    expect(screen.getByTestId('timeline-settings')).toBeTruthy();
    fireEvent.mouseDown(screen.getByTestId('timeline'), { button: 0 });
    expect(c.onDragStart).toHaveBeenCalledTimes(1);
  });

  it('changes the time level from the list and the zoom buttons', () => {
    const onLevel = vi.fn();
    const onClear = vi.fn();
    ui(
      <TimelineFrame
        {...frame()}
        info={timeline()}
        onRange={vi.fn()}
        onClear={onClear}
        onLevel={onLevel}
      />,
    );
    const level = screen.getByTestId('timeline-level') as HTMLSelectElement;
    expect(level.value).toBe('2');
    expect(Array.from(level.options).map((o) => o.textContent)).toEqual([
      'Years',
      'Quarters',
      'Months',
      'Days',
    ]);
    fireEvent.change(level, { target: { value: '0' } });
    expect(onLevel).toHaveBeenLastCalledWith(0);
    fireEvent.click(screen.getByTestId('timeline-zoom-in'));
    expect(onLevel).toHaveBeenLastCalledWith(3);
    fireEvent.click(screen.getByTestId('timeline-zoom-out'));
    expect(onLevel).toHaveBeenLastCalledWith(1);
    fireEvent.click(screen.getByTestId('timeline-clear'));
    expect(onClear).toHaveBeenCalledTimes(1);
  });

  it('stops zooming at years and days', () => {
    const { rerender } = ui(
      <TimelineFrame
        {...frame()}
        info={timeline({ level: 0, filtered: false })}
        onRange={vi.fn()}
        onClear={vi.fn()}
        onLevel={vi.fn()}
      />,
    );
    expect((screen.getByTestId('timeline-zoom-out') as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId('timeline-clear') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('timeline-range').textContent).toBe('All periods');
    rerender(
      <I18nextProvider i18n={i18n}>
        <TimelineFrame
          {...frame()}
          info={timeline({ level: 3 })}
          onRange={vi.fn()}
          onClear={vi.fn()}
          onLevel={vi.fn()}
        />
      </I18nextProvider>,
    );
    expect((screen.getByTestId('timeline-zoom-in') as HTMLButtonElement).disabled).toBe(true);
  });
});

describe('InsertFramesDialog', () => {
  it('inserts a slicer for every ticked field', async () => {
    const onInsert = vi.fn(async () => {});
    const onClose = vi.fn();
    ui(<InsertFramesDialog pivot={pivot()} kind="slicer" onInsert={onInsert} onClose={onClose} />);
    // Nothing ticked: a message, nothing inserted.
    fireEvent.click(screen.getByTestId('frame-insert-ok'));
    expect(screen.getByRole('alert').textContent).toBe('Choose at least one field.');
    expect(onInsert).not.toHaveBeenCalled();
    expect(screen.queryByTestId('frame-field-4')).toBeNull();
    fireEvent.click(screen.getByTestId('frame-field-6'));
    fireEvent.click(screen.getByTestId('frame-field-0'));
    fireEvent.click(screen.getByTestId('frame-insert-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(onInsert).toHaveBeenCalledWith([0, 6]);
  });

  it('offers only date fields for a timeline and shows the engine message', async () => {
    const onInsert = vi.fn(async () => {
      throw new Error('a timeline needs a date field');
    });
    const onClose = vi.fn();
    ui(
      <InsertFramesDialog pivot={pivot()} kind="timeline" onInsert={onInsert} onClose={onClose} />,
    );
    expect(screen.getAllByRole('checkbox')).toHaveLength(1);
    fireEvent.click(screen.getByTestId('frame-field-2'));
    fireEvent.click(screen.getByTestId('frame-insert-ok'));
    await waitFor(() =>
      expect(screen.getByRole('alert').textContent).toBe('a timeline needs a date field'),
    );
    expect(onClose).not.toHaveBeenCalled();
  });

  it('says so when no field fits', () => {
    const none = pivot({ fields: [field('Amount')] });
    ui(<InsertFramesDialog pivot={none} kind="timeline" onInsert={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByText('The pivot table has no date field.')).toBeTruthy();
  });
});

describe('FrameOptionsDialog', () => {
  it('applies caption, columns, style and the report connections', async () => {
    const onApply = vi.fn(async () => {});
    const onClose = vi.fn();
    const s = slicer();
    ui(
      <FrameOptionsDialog
        kind="slicer"
        caption={s.caption}
        columns={s.columns}
        style={s.style ?? null}
        connections={s.pivots ?? []}
        onApply={onApply}
        onClose={onClose}
      />,
    );
    fireEvent.change(screen.getByTestId('frame-caption'), { target: { value: 'Area' } });
    fireEvent.change(screen.getByTestId('frame-columns'), { target: { value: '0' } });
    fireEvent.click(screen.getByTestId('frame-options-ok'));
    expect(screen.getByRole('alert').textContent).toBe(
      'The number of columns must be between 1 and 50.',
    );
    fireEvent.change(screen.getByTestId('frame-columns'), { target: { value: '3' } });
    fireEvent.change(screen.getByTestId('frame-style'), {
      target: { value: 'SlicerStyleLight4' },
    });
    // No pivot table left: refused; then both connected.
    fireEvent.click(screen.getByTestId('frame-connection-Pivot1'));
    fireEvent.click(screen.getByTestId('frame-options-ok'));
    expect(screen.getByRole('alert').textContent).toBe('Connect at least one pivot table.');
    expect(onApply).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('frame-connection-Pivot1'));
    fireEvent.click(screen.getByTestId('frame-connection-Pivot2'));
    fireEvent.click(screen.getByTestId('frame-options-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(onApply).toHaveBeenCalledWith({
      caption: 'Area',
      columns: 3,
      style: 'SlicerStyleLight4',
      pivots: ['Pivot1', 'Pivot2'],
    });
  });

  it('lists the workbook styles first and applies the Show options of a timeline', async () => {
    const onApply = vi.fn(async () => {});
    ui(
      <FrameOptionsDialog
        kind="timeline"
        caption="Date"
        style="Mine"
        connections={[]}
        customStyles={['Mine']}
        show={{ header: true, selectionLabel: true, timeLevel: true, scrollbar: true }}
        onApply={onApply}
        onClose={vi.fn()}
      />,
    );
    const style = screen.getByTestId('frame-style') as HTMLSelectElement;
    expect(style.value).toBe('Mine');
    // The custom style, then the twelve built-in ones (no "(none)" entry).
    expect(style.options).toHaveLength(13);
    expect(style.options[0]!.textContent).toBe('Mine');
    fireEvent.click(screen.getByTestId('timeline-show-timeLevel'));
    fireEvent.click(screen.getByTestId('timeline-show-scrollbar'));
    fireEvent.click(screen.getByTestId('frame-options-ok'));
    await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
    expect(onApply).toHaveBeenCalledWith({
      caption: 'Date',
      columns: undefined,
      style: 'Mine',
      pivots: undefined,
      show: { header: true, selectionLabel: true, timeLevel: false, scrollbar: false },
    });
  });

  it('creates a new slicer style with a format per element and offers it', async () => {
    const onCreateStyle = vi.fn(async () => {});
    const onApply = vi.fn(async () => {});
    const s = slicer();
    ui(
      <FrameOptionsDialog
        kind="slicer"
        caption={s.caption}
        columns={s.columns}
        style={s.style ?? null}
        connections={s.pivots ?? []}
        customStyles={['Slicer style 1']}
        colors={s.colors}
        onCreateStyle={onCreateStyle}
        onApply={onApply}
        onClose={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('frame-style-new'));
    const dlg = screen.getByTestId('frame-style-dialog');
    // The first free default name.
    expect((within(dlg).getByTestId('frame-style-name') as HTMLInputElement).value).toBe(
      'Slicer style 2',
    );
    // Header: bold; selected items: a red fill and white text.
    fireEvent.click(within(dlg).getByTestId('frame-style-element-header'));
    fireEvent.click(within(dlg).getByTestId('frame-style-bold'));
    fireEvent.click(within(dlg).getByTestId('frame-style-element-selected'));
    expect((within(dlg).getByTestId('frame-style-bold') as HTMLInputElement).checked).toBe(false);
    fireEvent.click(within(dlg).getByTestId('frame-style-use-fill'));
    fireEvent.change(within(dlg).getByTestId('frame-style-fill'), {
      target: { value: '#c00000' },
    });
    fireEvent.click(within(dlg).getByTestId('frame-style-use-text'));
    fireEvent.change(within(dlg).getByTestId('frame-style-text'), {
      target: { value: '#ffffff' },
    });
    // The preview shows the chosen item in the new colours.
    const preview = within(dlg).getByTestId('frame-style-preview');
    expect(preview.style.getPropertyValue('--frame-on')).toBe('#C00000');
    expect(preview.style.getPropertyValue('--frame-on-text')).toBe('#FFFFFF');
    // A format put on and cleared again is not saved.
    fireEvent.click(within(dlg).getByTestId('frame-style-element-whole'));
    fireEvent.click(within(dlg).getByTestId('frame-style-use-border'));
    fireEvent.click(within(dlg).getByTestId('frame-style-clear'));
    fireEvent.click(within(dlg).getByTestId('frame-style-ok'));
    await waitFor(() => expect(onCreateStyle).toHaveBeenCalledTimes(1));
    expect(onCreateStyle).toHaveBeenCalledWith({
      name: 'Slicer style 2',
      formats: {
        header: { bold: true },
        selected: { fill: '#C00000', text: '#FFFFFF' },
      },
    });
    await waitFor(() => expect(screen.queryByTestId('frame-style-dialog')).toBeNull());
    // The new style is the choice; OK applies it.
    const style = screen.getByTestId('frame-style') as HTMLSelectElement;
    expect(style.value).toBe('Slicer style 2');
    fireEvent.click(screen.getByTestId('frame-options-ok'));
    await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({ style: 'Slicer style 2' }));
  });

  it('asks for a name and shows the engine message for a name in use', async () => {
    const onCreateStyle = vi.fn(async () => {
      throw new Error('style name in use');
    });
    ui(
      <FrameOptionsDialog
        kind="timeline"
        caption="Date"
        style={null}
        connections={[]}
        onCreateStyle={onCreateStyle}
        onApply={vi.fn(async () => {})}
        onClose={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId('frame-style-new'));
    const dlg = screen.getByTestId('frame-style-dialog');
    expect(within(dlg).getByText('Selected time block')).toBeTruthy();
    fireEvent.change(within(dlg).getByTestId('frame-style-name'), { target: { value: '  ' } });
    fireEvent.click(within(dlg).getByTestId('frame-style-ok'));
    expect(within(dlg).getByRole('alert').textContent).toBe('Enter a style name.');
    expect(onCreateStyle).not.toHaveBeenCalled();
    fireEvent.change(within(dlg).getByTestId('frame-style-name'), { target: { value: 'Mine' } });
    fireEvent.click(within(dlg).getByTestId('frame-style-ok'));
    await waitFor(() =>
      expect(within(dlg).getByRole('alert').textContent).toBe('style name in use'),
    );
    expect(screen.getByTestId('frame-style-dialog')).toBeTruthy();
  });

  it('has no columns and no connections for a table slicer or where they do not apply', async () => {
    const onApply = vi.fn(async () => {});
    ui(
      <FrameOptionsDialog
        kind="timeline"
        caption="Date"
        style={null}
        connections={[]}
        onApply={onApply}
        onClose={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('frame-columns')).toBeNull();
    expect(screen.queryByRole('group')).toBeNull();
    const style = screen.getByTestId('frame-style') as HTMLSelectElement;
    // No style yet: "(none)" is listed first, then the twelve timeline styles.
    expect(style.options).toHaveLength(13);
    expect(style.options[0]!.textContent).toBe('(none)');
    fireEvent.change(style, { target: { value: 'TimeSlicerStyleDark1' } });
    fireEvent.click(screen.getByTestId('frame-options-ok'));
    await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
    expect(onApply).toHaveBeenCalledWith({
      caption: 'Date',
      columns: undefined,
      style: 'TimeSlicerStyleDark1',
      pivots: undefined,
    });
  });
});

describe('PivotChartFilters', () => {
  const rect = { x: 40, y: 60, w: 300, h: 200 };

  it('shows one button per field and hides the unticked items', async () => {
    const onApply = vi.fn(async () => {});
    ui(<PivotChartFilters pivot={pivot()} rect={rect} onApply={onApply} />);
    const bar = screen.getByTestId('pivot-chart-filters');
    expect(bar.style.left).toBe('44px');
    expect(
      within(bar)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['Channel', 'Region', 'Kind']);
    expect(screen.getByTestId('pivot-chart-filter-0').getAttribute('data-area')).toBe('axis');
    // Region: "North" is hidden already; hide "West" too.
    fireEvent.click(screen.getByTestId('pivot-chart-filter-0'));
    const menu = screen.getByTestId('pivot-chart-filter-menu');
    const boxes = within(menu).getAllByRole('checkbox') as HTMLInputElement[];
    expect(boxes.map((b) => b.checked)).toEqual([false, false, true, true]);
    fireEvent.click(boxes[3]!);
    fireEvent.click(screen.getByTestId('pivot-chart-filter-ok'));
    await waitFor(() => expect(onApply).toHaveBeenCalledWith(0, [0, 2]));
    await waitFor(() => expect(screen.queryByTestId('pivot-chart-filter-menu')).toBeNull());
  });

  it('keeps at least one item and can show all again', async () => {
    const onApply = vi.fn(async () => {});
    ui(<PivotChartFilters pivot={pivot()} rect={rect} onApply={onApply} />);
    fireEvent.click(screen.getByTestId('pivot-chart-filter-1'));
    const all = screen.getByTestId('pivot-chart-filter-all') as HTMLInputElement;
    expect(all.checked).toBe(true);
    fireEvent.click(all);
    fireEvent.click(screen.getByTestId('pivot-chart-filter-ok'));
    expect(screen.getByRole('alert').textContent).toBe('Keep at least one item.');
    expect(onApply).not.toHaveBeenCalled();
    fireEvent.click(all);
    fireEvent.click(screen.getByTestId('pivot-chart-filter-ok'));
    await waitFor(() => expect(onApply).toHaveBeenCalledWith(1, []));
  });

  it('shows the engine message and draws nothing without fields', async () => {
    const onApply = vi.fn(async () => {
      throw new Error('pivot source not found');
    });
    const { unmount } = ui(<PivotChartFilters pivot={pivot()} rect={rect} onApply={onApply} />);
    fireEvent.click(screen.getByTestId('pivot-chart-filter-6'));
    fireEvent.click(screen.getByTestId('pivot-chart-filter-ok'));
    await waitFor(() =>
      expect(screen.getByRole('alert').textContent).toBe('pivot source not found'),
    );
    unmount();
    ui(
      <PivotChartFilters
        pivot={pivot({ rows: [], cols: [], filters: [] })}
        rect={rect}
        onApply={onApply}
      />,
    );
    expect(screen.queryByTestId('pivot-chart-filters')).toBeNull();
  });
});
