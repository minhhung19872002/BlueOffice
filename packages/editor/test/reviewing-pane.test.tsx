// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, ReviewState } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MarkupModeSelect, reviewersOf, ShowMarkupMenu } from '../src/MarkupMenus';
import { markKindOf, reviewCounts, reviewEntries, ReviewingPane } from '../src/ReviewingPane';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const review: ReviewState = {
  trackChanges: true,
  author: 'An',
  view: 'markup',
  markup: {
    mode: 'all',
    insertionsDeletions: true,
    formatting: true,
    comments: true,
    hiddenReviewers: [],
    deletionBalloons: false,
  },
  revisions: [
    {
      id: 2,
      kind: 'insert',
      author: 'Bình',
      date: null,
      start: { p: 1, o: 0 },
      end: { p: 1, o: 4 },
      text: 'mới',
    },
    {
      id: 1,
      kind: 'delete',
      author: 'An',
      date: null,
      start: { p: 0, o: 3 },
      end: { p: 0, o: 6 },
      text: 'cũ',
    },
    {
      id: 5,
      kind: 'paragraphFormat',
      author: 'An',
      date: null,
      start: { p: 2, o: 0 },
      end: { p: 2, o: 0 },
      text: '',
    },
  ],
  comments: [
    {
      id: 9,
      author: 'Chi',
      initials: null,
      date: null,
      text: 'Xem lại',
      done: false,
      parent: null,
      start: { p: 0, o: 0 },
      end: { p: 0, o: 2 },
      quote: 'Hợ',
    },
    {
      id: 10,
      author: 'An',
      initials: null,
      date: null,
      text: 'Đã sửa',
      done: false,
      parent: 9,
      start: null,
      end: null,
      quote: '',
    },
  ],
};

describe('reviewing pane', () => {
  it('lists changes and comment threads in document order with counts by type', () => {
    const entries = reviewEntries(review);
    expect(entries.map((e) => `${e.type}:${e.item.id}`)).toEqual([
      'comment:9',
      'revision:1',
      'revision:2',
      'revision:5',
    ]);
    expect(reviewCounts(entries)).toEqual({ insert: 1, delete: 1, format: 1, comment: 1 });
    expect(markKindOf('moveFrom')).toBe('delete');
    expect(markKindOf('cellInsert')).toBe('insert');
    expect(markKindOf('tableGrid')).toBe('format');
  });

  it('navigates, accepts and rejects from the pane, and switches its dock', () => {
    const run = vi.fn((_: EditCommand) => Promise.resolve(null));
    const onDock = vi.fn();
    wrap(
      <ReviewingPane
        review={review}
        run={run}
        language="en-US"
        dock="left"
        onDock={onDock}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId('reviewing-pane').dataset.dock).toBe('left');
    expect(screen.getByTestId('reviewing-summary').textContent).toBe('4 items');
    expect(screen.getByTestId('reviewing-count-comment').textContent).toBe('Comments: 1');
    const items = screen.getAllByTestId('reviewing-item');
    expect(items.map((i) => i.dataset.kind)).toEqual(['comment', 'delete', 'insert', 'format']);
    // The comment thread shows its reply count.
    expect(items[0]!.textContent).toContain('Xem lại (+1)');
    fireEvent.click(items[0]!.querySelector('button')!);
    expect(run).toHaveBeenLastCalledWith({ cmd: 'selectComment', id: 9 });
    fireEvent.click(items[2]!.querySelector('button')!);
    expect(run).toHaveBeenLastCalledWith({
      cmd: 'setSelection',
      selection: { anchor: { p: 1, o: 0 }, focus: { p: 1, o: 4 } },
    });
    fireEvent.click(screen.getAllByTestId('reviewing-accept')[0]!);
    expect(run).toHaveBeenLastCalledWith({ cmd: 'acceptRevision', id: 1 });
    fireEvent.click(screen.getAllByTestId('reviewing-reject')[1]!);
    expect(run).toHaveBeenLastCalledWith({ cmd: 'rejectRevision', id: 2 });
    // Comments have no accept/reject buttons.
    expect(screen.getAllByTestId('reviewing-accept')).toHaveLength(3);
    fireEvent.click(screen.getByTestId('reviewing-dock'));
    expect(onDock).toHaveBeenCalledWith('bottom');
  });

  it('says when there is nothing to review', () => {
    wrap(
      <ReviewingPane
        review={{ ...review, revisions: [], comments: [] }}
        run={() => Promise.resolve(null)}
        language="en-US"
        dock="bottom"
        onDock={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId('reviewing-summary').textContent).toBe('0 items');
    expect(screen.queryAllByTestId('reviewing-item')).toHaveLength(0);
  });
});

describe('markup menus', () => {
  const entry = (id: string, checked: boolean, onSelect = () => {}) => ({
    id,
    label: id,
    checked,
    enabled: true,
    onSelect,
  });

  it('mode dropdown shows the checked mode and runs the chosen one', () => {
    const simple = vi.fn();
    wrap(
      <MarkupModeSelect
        disabled={false}
        entries={[entry('review.markup.simple', false, simple), entry('review.markup.all', true)]}
      />,
    );
    const sel = screen.getByTestId('markup-mode') as HTMLSelectElement;
    expect(sel.value).toBe('review.markup.all');
    fireEvent.change(sel, { target: { value: 'review.markup.simple' } });
    expect(simple).toHaveBeenCalledOnce();
  });

  it('show-markup menu toggles marks and reviewers', () => {
    const comments = vi.fn();
    const onReviewers = vi.fn();
    wrap(
      <ShowMarkupMenu
        disabled={false}
        entries={[
          entry('review.markup.insDel', true),
          entry('review.markup.comments', true, comments),
        ]}
        reviewers={reviewersOf(review)}
        hidden={['Chi']}
        onReviewers={onReviewers}
      />,
    );
    fireEvent.click(screen.getByTestId('show-markup'));
    const box = document.querySelector<HTMLInputElement>(
      '[data-command="review.markup.comments"] input',
    )!;
    expect(box.checked).toBe(true);
    fireEvent.click(box);
    expect(comments).toHaveBeenCalledOnce();
    const people = screen.getAllByTestId('markup-reviewer') as HTMLInputElement[];
    expect(people.map((p) => p.dataset.reviewer)).toEqual(['An', 'Bình', 'Chi']);
    expect(people.map((p) => p.checked)).toEqual([true, true, false]);
    // Showing Chi again; hiding An.
    fireEvent.click(people[2]!);
    expect(onReviewers).toHaveBeenLastCalledWith([]);
    fireEvent.click(people[0]!);
    expect(onReviewers).toHaveBeenLastCalledWith(['Chi', 'An']);
    // "Everyone" is unchecked while someone is hidden; it shows them all.
    const all = screen.getByTestId('markup-reviewer-all') as HTMLInputElement;
    expect(all.checked).toBe(false);
    fireEvent.click(all);
    expect(onReviewers).toHaveBeenLastCalledWith([]);
  });
});
