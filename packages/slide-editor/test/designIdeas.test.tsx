// @vitest-environment happy-dom
import type { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SlideCommand, SlideDesignIdeas, SlideEditResult } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DesignIdeasPane, IDEA_WIDTH } from '../src/designIdeas';

afterEach(cleanup);

const i18n = createI18n('en-US');
const stores = {
  glyphs: { ensure: () => Promise.resolve() } as unknown as GlyphStore,
  images: { ensure: () => Promise.resolve() } as unknown as ImageStore,
};
const page = { width: 200, height: 112.5, commands: [] };

function host(ideas: SlideDesignIdeas, error?: string) {
  return vi.fn<(c: SlideCommand) => Promise<SlideEditResult | null>>((c) =>
    Promise.resolve(
      (c.cmd === 'designIdeas'
        ? { designIdeas: ideas, ...(error ? { error } : {}) }
        : {}) as unknown as SlideEditResult,
    ),
  );
}

function pane(run: ReturnType<typeof host>, revision = 0) {
  return (
    <I18nextProvider i18n={i18n}>
      <DesignIdeasPane {...stores} slide={0} revision={revision} run={run} />
    </I18nextProvider>
  );
}

describe('DesignIdeasPane', () => {
  it('asks the engine for ideas and applies the one clicked', async () => {
    const run = host({
      kind: 'bullets',
      ideas: [
        { id: 'bullets.classic', page },
        { id: 'bullets.sideTitle', page },
        { id: 'bullets.panel', page },
        { id: 'bullets.strip', page },
      ],
    });
    render(pane(run));
    expect(run).toHaveBeenCalledWith({ cmd: 'designIdeas', width: IDEA_WIDTH });
    const ideas = await screen.findAllByRole('listitem');
    expect(ideas).toHaveLength(4);
    expect(screen.getByText(/Title and text/)).toBeTruthy();
    expect(ideas[1]!.getAttribute('aria-label')).toBe('Apply layout 2');
    fireEvent.click(screen.getByTestId('design-idea-1'));
    expect(run).toHaveBeenLastCalledWith({ cmd: 'designApply', idea: 'bullets.sideTitle' });
  });

  it('asks again after the slide changed (waiting out typing)', async () => {
    vi.useFakeTimers();
    try {
      const run = host({ kind: 'titleOnly', ideas: [{ id: 'titleOnly.centered', page }] });
      const { rerender } = render(pane(run, 0));
      expect(run).toHaveBeenCalledTimes(1);
      rerender(pane(run, 1));
      rerender(pane(run, 2));
      expect(run).toHaveBeenCalledTimes(1);
      await vi.advanceTimersByTimeAsync(500);
      expect(run).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it('explains an empty slide and the master view', async () => {
    render(pane(host({ kind: null, ideas: [] })));
    await waitFor(() =>
      expect(screen.getByTestId('design-ideas-empty').textContent).toMatch(/Nothing to arrange/),
    );
    cleanup();
    render(pane(host({ kind: null, ideas: [] }, 'masterView')));
    await waitFor(() =>
      expect(screen.getByTestId('design-ideas-empty').textContent).toMatch(/master view/),
    );
  });
});
