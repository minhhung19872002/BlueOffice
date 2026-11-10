/**
 * The File screen of BlueDoc, BlueSheet and BlueSlide (CORE-UI-002): the shared
 * `Backstage` of `@blueoffice/ui` with the pages every app has — New, Open (pinned,
 * recent, browse), Info (document properties, protection, check document,
 * versions), Save, Save As (every output format), Print, Share, Export, Close and
 * Options. The pages run the app's registered commands (closing the screen first,
 * so their dialogs and panes show over the document); a command the app does not
 * register is left out.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  dropsMacros,
  EDITABLE_DOC_PROPERTIES,
  saveVariants,
  variantOf,
  type DocProperties,
  type EditableDocProperty,
  type RecentFile,
  type VariantApp,
} from '@blueoffice/platform';
import {
  Backstage,
  BackstageCommands,
  BackstageFiles,
  BackstageFormats,
  BackstageProperties,
  getDensity,
  setDensity,
  usePinnedFiles,
  type BackstageCommand,
  type BackstageFile,
  type BackstageFormat,
  type BackstageSection,
  type SettingStore,
} from '@blueoffice/ui';
import { useCallback, useEffect, useState } from 'react';

/** Access to the app's command registry. */
export interface BackstageCommandAccess {
  has(id: string): boolean;
  enabled(id: string): boolean;
  run(id: string): void;
}

/** Reads / changes the document properties (`docProperties` / `setDocProperties`). */
export interface BackstagePropertiesAccess {
  load(): Promise<DocProperties | null>;
  save(change: Partial<Pick<DocProperties, EditableDocProperty>>): Promise<DocProperties | null>;
}

export type BackstagePageId =
  'new' | 'open' | 'info' | 'saveAs' | 'print' | 'share' | 'export' | 'options';

export interface FileBackstageProps {
  app: VariantApp;
  open: boolean;
  onClose: () => void;
  commands: BackstageCommandAccess;
  /** A document is open. */
  hasDocument: boolean;
  /** Name of the open file. */
  fileName?: string;
  /** Variant of the open file (`DocInfo.variant`). */
  variant?: string | null;
  macros?: boolean;
  properties?: BackstagePropertiesAccess;
  /** Recent files and the user's settings (pinned files). */
  shell: SettingStore & { recentFiles(): Promise<RecentFile[]> };
  /** Opens a recent / pinned file by path (absent: the platform cannot, e.g. the web). */
  openPath?: (path: string) => void;
  /** Save As one of the app's document types (`saveVariants`). */
  onSaveAs: (ext: string) => void;
  /** Embedding host: Share / Close. */
  onShare?: () => void;
  onCloseDocument?: () => void;
  /** Page shown when the screen opens. */
  initialPage?: BackstagePageId;
}

/** i18n key of each command shown on a page. */
const COMMAND_LABELS: Record<string, string> = {
  'file.new': 'commands.file.new',
  'file.newFromTemplate': 'commands.file.newFromTemplate',
  'file.open': 'commands.file.open',
  'file.openSample': 'commands.file.openSample',
  'file.editTemplate': 'fileVariants.editTemplate',
  'file.themeFile': 'fileVariants.applyThemeFile',
  'file.protect': 'protect.cmd.protect',
  'review.savePassword': 'commands.review.savePassword',
  'file.savePassword': 'commands.review.savePassword',
  'review.checkDocument': 'inspect.command',
  'review.versions': 'versions.title',
  'file.sign': 'commands.file.sign',
  'file.signLine': 'commands.file.signLine',
  'file.verifySignatures': 'commands.file.verifySignatures',
  'file.print': 'backstage.print.preview',
  'file.printNow': 'commands.file.printNow',
  'file.exportPdf': 'commands.file.exportPdf',
  'file.exportPdfOptions': 'commands.file.exportPdfOptions',
  'file.exportSignedPdf': 'commands.file.exportSignedPdf',
  'file.exportPng': 'slideEditor.exportPng',
  'file.exportVideo': 'slideEditor.exportVideo',
  'file.exportDocs': 'slideExport.title',
  'file.saveCsv': 'sheetEditor.saveCsv',
  'file.saveHtml': 'webFormats.saveHtml',
  'file.saveMht': 'webFormats.saveMht',
  'file.saveFlatXml': 'webFormats.saveFlatXml',
  'file.saveEpub': 'webFormats.saveEpub',
  'file.saveSlk': 'tabularFormats.saveSlk',
  'file.saveDif': 'tabularFormats.saveDif',
  'file.savePrn': 'tabularFormats.savePrn',
  'file.saveThemeFile': 'fileVariants.saveThemeFile',
  'design.saveThemeFile': 'fileVariants.saveThemeFile',
  'view.toggleLanguage': 'backstage.options.language',
  'view.toggleTheme': 'backstage.options.theme',
  'view.density': 'backstage.options.density',
  'review.autoCorrect': 'commands.review.autoCorrect',
  'review.spelling': 'commands.review.spelling',
  'review.spellingDialog': 'commands.review.spellingDialog',
  'review.language': 'backstage.options.proofingLanguage',
  'file.checkUpdates': 'commands.file.checkUpdates',
  'options.shortcuts': 'commands.options.shortcuts',
  'options.customizeRibbon': 'commands.options.customizeRibbon',
  'options.customizeQat': 'commands.options.customizeQat',
};

/** Commands of each page, in order (the ones the app registers are shown). */
export const BACKSTAGE_COMMANDS: Record<
  'new' | 'browse' | 'info' | 'print' | 'export' | 'options',
  readonly string[]
> = {
  new: ['file.new', 'file.newFromTemplate'],
  browse: ['file.open', 'file.openSample', 'file.editTemplate', 'file.themeFile'],
  info: [
    'file.protect',
    'review.savePassword',
    'file.savePassword',
    'review.checkDocument',
    'review.versions',
    'file.sign',
    'file.signLine',
    'file.verifySignatures',
  ],
  print: ['file.print', 'file.printNow'],
  export: [
    'file.exportPdf',
    'file.exportPdfOptions',
    'file.exportSignedPdf',
    'file.exportPng',
    'file.exportVideo',
    'file.exportDocs',
    'file.saveCsv',
    'file.saveThemeFile',
    'design.saveThemeFile',
  ],
  options: [
    'view.toggleLanguage',
    'view.toggleTheme',
    'view.density',
    'review.autoCorrect',
    'review.spelling',
    'review.spellingDialog',
    'review.language',
    'options.shortcuts',
    'options.customizeRibbon',
    'options.customizeQat',
    'file.checkUpdates',
  ],
};

/** Other output formats of each app, beside its document types: (command, extension). */
export const OTHER_FORMATS: Record<VariantApp, readonly (readonly [string, string])[]> = {
  doc: [
    ['file.exportPdf', 'pdf'],
    // Web page, single-file web page, Flat OPC (CORE-FMT-003).
    ['file.saveHtml', 'html'],
    ['file.saveMht', 'mht'],
    ['file.saveFlatXml', 'xml'],
    // EPUB 3 publication (CORE-FMT-004).
    ['file.saveEpub', 'epub'],
  ],
  sheet: [
    ['file.saveCsv', 'csv'],
    ['file.exportPdf', 'pdf'],
    ['file.saveHtml', 'html'],
    ['file.saveMht', 'mht'],
    ['file.saveFlatXml', 'xml'],
    // The current sheet as SYLK, DIF, formatted text (CORE-FMT-004).
    ['file.saveSlk', 'slk'],
    ['file.saveDif', 'dif'],
    ['file.savePrn', 'prn'],
    ['file.saveThemeFile', 'thmx'],
  ],
  slide: [
    ['file.exportPdf', 'pdf'],
    ['file.exportPng', 'png'],
    ['file.exportVideo', 'gif'],
    ['file.saveFlatXml', 'xml'],
    ['design.saveThemeFile', 'thmx'],
  ],
};

const DENSITIES = ['normal', 'compact', 'touch'] as const;

/** The File screen of an app. */
export function FileBackstage(p: FileBackstageProps) {
  const { t, i18n } = useTranslation();
  const { app, open, onClose, commands } = p;
  const [page, setPage] = useState<string>(p.initialPage ?? (p.hasDocument ? 'info' : 'open'));
  const [props, setProps] = useState<DocProperties | null>(null);
  const [recent, setRecent] = useState<RecentFile[]>([]);
  const { pinned, toggle } = usePinnedFiles(p.shell, app);
  const [density, setDensityState] = useState(getDensity);
  const { properties, shell, hasDocument } = p;
  // Each time the screen opens: the start page (set while rendering), then the
  // properties and the recent files.
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) {
    setWasOpen(open);
    if (open) {
      setPage(p.initialPage ?? (hasDocument ? 'info' : 'open'));
      setProps(null);
    }
  }
  useEffect(() => {
    if (!open) return undefined;
    let live = true;
    void shell.recentFiles().then(
      (r) => live && setRecent(r),
      () => {},
    );
    if (properties && hasDocument) {
      void properties.load().then(
        (v) => live && setProps(v),
        () => {},
      );
    }
    return () => {
      live = false;
    };
  }, [open, properties, shell, hasDocument]);

  const runAndClose = useCallback(
    (id: string) => {
      onClose();
      commands.run(id);
    },
    [onClose, commands],
  );
  const list = (ids: readonly string[]): BackstageCommand[] =>
    ids
      .filter((id) => commands.has(id))
      // `review.savePassword` and `file.savePassword` are the same command in different apps.
      .filter((id, _, all) => !(id === 'file.savePassword' && all.includes('review.savePassword')))
      .map((id) => ({
        id,
        label: t(COMMAND_LABELS[id] ?? id),
        description: i18n.exists(describeKey(id)) ? t(describeKey(id)) : undefined,
        enabled: commands.enabled(id),
        onSelect: () => runAndClose(id),
      }));

  // Open: pinned first, then the other recent files.
  const pinnedPaths = new Set(pinned.map((f) => f.path));
  const files: BackstageFile[] = [
    ...pinned.map((f) => ({ id: f.path, name: f.name, detail: f.path, pinned: true })),
    ...recent
      .filter((r) => !pinnedPaths.has(r.path))
      .map((r) => ({
        id: r.path,
        name: r.name,
        detail: new Date(r.openedAt).toLocaleString(i18n.language),
        pinned: false,
      })),
  ];
  const openFile = (f: BackstageFile) => {
    if (!p.openPath) return;
    onClose();
    p.openPath(f.id);
  };

  // Save As: the app's document types, then the other formats.
  const current = variantOf(p.variant)?.ext;
  const types: BackstageFormat[] = saveVariants(app).map((v) => ({
    id: v.ext,
    label: t(`fileVariants.types.${v.ext}`),
    ext: `.${v.ext}`,
    current: v.ext === current,
    note: dropsMacros(p.macros, v.ext) ? t('fileVariants.macroDropped') : undefined,
    enabled: hasDocument,
    onSelect: () => {
      onClose();
      p.onSaveAs(v.ext);
    },
  }));
  const others: BackstageFormat[] = OTHER_FORMATS[app]
    .filter(([id]) => commands.has(id))
    .map(([id, ext]) => ({
      id,
      label: t(`backstage.formats.${ext}`),
      ext: `.${ext}`,
      enabled: commands.enabled(id),
      onSelect: () => runAndClose(id),
    }));

  const saveProps = (field: string, value: string) => {
    if (!properties) return;
    void properties.save({ [field]: value }).then(
      (v) => v && setProps(v),
      () => {},
    );
  };
  const readOnly: (keyof DocProperties)[] = ['lastModifiedBy', 'created', 'modified', 'revision'];
  const infoPage = (
    <>
      {properties && props && (
        <BackstageProperties
          title={t('backstage.info.properties')}
          fields={[
            ...EDITABLE_DOC_PROPERTIES.map((k) => ({
              id: k,
              label: t(`backstage.props.${k}`),
              value: props[k],
              multiline: k === 'comments',
            })),
            ...readOnly.map((k) => ({
              id: k,
              label: t(`backstage.props.${k}`),
              value: k === 'created' || k === 'modified' ? date(props[k], i18n.language) : props[k],
              readOnly: true,
            })),
          ]}
          onChange={saveProps}
        />
      )}
      <BackstageCommands
        title={t('backstage.info.manage')}
        commands={list(BACKSTAGE_COMMANDS.info)}
      />
    </>
  );

  const optionCommands = list(BACKSTAGE_COMMANDS.options);
  const optionsPage = (
    <>
      <BackstageCommands title={t('backstage.options.general')} commands={optionCommands} />
      {!commands.has('view.density') && (
        <div>
          <label>
            {t('backstage.options.density')}{' '}
            <select
              data-testid="backstage-density"
              value={density}
              onChange={(e) => {
                const d = e.target.value as (typeof DENSITIES)[number];
                setDensity(d);
                setDensityState(d);
              }}
            >
              {DENSITIES.map((d) => (
                <option key={d} value={d}>
                  {t(`backstage.options.densities.${d}`)}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
    </>
  );

  const sections: BackstageSection[] = [
    {
      id: 'new',
      label: t('backstage.nav.new'),
      content: <BackstageCommands commands={list(BACKSTAGE_COMMANDS.new)} />,
    },
    {
      id: 'open',
      label: t('backstage.nav.open'),
      content: (
        <>
          <BackstageCommands
            title={t('backstage.open.browse')}
            commands={list(BACKSTAGE_COMMANDS.browse)}
          />
          <BackstageFiles
            title={t('backstage.open.recent')}
            files={files}
            empty={t('backstage.open.noRecent')}
            canOpen={!!p.openPath}
            onOpen={openFile}
            onTogglePin={(f) => toggle({ name: f.name, path: f.id })}
            pinLabel={t('backstage.open.pin')}
            unpinLabel={t('backstage.open.unpin')}
          />
        </>
      ),
    },
    {
      id: 'info',
      label: t('backstage.nav.info'),
      enabled: hasDocument,
      content: infoPage,
    },
    {
      id: 'save',
      label: t('backstage.nav.save'),
      command: 'file.save',
      enabled: commands.has('file.save') && commands.enabled('file.save'),
      onSelect: () => commands.run('file.save'),
    },
    {
      id: 'saveAs',
      label: t('backstage.nav.saveAs'),
      enabled: hasDocument,
      content: (
        <BackstageFormats
          groups={[
            { title: t('backstage.saveAs.types'), formats: types },
            { title: t('backstage.saveAs.other'), formats: others },
          ]}
        />
      ),
    },
    {
      id: 'print',
      label: t('backstage.nav.print'),
      enabled: hasDocument,
      content: <BackstageCommands commands={list(BACKSTAGE_COMMANDS.print)} />,
    },
    {
      id: 'share',
      label: t('backstage.nav.share'),
      enabled: hasDocument,
      content: (
        <BackstageCommands
          empty={t('backstage.share.unavailable')}
          commands={
            p.onShare
              ? [
                  {
                    id: 'host.share',
                    label: t('host.share'),
                    description: t('backstage.share.describe'),
                    onSelect: () => {
                      onClose();
                      p.onShare?.();
                    },
                  },
                ]
              : []
          }
        />
      ),
    },
    {
      id: 'export',
      label: t('backstage.nav.export'),
      enabled: hasDocument,
      content: <BackstageCommands commands={list(BACKSTAGE_COMMANDS.export)} />,
    },
    ...(p.onCloseDocument
      ? [
          {
            id: 'close',
            label: t('backstage.nav.close'),
            command: 'host.close',
            onSelect: p.onCloseDocument,
          },
        ]
      : []),
    {
      id: 'options',
      label: t('backstage.nav.options'),
      bottom: true,
      content: optionsPage,
    },
  ];
  return (
    <Backstage
      open={open}
      onClose={onClose}
      sections={sections}
      page={page}
      onPageChange={setPage}
      heading={p.fileName}
      labels={{ title: t('ribbon.tabs.file'), back: t('backstage.back') }}
    />
  );
}

/** i18n key of a command's description (`file.new` → `backstage.describe.file_new`). */
const describeKey = (id: string) => `backstage.describe.${id.replace('.', '_')}`;

/** A W3CDTF date of the properties as a local date and time. */
function date(v: string, lang: string): string {
  if (!v) return '';
  const d = new Date(v);
  return Number.isNaN(d.getTime()) ? v : d.toLocaleString(lang);
}
