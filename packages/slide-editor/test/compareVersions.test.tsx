// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { DeckChange, EngineHost, SlideCommand } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SlideVersionsPane, deckChangeRows } from '../src/compareVersions';

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = i18n.t.bind(i18n) as (k: string, o?: Record<string, unknown>) => string;

const changes: DeckChange[] = [
  { kind: 'slideRemoved', oldSlide: 2 },
  { kind: 'textChanged', slide: 0, oldSlide: 0, shape: 'Title 1', shapeId: 2, old: 'A', new: 'B' },
  { kind: 'shapeRemoved', slide: 0, oldSlide: 0, shape: 'Logo', shapeId: 5 },
];

describe('BlueSlide compare versions', () => {
  it('groups changes by slide', () => {
    expect(deckChangeRows(changes, t).map((r) => [r.group, r.kind, r.where])).toEqual([
      ['Removed slide (was 3)', 'Slide removed', ''],
      ['Slide 1', 'Text changed', 'Title 1'],
      ['Slide 1', 'Shape removed', 'Logo'],
    ]);
  });

  it('compares with a file and shows the changed shape', async () => {
    const sent: SlideCommand[] = [];
    const engine = {
      slideCommand: vi.fn(async (c: SlideCommand) => {
        sent.push(c);
        return { deckChanges: changes, total: 3, truncated: false };
      }),
    } as unknown as EngineHost;
    const onShow = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <SlideVersionsPane
          engine={engine}
          openFile={async () => ({ name: 'old.pptx', bytes: new Uint8Array([1, 2]) })}
          onShow={onShow}
          onClose={() => {}}
        />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('versions-compare-file'));
    await waitFor(() => expect(screen.getAllByTestId('change-item')).toHaveLength(3));
    expect(sent[0]).toEqual({ cmd: 'compareDeck', name: 'old.pptx', base64: 'AQI=' });
    fireEvent.click(screen.getAllByTestId('change-item')[1]!);
    expect(onShow).toHaveBeenLastCalledWith(0, 2);
    // A removed shape: only its slide.
    fireEvent.click(screen.getAllByTestId('change-item')[2]!);
    expect(onShow).toHaveBeenLastCalledWith(0, undefined);
    // A removed slide is not in the open deck.
    fireEvent.click(screen.getAllByTestId('change-item')[0]!);
    expect(onShow).toHaveBeenCalledTimes(2);
  });
});
