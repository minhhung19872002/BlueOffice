/**
 * Read-only view of the VBA macro source of the open file (CORE-VBA-002).
 * BlueOffice never runs VBA: the project is kept byte for byte when saving and its
 * source text is only read ([MS-OVBA]) so the user can look at it and turn a module
 * into a BlueScript draft.
 */

/** `standard`: procedural module · `document`: module of the document, workbook, sheet or slide. */
export type VbaModuleKind = 'standard' | 'document' | 'class' | 'form';

export interface VbaModuleInfo {
  name: string;
  kind: VbaModuleKind;
  /** Source text with `\n` line ends. */
  source: string;
  /** The module could not be read (damaged or over the size limits). */
  unreadable?: boolean;
}

/** Answer of the `vbaProject` command. `modules` is empty when the file has no VBA project. */
export interface VbaProjectInfo {
  name: string;
  modules: VbaModuleInfo[];
  /** Code page of the project's text. */
  codePage: number;
  /** False: the code page is not decoded by BlueOffice and accented characters may be wrong. */
  codePageSupported: boolean;
  /** Modules or sources were left out because of the size limits. */
  truncated?: boolean;
  /** The project is present but could not be read. */
  error?: string;
}

/**
 * Accepted by `EngineHost.command` (BlueDoc), `EngineHost.slideCommand` (BlueSlide) and
 * `SheetHost.exec` (BlueSheet); answered with a `VbaProjectInfo`. It changes nothing.
 */
export type VbaCommand = { cmd: 'vbaProject' };
