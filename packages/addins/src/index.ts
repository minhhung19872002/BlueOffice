export { AddinBridge, DEFAULT_RATE_LIMIT, type AddinBridgeOptions, type RateLimit } from './bridge';
export { frameAttributes, type FrameAttributes } from './frame';
export { ADDINS_TAB, mergeAddinTabs, type AddinGroup } from './ribbon';
export {
  MAX_SELECTION_CELLS,
  docSelection,
  sheetSelection,
  slideSelection,
  type SheetSelectionState,
} from './selection';
export {
  STORAGE_LIMITS,
  STORAGE_PREFIX,
  AddinStorageError,
  addinStorage,
  defaultBacking,
  memoryBacking,
  type AddinStorage,
  type KeyValueBacking,
} from './storage';
export {
  CATALOGUE_PANE,
  MANAGE_COMMAND,
  addinCommandId,
  addinPaneId,
  useAddins,
  type AddinCommandRegistry,
  type AddinEntry,
  type Addins,
  type UseAddinsOptions,
} from './useAddins';
