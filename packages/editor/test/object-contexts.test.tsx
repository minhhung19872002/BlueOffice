// @vitest-environment happy-dom
import type { EditResult, FormatState } from '@blueoffice/platform';
import { OBJECT_COMMANDS, Ribbon, setRibbonPrefs, type RibbonTab } from '@blueoffice/ui';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { docObjectContexts, docObjectTabs } from '../src/objectContexts';

// CORE-UI-008: BlueDoc's Equation and Object tabs from the engine's selection state.

afterEach(cleanup);

const format = (f: Partial<FormatState>) => f as FormatState;
const edit = (e: Partial<Pick<EditResult, 'equation' | 'format'>>) =>
  e as Pick<EditResult, 'equation' | 'format'>;
const t = (k: string) => k;
const item = (id: string, labelKey: string) => ({ id, label: labelKey, onSelect: () => {} });

describe('docObjectContexts', () => {
  it('finds the equation the caret is in and the embedded object at the selection', () => {
    expect(docObjectContexts(null)).toEqual([]);
    expect(docObjectContexts(edit({ format: format({ bold: true }) }))).toEqual([]);
    expect(
      docObjectContexts(
        edit({ equation: { linear: 'x^2', display: false, isLinear: false } as never }),
      ),
    ).toEqual(['equation']);
    expect(docObjectContexts(edit({ format: format({ ole: true, pictureTools: true }) }))).toEqual([
      'object',
    ]);
    expect(docObjectContexts(edit({ format: format({ ole: false, pictureTools: true }) }))).toEqual(
      [],
    );
  });
});

describe('docObjectTabs', () => {
  it('reuses the existing equation and object commands', () => {
    const [eq, obj] = docObjectTabs(t, item, ['equation', 'object']);
    const ids = (tab: RibbonTab | undefined) =>
      tab?.groups.flatMap((g) => g.items.map((i) => i.id));
    expect(eq?.context).toBe('equation');
    expect(ids(eq)).toEqual(['insert.equation', 'equation.professional', 'equation.linear']);
    expect(obj?.context).toBe('object');
    expect(ids(obj)).toEqual(['object.edit', 'object.saveCopy', 'object.updateLinks']);
    for (const id of ids(obj) ?? []) expect(OBJECT_COMMANDS).toContain(id);
    expect(docObjectTabs(t, item, [])).toEqual([]);
  });

  it('shows the tabs under their context band in the ribbon', () => {
    setRibbonPrefs({ simplified: false, collapsed: false, autoHide: false });
    const home: RibbonTab = { id: 'home', label: 'Home', groups: [] };
    const { container } = render(
      <Ribbon
        tabs={[home, ...docObjectTabs(t, item, ['object'])]}
        labels={{ contexts: { object: 'Object tools' } }}
      />,
    );
    const band = container.querySelector('[data-context-group="object"]');
    expect(band?.textContent).toContain('Object tools');
    // An object appearing activates its tab.
    expect(screen.getByRole('tab', { selected: true }).textContent).toBe(
      'ribbon.objectTabs.object',
    );
    expect(container.querySelector('[data-command="object.edit"]')).not.toBeNull();
  });
});
