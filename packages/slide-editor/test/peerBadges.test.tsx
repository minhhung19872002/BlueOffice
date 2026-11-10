// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SlidePeer } from '@blueoffice/platform';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PeerBadges, peersBySlide } from '../src/peerBadges';

afterEach(cleanup);

const box = { cx: 0, cy: 0, w: 0, h: 0, rot: 0 };
const peer = (clientId: number, name: string, slide: number, slideOnly = false): SlidePeer => ({
  clientId,
  name,
  color: '#15803d',
  slide,
  rect: box,
  ...(slideOnly ? { slideOnly } : {}),
});

describe('slide thumbnail presence', () => {
  it('groups collaborators by slide', () => {
    const g = peersBySlide([peer(1, 'An', 0), peer(2, 'Bình', 2, true), peer(3, 'Chi', 0)]);
    expect(g.get(0)?.map((p) => p.clientId)).toEqual([1, 3]);
    expect(g.get(2)?.map((p) => p.clientId)).toEqual([2]);
    expect(g.has(1)).toBe(false);
  });

  it('draws one badge with initials and colour per collaborator, localised', () => {
    render(
      <I18nextProvider i18n={createI18n('vi-VN')}>
        <PeerBadges peers={[peer(1, 'Trần Thị Bình', 0), peer(2, '', 0, true)]} />
      </I18nextProvider>,
    );
    const badges = screen.getAllByTestId('peer-badge');
    expect(badges.map((b) => b.textContent)).toEqual(['TB', 'K']);
    expect(badges[0]!.getAttribute('aria-label')).toBe('Trần Thị Bình đang ở trang chiếu này');
    expect(badges[0]!.style.background).toMatch(/15803d|21, 128, 61/i);
  });

  it('caps the badges and counts the rest; nothing without collaborators', () => {
    const { container, rerender } = render(
      <I18nextProvider i18n={createI18n('en-US')}>
        <PeerBadges peers={[1, 2, 3, 4, 5].map((i) => peer(i, `User ${i}`, 0))} />
      </I18nextProvider>,
    );
    expect(screen.getAllByTestId('peer-badge')).toHaveLength(3);
    expect(screen.getByTestId('peer-badge-more').textContent).toBe('+2');
    rerender(
      <I18nextProvider i18n={createI18n('en-US')}>
        <PeerBadges peers={undefined} />
      </I18nextProvider>,
    );
    expect(container.querySelector('[data-testid="peer-badge"]')).toBeNull();
  });
});
