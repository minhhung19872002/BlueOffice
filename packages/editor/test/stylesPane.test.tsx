// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { DocStyleEntry } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { StylesPane } from '../src/StylesPane';
import { togglePane } from '../src/store';

afterEach(cleanup);

const i18n = createI18n('en-US');

const entry = (id: string, name: string, kind: 'paragraph' | 'character' = 'paragraph') =>
  ({
    id,
    name,
    kind,
    quick: true,
    preview: {
      font: 'Arial',
      size: 16,
      bold: true,
      italic: false,
      underline: false,
      color: '112233',
    },
  }) satisfies DocStyleEntry;

describe('StylesPane', () => {
  it('lists the document styles, marks the current one, filters and applies on click', async () => {
    const load = vi.fn(async () => [
      entry('Normal', 'Normal'),
      entry('Heading1', 'heading 1'),
      entry('Strong', 'Strong', 'character'),
    ]);
    const apply = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <StylesPane load={load} current="Heading1" refreshKey="a" apply={apply} />
      </I18nextProvider>,
    );
    const items = await screen.findAllByTestId('style-item');
    expect(items.map((b) => b.getAttribute('data-style-id'))).toEqual([
      'Normal',
      'Heading1',
      'Strong',
    ]);
    expect(items[1]!.getAttribute('aria-pressed')).toBe('true');
    expect((items[1]!.firstChild as HTMLElement).style.fontWeight).toBe('700');
    fireEvent.click(items[2]!);
    expect(apply).toHaveBeenCalledWith('Strong');
    fireEvent.change(screen.getByTestId('styles-filter'), { target: { value: 'head' } });
    expect(screen.getAllByTestId('style-item')).toHaveLength(1);
  });

  it('togglePane opens at the end and closes', () => {
    expect(togglePane([], 'styles')).toEqual(['styles']);
    expect(togglePane(['styles', 'comments'], 'styles', true)).toEqual(['comments', 'styles']);
    expect(togglePane(['styles', 'comments'], 'styles')).toEqual(['comments']);
  });
});
