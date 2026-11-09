// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { ControlInfo, CustomXmlPartInfo, EditCommand, EditResult } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { decodeXmlBytes, XmlMappingPane, xmlErrorKey } from '../src/XmlMappingPane';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const PART: CustomXmlPartInfo = {
  index: 0,
  partName: '/customXml/item1.xml',
  itemId: '{1}',
  root: 'dh:donHang',
  namespace: 'urn:x',
  schemaRefs: [],
  broken: false,
  prefixMappings: "xmlns:ns0='urn:x'",
  boundControls: 1,
  nodes: [
    { name: 'dh:donHang', xpath: '/ns0:donHang[1]', depth: 0 },
    { name: 'dh:khach', xpath: '/ns0:donHang[1]/ns0:khach[1]', depth: 1, value: 'An' },
  ],
};

const control = (patch: Partial<ControlInfo> = {}): ControlInfo => ({
  kind: 'text',
  title: null,
  tag: null,
  items: [],
  checked: null,
  date: null,
  format: null,
  placeholder: false,
  locked: false,
  ...patch,
});

function runner(error?: string) {
  const commands: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    commands.push(c);
    return { customXml: [PART], error: error ?? null } as unknown as EditResult;
  });
  return { commands, run };
}

describe('XML mapping pane', () => {
  it('lists the parts and binds the control at the cursor to a clicked node', async () => {
    const { commands, run } = runner();
    wrap(
      <XmlMappingPane
        run={run}
        control={control()}
        pickFile={async () => null}
        onClose={() => {}}
      />,
    );
    expect(commands[0]).toEqual({ cmd: 'customXmlParts' });
    await waitFor(() => expect(screen.getAllByTestId('xml-mapping-node')).toHaveLength(2));
    expect(screen.getByTestId('xml-mapping-control').textContent).toContain('chưa gắn dữ liệu');
    fireEvent.click(screen.getAllByTestId('xml-mapping-node')[1]!);
    await waitFor(() =>
      expect(commands[1]).toEqual({
        cmd: 'customXmlBind',
        index: 0,
        xpath: '/ns0:donHang[1]/ns0:khach[1]',
        prefixMappings: "xmlns:ns0='urn:x'",
      }),
    );
    // The picked leaf's value can be set.
    fireEvent.change(screen.getByTestId('xml-mapping-value'), { target: { value: 'Bình' } });
    fireEvent.click(screen.getByTestId('xml-mapping-set-value'));
    await waitFor(() =>
      expect(commands[2]).toEqual({
        cmd: 'customXmlSetValue',
        index: 0,
        xpath: '/ns0:donHang[1]/ns0:khach[1]',
        prefixMappings: "xmlns:ns0='urn:x'",
        value: 'Bình',
      }),
    );
  });

  it('shows a bound control and unbinds it', async () => {
    const { commands, run } = runner();
    wrap(
      <XmlMappingPane
        run={run}
        control={control({ binding: '/ns0:donHang[1]/ns0:khach[1]', bound: false })}
        pickFile={async () => null}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId('xml-mapping-control').textContent).toContain('không tìm thấy nút');
    fireEvent.click(screen.getByTestId('xml-mapping-unbind'));
    await waitFor(() => expect(commands).toContainEqual({ cmd: 'customXmlUnbind' }));
  });

  it('adds a part from pasted text and from a file, and reports errors', async () => {
    const { commands, run } = runner('badXml');
    const bytes = new Uint8Array([0xef, 0xbb, 0xbf, 0x3c, 0x61, 0x2f, 0x3e]);
    wrap(
      <XmlMappingPane run={run} control={null} pickFile={async () => bytes} onClose={() => {}} />,
    );
    fireEvent.click(screen.getByTestId('xml-mapping-paste'));
    fireEvent.change(screen.getByTestId('xml-mapping-paste-text'), {
      target: { value: '<a><b></a>' },
    });
    fireEvent.click(screen.getByTestId('xml-mapping-add-pasted'));
    await waitFor(() =>
      expect(commands).toContainEqual({ cmd: 'customXmlAdd', xml: '<a><b></a>' }),
    );
    await waitFor(() =>
      expect(screen.getByTestId('xml-mapping-error').textContent).toBe('XML không hợp lệ.'),
    );
    fireEvent.click(screen.getByTestId('xml-mapping-add-file'));
    await waitFor(() => expect(commands).toContainEqual({ cmd: 'customXmlAdd', xml: '<a/>' }));
  });

  it('decodes UTF-16 files and maps unknown errors', () => {
    const le = new Uint8Array([0xff, 0xfe, 0x3c, 0, 0x61, 0, 0x2f, 0, 0x3e, 0]);
    expect(decodeXmlBytes(le)).toBe('<a/>');
    expect(xmlErrorKey('noNode')).toBe('xmlMapping.error.noNode');
    expect(xmlErrorKey('something else')).toBe('xmlMapping.error.other');
  });
});
