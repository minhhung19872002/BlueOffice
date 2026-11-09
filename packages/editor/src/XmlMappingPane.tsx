/**
 * BlueDoc XML data mapping pane: the document's custom XML parts (ECMA-376 Part 1, §15.2.4)
 * with a tree of their nodes. Clicking a node binds the content control at the cursor to it
 * (§17.5.2.6 dataBinding); a leaf node's value can be set (every control bound to it follows).
 * Parts can be added from a file or pasted text, and removed. Every change is one undo step.
 */
import { useCallback, useEffect, useState } from 'react';
import type {
  ControlInfo,
  CustomXmlNode,
  CustomXmlPartInfo,
  EditCommand,
  EditResult,
} from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import styles from './XmlMappingPane.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Message keys the engine reports for custom XML commands. */
const ERROR_KEYS = new Set([
  'badXml',
  'badPart',
  'badXPath',
  'noNode',
  'notLeaf',
  'noControl',
  'locked',
  'notBindable',
  'tooLarge',
  'protected',
]);

/** i18n key of an engine error. */
export function xmlErrorKey(error: string): string {
  return `xmlMapping.error.${ERROR_KEYS.has(error) ? error : 'other'}`;
}

/** Text of a picked XML file: UTF-16 when it starts with a byte order mark, else UTF-8. */
export function decodeXmlBytes(bytes: Uint8Array): string {
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) {
    return new TextDecoder('utf-16le').decode(bytes.subarray(2));
  }
  if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) {
    return new TextDecoder('utf-16be').decode(bytes.subarray(2));
  }
  return new TextDecoder('utf-8').decode(bytes);
}

export function XmlMappingPane({
  run,
  control,
  pickFile,
  onClose,
}: {
  run: Run;
  /** The content control at the cursor. */
  control: ControlInfo | null;
  /** Ask the user for an XML file (through the platform). */
  pickFile: () => Promise<Uint8Array | null>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [parts, setParts] = useState<CustomXmlPartInfo[]>([]);
  const [index, setIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [paste, setPaste] = useState<string | null>(null);
  const [picked, setPicked] = useState<CustomXmlNode | null>(null);
  const [value, setValue] = useState('');

  const exec = useCallback(
    async (cmd: EditCommand) => {
      const r = await run(cmd);
      if (!r) return null;
      if (r.customXml) setParts(r.customXml);
      setError(r.error ?? null);
      return r;
    },
    [run],
  );

  useEffect(() => {
    let live = true;
    void run({ cmd: 'customXmlParts' }).then((r) => {
      if (live && r?.customXml) setParts(r.customXml);
    });
    return () => {
      live = false;
    };
  }, [run]);

  const part = parts[Math.min(index, parts.length - 1)] ?? null;

  const add = async (xml: string) => {
    const r = await exec({ cmd: 'customXmlAdd', xml });
    if (r && !r.error) {
      setIndex((r.customXml?.length ?? 1) - 1);
      setPaste(null);
    }
  };

  const addFile = async () => {
    const bytes = await pickFile();
    if (bytes) await add(decodeXmlBytes(bytes));
  };

  const clickNode = async (n: CustomXmlNode) => {
    if (!part) return;
    setPicked(n);
    setValue(n.value ?? '');
    if (control) {
      await exec({
        cmd: 'customXmlBind',
        index: part.index,
        xpath: n.xpath,
        prefixMappings: part.prefixMappings,
      });
    }
  };

  const status = !control
    ? t('xmlMapping.control.none')
    : !control.binding
      ? t('xmlMapping.control.unbound')
      : t(control.bound ? 'xmlMapping.control.bound' : 'xmlMapping.control.missing', {
          xpath: control.binding,
        });

  return (
    <aside
      className={styles.pane}
      aria-label={t('xmlMapping.title')}
      data-testid="xml-mapping-pane"
    >
      <div className={styles.header}>
        <strong>{t('xmlMapping.title')}</strong>
        <button type="button" onClick={onClose} aria-label={t('xmlMapping.close')}>
          ✕
        </button>
      </div>
      <div className={styles.status} data-testid="xml-mapping-control">
        <span>{status}</span>
        {control?.binding && (
          <button
            type="button"
            data-testid="xml-mapping-unbind"
            onClick={() => void exec({ cmd: 'customXmlUnbind' })}
          >
            {t('xmlMapping.unbind')}
          </button>
        )}
      </div>
      <div className={styles.actions}>
        <button type="button" data-testid="xml-mapping-add-file" onClick={() => void addFile()}>
          {t('xmlMapping.addFile')}
        </button>
        <button
          type="button"
          data-testid="xml-mapping-paste"
          onClick={() => setPaste(paste === null ? '' : null)}
        >
          {t('xmlMapping.paste')}
        </button>
        {part && (
          <button
            type="button"
            data-testid="xml-mapping-remove"
            onClick={() => {
              void exec({ cmd: 'customXmlRemove', index: part.index });
              setIndex(0);
              setPicked(null);
            }}
          >
            {t('xmlMapping.remove')}
          </button>
        )}
      </div>
      {paste !== null && (
        <div className={styles.paste}>
          <textarea
            data-testid="xml-mapping-paste-text"
            placeholder={t('xmlMapping.pasteHint')}
            value={paste}
            onChange={(e) => setPaste(e.target.value)}
            rows={6}
          />
          <button
            type="button"
            data-testid="xml-mapping-add-pasted"
            disabled={!paste.trim()}
            onClick={() => void add(paste)}
          >
            {t('xmlMapping.addPasted')}
          </button>
        </div>
      )}
      {error && (
        <div className={styles.error} role="alert" data-testid="xml-mapping-error">
          {t(xmlErrorKey(error))}
        </div>
      )}
      {parts.length === 0 ? (
        <div className={styles.meta}>{t('xmlMapping.noParts')}</div>
      ) : (
        <label className={styles.field}>
          <span>{t('xmlMapping.part')}</span>
          <select
            data-testid="xml-mapping-part"
            value={part?.index ?? 0}
            onChange={(e) => {
              setIndex(Number(e.target.value));
              setPicked(null);
            }}
          >
            {parts.map((p) => (
              <option key={p.partName} value={p.index}>
                {p.root || p.partName} ({p.partName})
              </option>
            ))}
          </select>
        </label>
      )}
      {part && (
        <>
          <div className={styles.meta}>
            {part.namespace && <div>{part.namespace}</div>}
            <div>{t('xmlMapping.boundCount', { count: part.boundControls })}</div>
            {control && <div>{t('xmlMapping.bindHint')}</div>}
          </div>
          {part.broken && <div className={styles.error}>{t('xmlMapping.broken')}</div>}
          <ul className={styles.tree} aria-label={t('xmlMapping.nodes')}>
            {part.nodes.map((n) => (
              <li key={n.xpath}>
                <button
                  type="button"
                  className={`${styles.node} ${control?.binding === n.xpath ? styles.bound : ''} ${picked?.xpath === n.xpath ? styles.picked : ''}`}
                  style={{ paddingLeft: 4 + n.depth * 14 }}
                  data-testid="xml-mapping-node"
                  data-xpath={n.xpath}
                  title={n.xpath}
                  onClick={() => void clickNode(n)}
                >
                  <span className={styles.name}>{n.name}</span>
                  {n.value !== undefined && <span className={styles.value}>{n.value}</span>}
                </button>
              </li>
            ))}
          </ul>
          {picked && picked.value !== undefined && (
            <div className={styles.paste}>
              <label className={styles.field}>
                <span>{t('xmlMapping.value')}</span>
                <input
                  data-testid="xml-mapping-value"
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                />
              </label>
              <button
                type="button"
                data-testid="xml-mapping-set-value"
                onClick={() =>
                  void exec({
                    cmd: 'customXmlSetValue',
                    index: part.index,
                    xpath: picked.xpath,
                    prefixMappings: part.prefixMappings,
                    value,
                  })
                }
              >
                {t('xmlMapping.setValue')}
              </button>
            </div>
          )}
        </>
      )}
    </aside>
  );
}
