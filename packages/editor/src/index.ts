import '@blueoffice/ui/theme.css';

export { BlueEditor, type BlueEditorProps, type HostConfig } from './BlueEditor';
export { CommandRegistry, normalizeShortcut, type Command } from './commands';
export { DOC_REPEATABLE, RepeatTracker, SLIDE_REPEATABLE } from './repeat';
export {
  SHORTCUTS_FORMAT,
  assignShortcut,
  exportShortcuts,
  importShortcuts,
  loadShortcuts,
  saveShortcuts,
  shortcutFromKey,
  shortcutsKey,
  validShortcut,
  type ShortcutApp,
  type ShortcutMap,
} from './shortcuts';
export {
  ShortcutsDialog,
  useCustomShortcuts,
  type CustomShortcutsOptions,
  type ShortcutCommand,
  type ShortcutsDialogProps,
} from './ShortcutsDialog';
export {
  CUSTOMIZATION_FORMAT,
  CUSTOMIZATION_VERSION,
  exportCustomization,
  importCustomization,
  loadCustomization,
  ribbonKey,
  saveCustomization,
  type ImportResult,
  type UiCustomization,
} from './ribbonCustomization';
export {
  customizeCommands,
  RibbonCustomizeDialog,
  useRibbonCustomization,
  type CustomizeCommand,
  type CustomizePage,
  type RibbonCustomizationOptions,
  type RibbonCustomizeDialogProps,
} from './RibbonCustomizeDialog';
export { commandLabel, shortcutCommands, useContextMenu, type OpenContextMenu } from './commandUi';
export {
  SCRIPT_COMMAND_PREFIX,
  bindScripts,
  scriptShortcutConflict,
  useScriptBindings,
  validScriptShortcut,
} from './scriptBindings';
export { MacroBanner, PasswordDialog, SavePasswordPanel } from './SecurityPanels';
export { SaveAsTypeDialog, planOpen, saveName, type OpenPlan } from './FileVariants';
export {
  BACKSTAGE_COMMANDS,
  FileBackstage,
  OTHER_FORMATS,
  type BackstageCommandAccess,
  type BackstagePageId,
  type BackstagePropertiesAccess,
  type FileBackstageProps,
} from './FileBackstage';
export { PdfExportDialog, type PdfExportDialogProps } from './PdfExportDialog';
export {
  FileProtectionUi,
  ProtectDialog,
  ProtectionBar,
  ProtectionStatus,
  OpenProtectionDialog,
  securityMessage,
  useFileProtection,
  type FileProtection,
  type RunSecurity,
  type SecurityAnswer,
} from './ProtectionPanels';
export type { EditorState } from './store';
export { SymbolPanel, useSymbolPickerLabels } from './SymbolPanel';
export { IconPanel, useIconLibraryLabels } from './IconPanel';
export {
  insertDocIcons,
  insertSheetIcons,
  insertSlideIcons,
  type DocIconTarget,
  type IconRaster,
  type SlideIconTarget,
} from './insertIcons';
export { CheckDocumentDialog, type CheckDocumentBridge, type CheckTab } from './CheckDocument';
export { DocCheckDocumentDialog, docCheckBridge } from './DocCheckDocument';
export { ClipboardPanel, previewText } from './clipboard/ClipboardPanel';
export { taskPaneLabels } from './taskPaneLabels';
export {
  VersionsPane,
  versionDate,
  type ChangeRow,
  type CompareOutcome,
} from './versions/VersionsPane';
export {
  CLIP_HISTORY_MAX,
  ClipboardHistory,
  clipboardHistory,
  useClipboardHistory,
  type ClipApp,
  type ClipItem,
  type ClipKind,
} from './clipboard/history';
export {
  PASTE_OPTIONS_SHORTCUT,
  PasteOptionsButton,
  type PasteOptionItem,
  type PasteOptionsProps,
} from './clipboard/PasteOptions';
export { fromBase64, toBase64 } from './clipboard/docPaste';
export { pictureTabLabels, presetName, registerPictureCommands } from './pictureTools';
export { objectPlatform, objectToolsLabels, registerObjectCommands } from './objectTools';
// Spelling shared with BlueSheet / BlueSlide (CORE-SPELL-006).
export {
  SpellMenu,
  loadIgnoredWords,
  loadUserWords,
  saveIgnoredWord,
  saveUserWord,
} from './SpellMenu';
export { PROOF_LANGUAGES } from './SpellCheckDialog';
export {
  ProofWalkDialog,
  nextWalkIssue,
  posBefore,
  type ProofWalkActions,
  type WalkIssue,
} from './ProofWalkDialog';
// Print panel shared by the three apps (CORE-PRINT-001).
export {
  PrintPanel,
  jobOptions,
  PER_SHEET,
  PAPERS,
  type PrintApp,
  type PrintPanelProps,
} from './print/PrintPanel';
// Read aloud shared with BlueSheet / BlueSlide (CORE-LANG-003).
export {
  READ_RATES,
  ReadAloudSession,
  type ReadAloudListener,
  type ReadAloudSettings,
  type ReadAloudState,
  type ReadPosition,
  type ReadUnit,
} from './readAloud';
export {
  useReadAloud,
  type ReadAloud,
  type ReadAloudRequest,
  type ReadUnitKind,
} from './ReadAloudBar';
export {
  READ_ALOUD_COMMANDS,
  READ_ALOUD_SHORTCUT,
  ReadAloudLink,
  registerReadAloudCommands,
} from './readAloudCommands';
export { docReadRequest, docUnits, type SpeechHighlighter } from './docReadAloud';
// SmartArt dialog shared with BlueSheet / BlueSlide (CORE-SMART-005).
export {
  SMARTART_LAYOUTS,
  SmartArtDialog,
  itemsOf as smartArtItemsOf,
  textOf as smartArtTextOf,
  type SmartArtDraft,
} from './SmartArtDialog';
// Translation shared with BlueSheet / BlueSlide (CORE-LANG-001).
export {
  useTranslate,
  type Translate,
  type TranslateAction,
  type TranslateOpen,
  type TranslateRun,
  type TranslateSource,
} from './TranslatePane';
export { TranslateLink, registerTranslateCommands } from './translateCommands';
export {
  docInsertAfter,
  docSelectionText,
  translateDocumentSegments,
  translatedName,
} from './docTranslate';
// Dictation shared with BlueSheet / BlueSlide (CORE-LANG-002).
export {
  DICTATION_COMMANDS,
  DictationSession,
  applyTokens,
  composeDictation,
  diffEdit,
  normalizeWord,
  parseDictation,
  sentenceStart,
  type ComposeOptions,
  type DictationBreaks,
  type DictationEdit,
  type DictationListener,
  type DictationState,
  type DictationTarget,
  type DictationToken,
} from './dictation';
export { useDictation, type Dictation } from './DictationBar';
export {
  DICTATE_SHORTCUT,
  DICTATION_COMMANDS_IDS,
  DictationLink,
  registerDictationCommands,
} from './dictationCommands';
export { DOC_LINE_BREAK, docDictationTarget, type DictationPreviewer } from './docDictation';
