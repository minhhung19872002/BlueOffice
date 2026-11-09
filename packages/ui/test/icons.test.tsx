// @vitest-environment happy-dom
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { hasIcon, Icon, ICONS, renderSourcesMd } from '../src';

afterEach(cleanup);

const sourcesPath = resolve(import.meta.dirname, '../icons/SOURCES.md');

describe('icon set', () => {
  it('SOURCES.md lists the origin of every icon', () => {
    const md = renderSourcesMd();
    if (process.env.BO_WRITE_ICONS === '1') writeFileSync(sourcesPath, md);
    expect(readFileSync(sourcesPath, 'utf8').replace(/\r\n/g, '\n')).toBe(md);
    for (const id of Object.keys(ICONS)) expect(md).toContain(`| \`${id}\` |`);
  });

  it.each([16, 20, 24, 32] as const)('renders every icon as a 24-grid SVG at %ipx', (size) => {
    for (const id of Object.keys(ICONS)) {
      const { container, unmount } = render(<Icon name={id} size={size} />);
      const svg = container.querySelector('svg');
      expect(svg, id).not.toBeNull();
      expect(svg!.getAttribute('viewBox'), id).toBe('0 0 24 24');
      expect(svg!.getAttribute('width'), id).toBe(String(size));
      expect(svg!.getAttribute('stroke-width'), id).toBe('1.5');
      unmount();
    }
  });

  it('is decorative unless labelled', () => {
    const { container } = render(
      <>
        <Icon name="format.bold" />
        <Icon name="file.save" label="Lưu" />
      </>,
    );
    const [a, b] = Array.from(container.querySelectorAll('svg'));
    expect(a!.getAttribute('aria-hidden')).toBe('true');
    expect(b!.getAttribute('role')).toBe('img');
    expect(b!.getAttribute('aria-label')).toBe('Lưu');
  });

  it('covers the basic catalogue (common, text, paragraph, insert, sheet, slide)', () => {
    const basic = [
      'file.new',
      'file.open',
      'file.save',
      'file.print',
      'edit.undo',
      'edit.redo',
      'edit.cut',
      'edit.copy',
      'edit.paste',
      'edit.find',
      'edit.replace',
      'format.bold',
      'format.italic',
      'format.underline',
      'format.strike',
      'format.alignLeft',
      'format.alignCenter',
      'format.alignRight',
      'insert.table',
      'insert.image',
      'insert.equation',
      'app.doc',
      'app.sheet',
      'app.slide',
      'format.thousands',
      'format.currencyVnd',
    ];
    expect(basic.filter((id) => !hasIcon(id))).toEqual([]);
  });

  it('app marks use the accent part and an unknown id renders nothing', () => {
    const { container } = render(<Icon name="app.sheet" />);
    expect(container.querySelector('.bo-icon-accent')).not.toBeNull();
    const none = render(<Icon name="no.such.command" />);
    expect(none.container.innerHTML).toBe('');
  });
});
