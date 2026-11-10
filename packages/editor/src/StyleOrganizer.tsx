/**
 * Style Organizer (DOC-STYLE-001): copy, delete and rename styles between the open
 * document and another `.docx` / `.dotx` opened through the platform's files. Changes
 * to the open document are undoable commands; changes to the other file are written
 * as a new file (saved through the platform).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  EditCommand,
  EditResult,
  OpenedFile,
  OrganizerAction,
  OrganizerInfo,
  OrganizerStyle,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import { fromBase64, toBase64 } from './clipboard/docPaste';
import styles from './Editor.module.css';

const DOCX_MIME = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
const DOTX_MIME = 'application/vnd.openxmlformats-officedocument.wordprocessingml.template';

export interface StyleOrganizerProps {
  run: (c: EditCommand) => Promise<EditResult | null>;
  query: (c: EditCommand) => Promise<EditResult | null>;
  openFile: (accept: string[]) => Promise<OpenedFile | null>;
  saveFile: (bytes: Uint8Array, name: string, mime: string) => Promise<string | null>;
  onClose: () => void;
}

function List({
  label,
  items,
  picked,
  onPick,
  testId,
}: {
  label: string;
  items: OrganizerStyle[];
  picked: string[];
  onPick: (ids: string[]) => void;
  testId: string;
}) {
  return (
    <label className={styles.field} style={{ flexDirection: 'column', alignItems: 'stretch' }}>
      <span style={{ width: 'auto' }}>{label}</span>
      <select
        multiple
        size={12}
        data-testid={testId}
        value={picked}
        onChange={(e) => onPick(Array.from(e.target.selectedOptions, (o) => o.value))}
      >
        {items.map((s) => (
          <option key={s.id} value={s.id}>
            {s.name}
          </option>
        ))}
      </select>
    </label>
  );
}

export function StyleOrganizer({ run, query, openFile, saveFile, onClose }: StyleOrganizerProps) {
  const { t } = useTranslation();
  const [file, setFile] = useState<{ name: string; b64: string } | null>(null);
  const [info, setInfo] = useState<OrganizerInfo | null>(null);
  const [left, setLeft] = useState<string[]>([]);
  const [right, setRight] = useState<string[]>([]);
  const [newName, setNewName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);

  const open = async () => {
    const f = await openFile(['.docx', '.dotx', '.docm', '.dotm']);
    if (!f) return;
    const b64 = toBase64(f.bytes);
    const r = await query({ cmd: 'styleOrganizer', file: b64 });
    setError(r?.error ?? null);
    if (r?.styleMgr?.organizer) {
      setFile({ name: f.name, b64 });
      setInfo(r.styleMgr.organizer);
      setDirty(false);
    }
  };

  const apply = async (action: OrganizerAction, toFile: boolean, ids: string[]) => {
    if (!file || ids.length === 0) return;
    const r = await run({
      cmd: 'styleOrganizerApply',
      file: file.b64,
      action,
      ids,
      toFile,
      ...(action === 'rename' ? { newName } : {}),
    });
    setError(r?.error ?? null);
    const o = r?.styleMgr?.organizer;
    if (!o) return;
    setInfo(o);
    setNote(null);
    if (o.output) {
      setFile({ ...file, b64: o.output });
      setDirty(true);
    }
  };

  const save = async () => {
    if (!file) return;
    const mime = /\.dot[xm]$/i.test(file.name) ? DOTX_MIME : DOCX_MIME;
    const saved = await saveFile(fromBase64(file.b64), file.name, mime);
    if (saved) {
      setDirty(false);
      setNote(t('docStyles.organizer.saved', { name: saved }));
    }
  };

  return (
    <Dialog
      open
      width={640}
      onOpenChange={(o) => !o && onClose()}
      title={t('docStyles.organizer.title')}
      testId="style-organizer"
      footer={
        <>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="organizer-save"
            disabled={!dirty}
            onClick={() => void save()}
          >
            {t('docStyles.organizer.saveFile')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="organizer-close"
            onClick={onClose}
          >
            {t('docStyles.organizer.close')}
          </button>
        </>
      }
    >
      <div onKeyDown={(e) => e.stopPropagation()}>
        <p>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="organizer-open"
            onClick={() => void open()}
          >
            {t('docStyles.organizer.open')}
          </button>{' '}
          {file?.name}
        </p>
        {!info ? (
          <p className={styles.hint}>{t('docStyles.organizer.noFile')}</p>
        ) : (
          <div style={{ display: 'grid', gridTemplateColumns: '1fr auto 1fr', gap: 8 }}>
            <List
              label={t('docStyles.organizer.current')}
              items={info.current}
              picked={left}
              onPick={setLeft}
              testId="organizer-current"
            />
            <div
              style={{ display: 'flex', flexDirection: 'column', gap: 6, justifyContent: 'center' }}
            >
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="organizer-copy-to-file"
                disabled={left.length === 0}
                onClick={() => void apply('copy', true, left)}
              >
                {t('docStyles.organizer.copyToFile')}
              </button>
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="organizer-copy-to-current"
                disabled={right.length === 0}
                onClick={() => void apply('copy', false, right)}
              >
                {t('docStyles.organizer.copyToCurrent')}
              </button>
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="organizer-delete-current"
                disabled={left.length === 0}
                onClick={() => void apply('delete', false, left)}
              >
                {t('docStyles.organizer.deleteCurrent')}
              </button>
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="organizer-delete-file"
                disabled={right.length === 0}
                onClick={() => void apply('delete', true, right)}
              >
                {t('docStyles.organizer.deleteFile')}
              </button>
              <input
                aria-label={t('docStyles.organizer.newName')}
                placeholder={t('docStyles.organizer.newName')}
                data-testid="organizer-new-name"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
              />
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="organizer-rename-current"
                disabled={left.length !== 1 || !newName.trim()}
                onClick={() => void apply('rename', false, left)}
              >
                {t('docStyles.organizer.renameCurrent')}
              </button>
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="organizer-rename-file"
                disabled={right.length !== 1 || !newName.trim()}
                onClick={() => void apply('rename', true, right)}
              >
                {t('docStyles.organizer.renameFile')}
              </button>
            </div>
            <List
              label={t('docStyles.organizer.file', { name: file?.name ?? '' })}
              items={info.file}
              picked={right}
              onPick={setRight}
              testId="organizer-file"
            />
          </div>
        )}
        {note && <p className={styles.hint}>{note}</p>}
        {error && (
          <p role="alert" className={styles.hint} data-testid="organizer-error">
            {t(`docStyles.errors.${error}`, { defaultValue: t('docStyles.errors.failed') })}
          </p>
        )}
      </div>
    </Dialog>
  );
}
