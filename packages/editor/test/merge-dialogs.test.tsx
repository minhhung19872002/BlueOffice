// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  EditCommand,
  EditResult,
  LabelSheet,
  MailMessage,
  MailProvider,
  MergeDataProvider,
  MergeOut,
  MergeState,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  answersOf,
  buildRule,
  EnvelopeDialog,
  FinishDialog,
  fromBase64,
  labelPresetName,
  LabelsDialog,
  ProviderDialog,
  RecipientsDialog,
  recipientOptions,
  RuleDialog,
  withField,
} from '../src/MergeDialogs';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const MERGE: MergeState = {
  name: 'khach-hang.csv',
  fields: ['Họ tên', 'Email', 'TinhThanh'],
  count: 3,
  preview: null,
  active: 3,
  options: {},
};

const SHEET: LabelSheet = {
  id: 'a4-3x8',
  pageWidth: 210,
  pageHeight: 297,
  rows: 8,
  cols: 3,
  top: 12.9,
  left: 7.25,
  labelWidth: 63.5,
  labelHeight: 33.9,
  hPitch: 66,
  vPitch: 33.9,
};

/** A fake engine: answers queries from `out`, records every command. */
function engine(out: Partial<Record<EditCommand['cmd'], MergeOut>> = {}) {
  const sent: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    sent.push(c);
    const mergeOut = out[c.cmd];
    return { info: { name: 'x.docx' }, ...(mergeOut ? { mergeOut } : {}) } as EditResult;
  });
  return { run, sent };
}

const RECORDS: MergeOut = {
  records: {
    fields: MERGE.fields,
    total: 3,
    active: 3,
    rows: [
      { index: 0, values: ['Nguyễn Văn An', 'an@vd.vn', 'Hà Nội'], included: true, position: 0 },
      { index: 1, values: ['Trần Thị Bình', '', 'Đà Nẵng'], included: true, position: 1 },
      { index: 2, values: ['Nguyễn Văn An', 'an2@vd.vn', 'Huế'], included: true, position: 2 },
    ],
  },
};

describe('helpers', () => {
  it('builds recipient options from the dialog rows', () => {
    expect(
      recipientOptions(
        [
          { field: 'TinhThanh', op: 'eq', value: 'Hà Nội' },
          { field: '', op: 'eq', value: 'ignored' },
        ],
        [{ field: 'Họ tên', descending: true }, { field: '' }, { field: '' }],
        new Set([2, 0]),
      ),
    ).toEqual({
      filter: [{ field: 'TinhThanh', op: 'eq', value: 'Hà Nội' }],
      sort: [{ field: 'Họ tên', descending: true }],
      excluded: [0, 2],
    });
  });

  it('builds rules only when complete', () => {
    const base = {
      field: 'GioiTinh',
      op: '=' as const,
      value: 'Nữ',
      then: 'Bà',
      otherwise: 'Ông',
      name: '',
      prompt: 'Ngày họp?',
      default: '',
      once: true,
    };
    expect(buildRule('if', base)).toEqual({
      kind: 'if',
      field: 'GioiTinh',
      op: '=',
      value: 'Nữ',
      then: 'Bà',
      else: 'Ông',
    });
    expect(buildRule('ask', base)).toBeNull();
    expect(buildRule('ask', { ...base, name: ' NgayHop ' })).toMatchObject({ name: 'NgayHop' });
    expect(buildRule('next', base)).toEqual({ kind: 'next' });
    expect(buildRule('skipIf', { ...base, field: '' })).toBeNull();
  });

  it('collects answers once or per record', () => {
    const prompts = [
      { key: 'ASK:Ngay', kind: 'ask' as const, prompt: 'Ngày?', default: '1/12', once: true },
      {
        key: 'FILLIN:Ghi chú?',
        kind: 'fillin' as const,
        prompt: 'Ghi chú?',
        default: '',
        once: false,
      },
    ];
    expect(
      answersOf(
        prompts,
        { 'ASK:Ngay': '5/12' },
        { 'FILLIN:Ghi chú?': ['a', 'b'] },
        {
          'FILLIN:Ghi chú?': true,
        },
      ),
    ).toEqual({ 'ASK:Ngay': ['5/12'], 'FILLIN:Ghi chú?': ['a', 'b'] });
    expect(answersOf(prompts, {}, {}, {})).toEqual({
      'ASK:Ngay': ['1/12'],
      'FILLIN:Ghi chú?': [''],
    });
  });

  it('names presets from their geometry and decodes base64', () => {
    expect(labelPresetName(SHEET)).toBe('A4 · 3 × 8 · 63.5 × 33.9 mm');
    expect(withField('Thư - ', 'Họ tên')).toBe('Thư - «Họ tên»');
    expect([...fromBase64('UEsDBA==')]).toEqual([0x50, 0x4b, 3, 4]);
  });
});

describe('RecipientsDialog', () => {
  it('filters, sorts, excludes and finds duplicates', async () => {
    const { run, sent } = engine({
      mergeRecords: RECORDS,
      mergeDuplicates: { duplicates: [[0, 2]] },
    });
    const onClose = vi.fn();
    wrap(<RecipientsDialog merge={MERGE} run={run} onClose={onClose} />);
    await screen.findByTestId('merge-record-2');
    expect(screen.getByTestId('merge-active').textContent).toBe('3 of 3 records will be merged');
    fireEvent.change(screen.getByTestId('merge-filter-field-0'), {
      target: { value: 'TinhThanh' },
    });
    fireEvent.change(screen.getByTestId('merge-filter-op-0'), { target: { value: 'ne' } });
    fireEvent.change(screen.getByTestId('merge-filter-value-0'), { target: { value: 'Huế' } });
    fireEvent.click(screen.getByTestId('merge-filter-add'));
    fireEvent.change(screen.getByTestId('merge-filter-join-1'), { target: { value: 'or' } });
    fireEvent.change(screen.getByTestId('merge-filter-field-1'), { target: { value: 'Email' } });
    fireEvent.change(screen.getByTestId('merge-filter-op-1'), { target: { value: 'notEmpty' } });
    expect(screen.queryByTestId('merge-filter-value-1')).toBeNull();
    fireEvent.change(screen.getByTestId('merge-sort-field-0'), { target: { value: 'Họ tên' } });
    fireEvent.change(screen.getByTestId('merge-sort-order-0'), { target: { value: 'desc' } });
    fireEvent.click(screen.getByTestId('merge-include-1'));
    fireEvent.click(screen.getByTestId('merge-duplicates'));
    await waitFor(() =>
      expect(screen.getByTestId('merge-duplicates-count').textContent).toBe(
        '1 groups of duplicates',
      ),
    );
    expect(screen.getByTestId('merge-record-2').getAttribute('data-duplicate')).toBe('true');
    fireEvent.click(screen.getByTestId('merge-recipients-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(sent.at(-1)).toEqual({
      cmd: 'mergeSetRecipients',
      options: {
        filter: [
          { field: 'TinhThanh', op: 'ne', value: 'Huế' },
          { field: 'Email', op: 'notEmpty', value: '', or: true },
        ],
        sort: [{ field: 'Họ tên', descending: true }],
        excluded: [1],
      },
    });
  });
});

describe('RuleDialog', () => {
  it('inserts an IF rule', async () => {
    const { run, sent } = engine();
    const onClose = vi.fn();
    wrap(<RuleDialog kind="if" fields={['GioiTinh', 'Họ tên']} run={run} onClose={onClose} />);
    expect(screen.getByText('Conditional text (IF)')).toBeTruthy();
    fireEvent.change(screen.getByTestId('merge-rule-op'), { target: { value: '<>' } });
    fireEvent.change(screen.getByTestId('merge-rule-value'), { target: { value: 'Nam' } });
    fireEvent.change(screen.getByTestId('merge-rule-then'), { target: { value: 'Kính gửi bà' } });
    fireEvent.change(screen.getByTestId('merge-rule-else'), { target: { value: 'Kính gửi ông' } });
    fireEvent.click(screen.getByTestId('merge-rule-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(sent).toEqual([
      {
        cmd: 'mergeInsertRule',
        rule: {
          kind: 'if',
          field: 'GioiTinh',
          op: '<>',
          value: 'Nam',
          then: 'Kính gửi bà',
          else: 'Kính gửi ông',
        },
      },
    ]);
  });

  it('needs a bookmark name for ASK', () => {
    const { run } = engine();
    wrap(<RuleDialog kind="ask" fields={[]} run={run} onClose={() => {}} />);
    const ok = screen.getByTestId('merge-rule-ok') as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    fireEvent.change(screen.getByTestId('merge-rule-name'), { target: { value: 'NgayHop' } });
    expect(ok.disabled).toBe(false);
  });
});

/** A fake mail service: keeps what it was asked to send. */
class FakeMail implements MailProvider {
  batches: MailMessage[][] = [];
  async send(messages: MailMessage[]) {
    this.batches.push(messages);
    return { sent: messages.length, failed: [] };
  }
}

describe('FinishDialog', () => {
  it('answers prompts and sends one e-mail per recipient', async () => {
    const mails: MailMessage[] = [
      { to: 'an@vd.vn', subject: 'Thư mời Nguyễn Văn An', html: '<p>Kính gửi Nguyễn Văn An</p>' },
      { to: 'an2@vd.vn', subject: 'Thư mời Nguyễn Văn An', html: '<p>…</p>' },
    ];
    const { run, sent } = engine({
      mergePrompts: {
        prompts: [
          {
            key: 'FILLIN:Lời chào?',
            kind: 'fillin',
            prompt: 'Lời chào?',
            default: 'Xin chào',
            once: false,
          },
        ],
      },
      mergeToEmails: { mails, skipped: 1 },
    });
    const mail = new FakeMail();
    wrap(
      <FinishDialog
        mode="email"
        merge={MERGE}
        run={run}
        host={{ onMerged: () => {}, saveZip: async () => null, mail }}
        onClose={() => {}}
      />,
    );
    const answer = await screen.findByTestId('merge-answer-FILLIN:Lời chào?');
    expect((answer as HTMLInputElement).value).toBe('Xin chào');
    fireEvent.change(answer, { target: { value: 'Thân gửi' } });
    expect((screen.getByTestId('merge-mail-to') as HTMLSelectElement).value).toBe('Email');
    fireEvent.change(screen.getByTestId('merge-mail-subject'), { target: { value: 'Thư mời ' } });
    fireEvent.change(screen.getByTestId('merge-subject-field'), { target: { value: 'Họ tên' } });
    fireEvent.change(screen.getByTestId('merge-mail-attach'), { target: { value: 'pdf' } });
    fireEvent.click(screen.getByTestId('merge-finish-ok'));
    await screen.findByTestId('merge-done');
    expect(sent.slice(1)).toEqual([
      { cmd: 'mergeSetAnswers', answers: { 'FILLIN:Lời chào?': ['Thân gửi'] } },
      { cmd: 'mergeToEmails', to: 'Email', subject: 'Thư mời «Họ tên»', attach: 'pdf' },
    ]);
    expect(mail.batches).toEqual([mails]);
    expect(mail.batches[0]?.map((m) => m.subject)).toEqual([
      'Thư mời Nguyễn Văn An',
      'Thư mời Nguyễn Văn An',
    ]);
    expect(screen.getByTestId('merge-done').textContent).toBe(
      '2 e-mails sent, 0 failed, 1 without an address',
    );
  });

  it('asks per record when chosen', async () => {
    const { run, sent } = engine({
      mergePrompts: {
        prompts: [{ key: 'FILLIN:Q', kind: 'fillin', prompt: 'Q', default: 'd', once: false }],
      },
    });
    wrap(
      <FinishDialog
        mode="document"
        merge={{ ...MERGE, active: 2 }}
        run={run}
        host={{ onMerged: () => {}, saveZip: async () => null }}
        onClose={() => {}}
      />,
    );
    fireEvent.click(await screen.findByTestId('merge-prompt-per-record-FILLIN:Q'));
    fireEvent.change(screen.getByTestId('merge-answer-FILLIN:Q-1'), { target: { value: 'hai' } });
    expect(screen.queryByTestId('merge-answer-FILLIN:Q-2')).toBeNull();
    fireEvent.click(screen.getByTestId('merge-finish-ok'));
    await waitFor(() => expect(sent.some((c) => c.cmd === 'mergeToDocument')).toBe(true));
    expect(sent.find((c) => c.cmd === 'mergeSetAnswers')).toEqual({
      cmd: 'mergeSetAnswers',
      answers: { 'FILLIN:Q': ['d', 'hai'] },
    });
  });

  it('saves the zip of PDFs', async () => {
    const { run, sent } = engine({
      mergePrompts: { prompts: [] },
      mergeToPdfs: {
        zip: 'UEsDBA==',
        files: [
          { name: 'Nguyễn Văn An.pdf', pages: 2 },
          { name: 'Trần Thị Bình.pdf', pages: 2 },
        ],
      },
    });
    const saveZip = vi.fn(async () => 'ok');
    wrap(
      <FinishDialog
        mode="pdfs"
        merge={MERGE}
        run={run}
        host={{ onMerged: () => {}, saveZip }}
        onClose={() => {}}
      />,
    );
    expect((screen.getByTestId('merge-pattern') as HTMLInputElement).value).toBe('«Họ tên»');
    await waitFor(() => expect(sent[0]?.cmd).toBe('mergePrompts'));
    fireEvent.click(screen.getByTestId('merge-finish-ok'));
    await screen.findByTestId('merge-done');
    expect(sent.at(-1)).toEqual({ cmd: 'mergeToPdfs', pattern: '«Họ tên»' });
    expect(saveZip).toHaveBeenCalledWith(new Uint8Array([0x50, 0x4b, 3, 4]), 'khach-hang.csv.zip');
    expect(screen.getByTestId('merge-done').textContent).toBe(
      '2 PDF files saved in one zip archive',
    );
  });

  it('e-mail needs a mail service', () => {
    const { run } = engine({ mergePrompts: { prompts: [] } });
    wrap(
      <FinishDialog
        mode="email"
        merge={MERGE}
        run={run}
        host={{ onMerged: () => {}, saveZip: async () => null }}
        onClose={() => {}}
      />,
    );
    expect((screen.getByTestId('merge-finish-ok') as HTMLButtonElement).disabled).toBe(true);
  });
});

describe('ProviderDialog', () => {
  it('loads a table from the host data provider', async () => {
    const table = {
      name: 'khach_hang',
      fields: ['Họ tên', 'Email'],
      rows: [['Nguyễn Văn An', 'an@vd.vn']],
    };
    const provider: MergeDataProvider = {
      sources: async () => [
        { id: 'saved', label: 'Danh sách khách hàng' },
        { id: 'sql', label: 'Truy vấn SQL', query: true },
      ],
      load: vi.fn(async () => table),
    };
    const { run, sent } = engine();
    const onLoaded = vi.fn();
    wrap(<ProviderDialog provider={provider} run={run} onLoaded={onLoaded} onClose={() => {}} />);
    await screen.findByText('Truy vấn SQL');
    fireEvent.change(screen.getByTestId('merge-provider-source'), { target: { value: 'sql' } });
    const ok = screen.getByTestId('merge-provider-ok') as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    fireEvent.change(screen.getByTestId('merge-provider-query'), {
      target: { value: 'SELECT ho_ten, email FROM khach_hang' },
    });
    fireEvent.click(ok);
    await waitFor(() => expect(onLoaded).toHaveBeenCalled());
    expect(provider.load).toHaveBeenCalledWith('sql', 'SELECT ho_ten, email FROM khach_hang');
    expect(sent).toEqual([{ cmd: 'mergeTable', ...table }]);
  });
});

describe('Envelopes and labels', () => {
  const presets: MergeOut = {
    presets: {
      labels: [SHEET],
      envelopes: [
        { id: 'dl', width: 220, height: 110 },
        { id: 'c5', width: 229, height: 162 },
      ],
    },
  };

  it('envelope: size, address with fields, position', async () => {
    const { run, sent } = engine({ mergePresets: presets });
    const onDone = vi.fn();
    wrap(
      <EnvelopeDialog fields={['Họ tên', 'DiaChi']} run={run} onDone={onDone} onClose={() => {}} />,
    );
    await screen.findByText('C5 (229 × 162 mm)');
    fireEvent.change(screen.getByTestId('merge-envelope-size'), { target: { value: 'c5' } });
    fireEvent.change(screen.getByTestId('merge-envelope-delivery'), {
      target: { value: '«Họ tên»\n' },
    });
    fireEvent.change(screen.getByTestId('merge-envelope-field'), { target: { value: 'DiaChi' } });
    fireEvent.change(screen.getByTestId('merge-envelope-return'), {
      target: { value: 'Công ty BlueStar' },
    });
    fireEvent.change(screen.getByTestId('merge-envelope-x'), { target: { value: '100' } });
    fireEvent.change(screen.getByTestId('merge-envelope-y'), { target: { value: '80' } });
    fireEvent.click(screen.getByTestId('merge-envelope-ok'));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(sent.at(-1)).toEqual({
      cmd: 'mergeEnvelope',
      size: { id: 'c5', width: 229, height: 162 },
      delivery: '«Họ tên»\n«DiaChi»',
      returnAddress: 'Công ty BlueStar',
      at: [100, 80],
    });
  });

  it('labels: preset, custom geometry and mode', async () => {
    const { run, sent } = engine({ mergePresets: presets });
    const onDone = vi.fn();
    wrap(<LabelsDialog fields={['Họ tên']} run={run} onDone={onDone} onClose={() => {}} />);
    await screen.findByText('A4 · 3 × 8 · 63.5 × 33.9 mm');
    expect(screen.getByTestId('merge-labels-per-page').textContent).toBe('24 labels per page');
    fireEvent.change(screen.getByTestId('merge-labels-sheet'), { target: { value: 'custom' } });
    fireEvent.change(screen.getByTestId('merge-labels-rows'), { target: { value: '7' } });
    expect(screen.getByTestId('merge-labels-per-page').textContent).toBe('21 labels per page');
    fireEvent.click(screen.getByTestId('merge-labels-same'));
    fireEvent.click(screen.getByTestId('merge-labels-ok'));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(sent.at(-1)).toEqual({
      cmd: 'mergeLabels',
      sheet: { ...SHEET, id: 'custom', rows: 7 },
      text: '«Họ tên»',
      mode: 'same',
    });
  });
});
