// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  BLOCK_GALLERIES,
  BLOCK_GALLERY_GROUPS,
  type BlockInfo,
  blocksOfGroup,
  type EditCommand,
  type EditResult,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  BlockGalleryMenu,
  BlocksOrganizer,
  blockErrorKey,
  clearBlockPreviews,
  galleryLabelKey,
  SaveBlockDialog,
  suggestedBlockName,
} from '../src/BuildingBlocks';

afterEach(() => {
  cleanup();
  clearBlockPreviews();
});

const i18n = createI18n('en-US');
const vi18n = createI18n('vi-VN');
const wrap = (n: ReactNode) =>
  render(
    <div className="bo-root">
      <I18nextProvider i18n={i18n}>{n}</I18nextProvider>
    </div>,
  );

const block = (
  key: string,
  name: string,
  gallery: string,
  category = 'General',
  extra: Partial<BlockInfo> = {},
): BlockInfo => ({
  key,
  store: key.startsWith('doc') ? 'document' : key.startsWith('user') ? 'user' : 'builtin',
  name,
  gallery,
  category,
  description: `${name} description`,
  behavior: 'p',
  text: `${name} text`,
  ...extra,
});

const LIST: BlockInfo[] = [
  block('builtin:3', 'Header – Three columns', 'hdrs', 'BlueOffice'),
  block('builtin:4', 'Header – Title', 'hdrs', 'BlueOffice'),
  block('builtin:7', 'Footer – Page', 'ftrs', 'BlueOffice'),
  block('builtin:11', 'Page number – Centre', 'pgNumB', 'BlueOffice'),
  block('builtin:20', 'Recipients', 'autoTxt', 'Administrative'),
  block('user:0', 'kg', 'autoTxt', 'Letters', { behavior: 'content' }),
  block('doc:0', 'Project header', 'hdrs', 'Project'),
  block('doc:1', 'Sign-off', 'docParts', 'Project'),
];

/** An engine double: answers the list and previews, records what else is sent. */
function engine(list: BlockInfo[] = LIST, answer: (c: EditCommand) => Partial<EditResult> = () => ({})) {
  const sent: EditCommand[] = [];
  const reply = async (c: EditCommand): Promise<EditResult> => {
    if (c.cmd === 'buildingBlocks') return { blocks: { list } } as EditResult;
    if (c.cmd === 'buildingBlockPreview')
      return { blocks: { list, preview: `PNG-${c.key}` } } as EditResult;
    if (c.cmd === 'copy')
      return { clip: { text: 'Kính gửi: Ban Giám đốc, công ty\nDòng hai', html: '', internal: '' } } as EditResult;
    sent.push(c);
    return { blocks: { list }, ...answer(c) } as EditResult;
  };
  return { run: vi.fn(reply), query: vi.fn(reply), sent };
}

describe('building block helpers', () => {
  it('groups galleries for the ribbon, document blocks first', () => {
    expect(blocksOfGroup(LIST, 'header').map((b) => b.key)).toEqual([
      'doc:0',
      'builtin:3',
      'builtin:4',
    ]);
    expect(blocksOfGroup(LIST, 'quickParts').map((b) => b.key)).toEqual([
      'doc:1',
      'user:0',
      'builtin:20',
    ]);
    expect(blocksOfGroup(LIST, 'pageNumber')).toHaveLength(1);
    expect(blocksOfGroup(LIST, 'coverPage')).toEqual([]);
    // Every gallery offered when saving is shown by some ribbon gallery.
    const shown = Object.values(BLOCK_GALLERY_GROUPS).flat() as string[];
    for (const g of BLOCK_GALLERIES) expect(shown).toContain(g);
  });

  it('names galleries and errors in both languages', () => {
    for (const g of [...BLOCK_GALLERIES, 'pgNumT', 'pgNumB', 'pgNumMargins', 'custHdrs']) {
      expect(i18n.exists(galleryLabelKey(g))).toBe(true);
      expect(vi18n.exists(galleryLabelKey(g))).toBe(true);
    }
    expect(galleryLabelKey('custHdrs')).toBe('blocks.galleries.other');
    expect(blockErrorKey('noAutoText')).toBe('blocks.errors.noAutoText');
    expect(blockErrorKey('something else')).toBeNull();
    expect(blockErrorKey(null)).toBeNull();
    for (const e of ['noBlock', 'badName', 'emptySelection', 'builtinBlock', 'noAutoText', 'bodyOnly'])
      expect(vi18n.exists(blockErrorKey(e) ?? '')).toBe(true);
    for (const g of Object.keys(BLOCK_GALLERY_GROUPS))
      expect(vi18n.exists(`blocks.groups.${g}`)).toBe(true);
  });

  it('suggests a block name from the first words, without commas', () => {
    expect(suggestedBlockName('Kính gửi: Ban Giám đốc, công ty\nDòng hai')).toBe(
      'Kính gửi: Ban Giám',
    );
    expect(suggestedBlockName('  ')).toBe('');
    expect(suggestedBlockName('a,b')).toBe('a b');
  });
});

describe('gallery dropdown', () => {
  const menu = (e: ReturnType<typeof engine>, onAction = vi.fn()) =>
    wrap(
      <BlockGalleryMenu
        groups={['header']}
        label="Header"
        icon={null}
        testId="header-gallery"
        disabled={false}
        run={e.run}
        query={e.query}
        page={2}
        actions={[{ id: 'insert.header', label: 'Edit header', onSelect: onAction }]}
      />,
    );

  it('shows the blocks by category with engine previews and inserts the picked one', async () => {
    const e = engine();
    menu(e);
    fireEvent.click(screen.getByTestId('header-gallery'));
    const gallery = await screen.findByTestId('block-gallery-header');
    const items = [...gallery.querySelectorAll<HTMLElement>('[data-block]')];
    expect(items.map((i) => i.dataset.block)).toEqual([
      'Project header',
      'Header – Three columns',
      'Header – Title',
    ]);
    expect([...gallery.querySelectorAll('h4')].map((h) => h.textContent)).toEqual([
      'Project',
      'BlueOffice',
    ]);
    // Previews are pictures drawn by the engine.
    await waitFor(() => expect(screen.getAllByTestId('block-preview')).toHaveLength(3));
    expect(screen.getAllByTestId('block-preview')[0]?.getAttribute('src')).toBe(
      'data:image/png;base64,PNG-doc:0',
    );
    fireEvent.click(items[1] as HTMLElement);
    expect(e.sent).toEqual([{ cmd: 'insertBuildingBlock', key: 'builtin:3', page: 2 }]);
    // The menu closed.
    expect(screen.queryByTestId('block-gallery-header')).toBeNull();
  });

  it('runs the action below the gallery and says when a gallery is empty', async () => {
    const e = engine([]);
    const onAction = vi.fn();
    menu(e, onAction);
    fireEvent.click(screen.getByTestId('header-gallery'));
    expect((await screen.findByTestId('blocks-empty')).textContent).toBe(
      'No building blocks in this gallery.',
    );
    fireEvent.click(document.querySelector('[data-command="insert.header"]') as HTMLElement);
    expect(onAction).toHaveBeenCalledTimes(1);
    expect(e.sent).toEqual([]);
  });
});

describe('save selection as building block', () => {
  it('suggests the name, refuses a comma and saves with the chosen properties', async () => {
    const e = engine();
    const onClose = vi.fn();
    wrap(<SaveBlockDialog run={e.run} query={e.query} onClose={onClose} />);
    const name = screen.getByTestId('block-name') as HTMLInputElement;
    await waitFor(() => expect(name.value).toBe('Kính gửi: Ban Giám'));
    const ok = screen.getByTestId('save-block-ok') as HTMLButtonElement;
    fireEvent.change(name, { target: { value: 'a, b' } });
    expect(ok.disabled).toBe(true);
    fireEvent.change(name, { target: { value: ' kg ' } });
    expect(ok.disabled).toBe(false);
    fireEvent.change(screen.getByTestId('block-gallery'), { target: { value: 'hdrs' } });
    fireEvent.change(screen.getByTestId('block-category'), { target: { value: 'Thư từ' } });
    fireEvent.change(screen.getByTestId('block-description'), { target: { value: 'Lời chào' } });
    fireEvent.change(screen.getByTestId('block-behavior'), { target: { value: 'p' } });
    fireEvent.change(screen.getByTestId('block-store'), { target: { value: 'document' } });
    fireEvent.click(ok);
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(e.sent).toEqual([
      {
        cmd: 'saveBuildingBlock',
        store: 'document',
        spec: {
          name: 'kg',
          gallery: 'hdrs',
          category: 'Thư từ',
          description: 'Lời chào',
          behavior: 'p',
        },
      },
    ]);
  });

  it('defaults to auto text in the user store and shows the engine error', async () => {
    const e = engine(LIST, () => ({ error: 'emptySelection' }));
    const onClose = vi.fn();
    wrap(<SaveBlockDialog run={e.run} query={e.query} onClose={onClose} />);
    fireEvent.change(screen.getByTestId('block-name'), { target: { value: 'x' } });
    fireEvent.click(screen.getByTestId('save-block-ok'));
    expect((await screen.findByTestId('save-block-error')).textContent).toBe(
      'Select the content to save as a building block first.',
    );
    expect(onClose).not.toHaveBeenCalled();
    expect(e.sent[0]).toMatchObject({
      cmd: 'saveBuildingBlock',
      store: 'user',
      spec: { gallery: 'autoTxt', behavior: 'content' },
    });
  });
});

describe('organizer', () => {
  const open = (e: ReturnType<typeof engine>, onClose = vi.fn()) => {
    wrap(<BlocksOrganizer run={e.run} query={e.query} page={0} onClose={onClose} />);
    return onClose;
  };
  const row = (name: string) =>
    document.querySelector(`[data-testid="organizer-list"] [data-block="${name}"]`) as HTMLElement;
  const button = (id: string) => screen.getByTestId(id) as HTMLButtonElement;

  it('lists every block with its gallery and store; library blocks are read-only', async () => {
    const e = engine();
    open(e);
    await waitFor(() => expect(row('kg')).not.toBeNull());
    expect(document.querySelectorAll('[data-testid="organizer-list"] tbody tr')).toHaveLength(
      LIST.length,
    );
    expect(row('kg').textContent).toContain('Auto text');
    expect(row('kg').textContent).toContain('My building blocks');
    expect(button('organizer-insert').disabled).toBe(true);
    fireEvent.click(row('Recipients'));
    expect(button('organizer-edit').disabled).toBe(true);
    expect(button('organizer-delete').disabled).toBe(true);
    expect(button('organizer-insert').disabled).toBe(false);
    await waitFor(() =>
      expect(screen.getByTestId('block-preview').getAttribute('src')).toContain('PNG-builtin:20'),
    );
  });

  it('edits the properties of a block and deletes one', async () => {
    const e = engine();
    open(e);
    await waitFor(() => expect(row('kg')).not.toBeNull());
    fireEvent.click(row('kg'));
    fireEvent.click(button('organizer-edit'));
    const name = screen.getByTestId('block-name') as HTMLInputElement;
    expect(name.value).toBe('kg');
    fireEvent.change(name, { target: { value: 'kinh gui' } });
    fireEvent.change(screen.getByTestId('block-gallery'), { target: { value: 'docParts' } });
    fireEvent.click(button('organizer-apply'));
    await waitFor(() => expect(e.sent).toHaveLength(1));
    expect(e.sent[0]).toEqual({
      cmd: 'editBuildingBlock',
      key: 'user:0',
      spec: {
        name: 'kinh gui',
        gallery: 'docParts',
        category: 'Letters',
        description: 'kg description',
        behavior: 'content',
      },
    });
    await waitFor(() => expect(screen.queryByTestId('organizer-form')).toBeNull());
    fireEvent.click(row('Sign-off'));
    fireEvent.click(button('organizer-delete'));
    await waitFor(() => expect(e.sent).toHaveLength(2));
    expect(e.sent[1]).toEqual({ cmd: 'deleteBuildingBlock', key: 'doc:1' });
  });

  it('inserts the selected block and closes; engine errors are shown', async () => {
    const e = engine(LIST, (c) => (c.cmd === 'deleteBuildingBlock' ? { error: 'noBlock' } : {}));
    const onClose = open(e);
    await waitFor(() => expect(row('Sign-off')).not.toBeNull());
    fireEvent.click(row('Sign-off'));
    fireEvent.click(button('organizer-delete'));
    expect((await screen.findByTestId('organizer-error')).textContent).toBe(
      'The building block no longer exists.',
    );
    fireEvent.click(button('organizer-insert'));
    expect(onClose).toHaveBeenCalled();
    expect(e.sent[1]).toEqual({ cmd: 'insertBuildingBlock', key: 'doc:1', page: 0 });
  });
});
