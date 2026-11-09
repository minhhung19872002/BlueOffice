// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, ReviewState } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';
import { ReviewingPane } from '../src/ReviewingPane';
import type { EditorState } from '../src/store';
import {
  balloonLabel,
  LockTrackingDialog,
  registerTrackingCommands,
  reviewErrorKey,
  revisionDetail,
} from '../src/TrackingControls';

afterEach(cleanup);

const i18n = createI18n('en-US');
const vi18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);
const t = (k: string, o?: { defaultValue: string }) => i18n.t(k, o);

const review = (over: Partial<ReviewState> = {}): ReviewState => ({
  trackChanges: true,
  author: 'An',
  view: 'markup',
  revisions: [
    {
      id: 3,
      kind: 'format',
      author: 'An',
      date: null,
      start: { p: 0, o: 0 },
      end: { p: 0, o: 3 },
      text: 'Một',
      detail: ['bold', 'italic'],
    },
  ],
  comments: [],
  ...over,
});

describe('formatting revision details', () => {
  it('names the changed properties in both languages', () => {
    expect(revisionDetail(t, { detail: ['bold', 'italic'] })).toBe('Bold, Italic');
    expect(revisionDetail((k, o) => vi18n.t(k, o), { detail: ['align', 'indent'] })).toBe(
      'Căn lề, Thụt lề',
    );
    expect(revisionDetail(t, {})).toBe('');
  });

  it('adds them to the balloon label', () => {
    expect(balloonLabel(t, 'format', { detail: ['bold'] })).toBe('Formatting (Bold)');
    expect(balloonLabel(t, 'insert', undefined)).toBe(i18n.t('markup.kind.insert'));
  });

  it('shows them in the change list', () => {
    wrap(
      <ReviewingPane
        review={review()}
        run={vi.fn()}
        language="en-US"
        dock="left"
        onDock={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId('reviewing-detail').textContent).toBe('Bold, Italic');
  });

  it('maps the locked error', () => {
    expect(reviewErrorKey('review:locked')).toBe('review.lock.lockedError');
    expect(reviewErrorKey('other')).toBeNull();
    expect(i18n.exists('review.lock.lockedError')).toBe(true);
  });
});

describe('tracking commands', () => {
  it('moves paragraphs and opens the lock dialog', async () => {
    const r = new CommandRegistry<EditorState>();
    const run = vi.fn(async (_c: EditCommand) => {});
    const open = vi.fn();
    registerTrackingCommands(r, { hasDoc: () => true, run, openLockDialog: open });
    const s = { review: review({ trackingLocked: true }), edit: null } as unknown as EditorState;
    await r.get('review.moveParagraphUp')?.run(s);
    await r.get('review.moveParagraphDown')?.run(s);
    expect(run.mock.calls.map((c) => c[0])).toEqual([
      { cmd: 'moveParagraphs', up: true },
      { cmd: 'moveParagraphs', up: false },
    ]);
    expect(r.isChecked('review.lockTracking', s)).toBe(true);
    await r.get('review.lockTracking')?.run(s);
    expect(open).toHaveBeenCalledOnce();
  });

  it('locks with a confirmed password', () => {
    const apply = vi.fn();
    wrap(<LockTrackingDialog review={review()} onApply={apply} onClose={() => {}} />);
    fireEvent.change(screen.getByTestId('lock-tracking-password'), { target: { value: 'pw' } });
    expect(screen.getByTestId('lock-tracking-ok')).toHaveProperty('disabled', true);
    fireEvent.change(screen.getByTestId('lock-tracking-confirm'), { target: { value: 'pw' } });
    fireEvent.click(screen.getByTestId('lock-tracking-ok'));
    expect(apply).toHaveBeenCalledWith({
      cmd: 'protectDocument',
      edit: 'trackedChanges',
      password: 'pw',
    });
  });

  it('unlocks with the password', () => {
    const apply = vi.fn();
    wrap(
      <LockTrackingDialog
        review={review({ trackingLocked: true, lockPassword: true })}
        onApply={apply}
        onClose={() => {}}
      />,
    );
    expect(screen.queryByTestId('lock-tracking-confirm')).toBeNull();
    fireEvent.change(screen.getByTestId('lock-tracking-password'), { target: { value: 'pw' } });
    fireEvent.click(screen.getByTestId('lock-tracking-ok'));
    expect(apply).toHaveBeenCalledWith({ cmd: 'unprotectDocument', password: 'pw' });
  });
});
