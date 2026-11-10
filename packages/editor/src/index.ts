import '@blueoffice/ui/theme.css';

export { BlueEditor, type BlueEditorProps, type HostConfig } from './BlueEditor';
export { CommandRegistry, type Command } from './commands';
export { MacroBanner, PasswordDialog, SavePasswordPanel } from './SecurityPanels';
export { SaveAsTypeDialog, planOpen, saveName, type OpenPlan } from './FileVariants';
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
export { CheckDocumentDialog, type CheckDocumentBridge, type CheckTab } from './CheckDocument';
export { DocCheckDocumentDialog, docCheckBridge } from './DocCheckDocument';
export { ClipboardPanel, previewText } from './clipboard/ClipboardPanel';
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
