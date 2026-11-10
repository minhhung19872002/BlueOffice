// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  CommentThreads,
  MentionComposer,
  insertMention,
  mentionPieces,
  mentionQuery,
  shiftMentions,
  type CommentThreadsLabels,
  type MentionComposerLabels,
  type MentionSubmit,
  type MentionSuggestion,
} from '../src';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const labels: MentionComposerLabels = {
  placeholder: 'Comment',
  submit: 'Send',
  assignTo: 'Assign to',
  noMatches: 'Nobody',
  people: 'People',
};

const binh: MentionSuggestion = { id: 'u2', name: 'Trần Thị Bình', email: 'binh@drive.mock' };
const cuong: MentionSuggestion = { id: 'u3', name: 'Lê Văn Cường', email: 'cuong@drive.mock' };

describe('mention helpers', () => {
  it('finds the @query before the caret', () => {
    expect(mentionQuery('Gửi @Bì', 7)).toEqual({ at: 4, query: 'Bì' });
    expect(mentionQuery('@', 1)).toEqual({ at: 0, query: '' });
    // Not at a word start (an e-mail address), or a blank after the query.
    expect(mentionQuery('an@x', 4)).toBeNull();
    expect(mentionQuery('@An xem', 7)).toBeNull();
  });

  it('inserts a mention and keeps the others anchored', () => {
    const first = insertMention({ text: '@Bì xem', mentions: [] }, 0, 3, binh);
    expect(first.text).toBe('@Trần Thị Bình  xem');
    expect(first.mentions).toEqual([{ ...binh, start: 0, length: 14 }]);
    expect(first.caret).toBe(15);
    // A second one before it shifts the first.
    const text = `@L ${first.text}`;
    const shifted = shiftMentions(first.text, text, first.mentions, 3);
    expect(shifted[0]?.start).toBe(3);
    const second = insertMention({ text, mentions: shifted }, 0, 2, cuong);
    expect(
      second.mentions.map((m) => [m.id, second.text.slice(m.start, m.start + m.length)]),
    ).toEqual([
      ['u3', '@Lê Văn Cường'],
      ['u2', '@Trần Thị Bình'],
    ]);
  });

  it('drops a mention whose name was edited', () => {
    const d = insertMention({ text: '@B', mentions: [] }, 0, 2, binh);
    const edited = d.text.replace('Bình', 'Bin');
    expect(shiftMentions(d.text, edited, d.mentions)).toEqual([]);
  });

  it('splits text into mention pieces', () => {
    const p = mentionPieces('Xem @An nhé', [{ id: 'u1', name: 'An', start: 4, length: 3 }]);
    expect(p.map((x) => [x.text, !!x.mention])).toEqual([
      ['Xem ', false],
      ['@An', true],
      [' nhé', false],
    ]);
  });
});

describe('MentionComposer', () => {
  it('opens the picker on @, picks with the keyboard and assigns a task', async () => {
    vi.useFakeTimers();
    const search = vi.fn(async (q: string) =>
      [binh, cuong].filter((p) => p.name.toLowerCase().includes(q.toLowerCase())),
    );
    const onSubmit = vi.fn<(v: MentionSubmit) => void>();
    render(<MentionComposer labels={labels} search={search} onSubmit={onSubmit} testId="box" />);
    const box = screen.getByTestId('box') as HTMLTextAreaElement;
    fireEvent.change(box, { target: { value: 'Nhờ @', selectionStart: 5 } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(search).toHaveBeenLastCalledWith('');
    expect(screen.getAllByTestId('mention-option')).toHaveLength(2);
    fireEvent.keyDown(box, { key: 'ArrowDown' });
    fireEvent.keyDown(box, { key: 'Enter' });
    expect(box.value).toBe('Nhờ @Lê Văn Cường ');
    expect(screen.queryByTestId('mention-picker')).toBeNull();

    // The assign checkbox appears once someone is mentioned.
    fireEvent.click(screen.getByTestId('mention-assign'));
    fireEvent.change(box, { target: { value: `${box.value}kiểm tra`, selectionStart: 26 } });
    fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true });
    expect(onSubmit).toHaveBeenCalledWith({
      text: 'Nhờ @Lê Văn Cường kiểm tra',
      mentions: [{ ...cuong, start: 4, length: 13 }],
      assignee: cuong,
    });
    expect(box.value).toBe('');
  });

  it('filters by the typed name and shows when nobody matches', async () => {
    vi.useFakeTimers();
    const search = vi.fn(async (q: string) => [binh, cuong].filter((p) => p.name.includes(q)));
    render(<MentionComposer labels={labels} search={search} onSubmit={() => {}} testId="box" />);
    const box = screen.getByTestId('box');
    fireEvent.change(box, { target: { value: '@Bì', selectionStart: 3 } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(screen.getAllByTestId('mention-option').map((o) => o.textContent)).toEqual([
      'Trần Thị Bìnhbinh@drive.mock',
    ]);
    // Clicking an option picks it.
    fireEvent.mouseDown(screen.getByTestId('mention-option'));
    expect((box as HTMLTextAreaElement).value).toBe('@Trần Thị Bình ');
    fireEvent.change(box, { target: { value: '@Zz', selectionStart: 3 } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(screen.getByTestId('mention-picker').textContent).toBe('Nobody');
    // Escape closes the picker first.
    fireEvent.keyDown(box, { key: 'Escape' });
    expect(screen.queryByTestId('mention-picker')).toBeNull();
  });

  it('has no picker without a people directory', async () => {
    vi.useFakeTimers();
    const onSubmit = vi.fn();
    render(<MentionComposer labels={labels} onSubmit={onSubmit} multiline={false} testId="box" />);
    const box = screen.getByTestId('box');
    fireEvent.change(box, { target: { value: '@An', selectionStart: 3 } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(screen.queryByTestId('mention-picker')).toBeNull();
    fireEvent.keyDown(box, { key: 'Enter' });
    expect(onSubmit).toHaveBeenCalledWith({ text: '@An', mentions: [], assignee: null });
  });
});

describe('CommentThreads with mentions', () => {
  const threadLabels: CommentThreadsLabels = {
    title: 'Comments',
    empty: 'None',
    newComment: 'New',
    newPlaceholder: 'Write',
    reply: 'Reply',
    replyPlaceholder: 'Reply…',
    resolve: 'Resolve',
    reopen: 'Reopen',
    delete: 'Delete',
    close: 'Close',
    resolved: 'resolved',
    assignTo: 'Assign to',
    noMatches: 'Nobody',
    people: 'People',
    assignedTo: 'Assigned to {name}',
    taskDone: 'Assigned to {name} · done',
  };
  const thread = (done: boolean) => ({
    id: 't1',
    anchor: 'B2',
    done,
    comments: [
      {
        id: 'c1',
        author: 'An',
        date: '',
        text: 'Xem @Trần Thị Bình',
        mentions: [{ ...binh, start: 4, length: 14 }],
        task: { assignee: { name: 'Trần Thị Bình' } },
      },
    ],
  });

  it('highlights mentions and shows the task; resolving completes it', () => {
    const props = {
      labels: threadLabels,
      newAnchor: null,
      onAdd: () => {},
      onReply: () => {},
      onResolve: () => {},
      onDelete: () => {},
      onClose: () => {},
    };
    const { rerender } = render(<CommentThreads {...props} threads={[thread(false)]} />);
    expect(screen.getByTestId('comment-mention').textContent).toBe('@Trần Thị Bình');
    expect(screen.getByTestId('comment-task').textContent).toBe('Assigned to Trần Thị Bình');
    rerender(<CommentThreads {...props} threads={[thread(true)]} />);
    const task = screen.getByTestId('comment-task');
    expect(task.textContent).toBe('Assigned to Trần Thị Bình · done');
    expect(task.dataset.done).toBe('true');
  });
});
