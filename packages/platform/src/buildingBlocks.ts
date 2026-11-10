/**
 * BlueDoc building blocks (DOC-BB-001) over the JSON command bridge: the entries of
 * the document's glossary part, of the user's building blocks file and of the library
 * shipped with BlueDoc (mirrors `bo_engine::blocks`, ECMA-376 Part 1, §17.12).
 */

/** Where a block is kept. */
export type BlockStore = 'document' | 'user' | 'builtin';

/** §17.18.14 `ST_DocPartBehavior`: in line, in paragraphs of its own, on a page of its own. */
export type BlockBehavior = 'content' | 'p' | 'pg';

/** Galleries offered when saving a block (§17.18.15 `ST_DocPartGallery`). */
export const BLOCK_GALLERIES = [
  'autoTxt',
  'docParts',
  'hdrs',
  'ftrs',
  'pgNum',
  'coverPg',
  'txtBox',
  'tbls',
  'eq',
  'watermarks',
  'bib',
  'custom1',
  'custom2',
  'custom3',
  'custom4',
  'custom5',
] as const;

/** Properties of a block as the dialogs edit them. */
export interface BlockSpec {
  name: string;
  /** `ST_DocPartGallery` value. */
  gallery: string;
  category: string;
  description: string;
  behavior: BlockBehavior;
}

/** One block of `blocks.list`. */
export interface BlockInfo extends BlockSpec {
  /** Key for the other commands (`doc:3`, `user:0`, `builtin:12`). */
  key: string;
  store: BlockStore;
  /** Beginning of the block's text. */
  text: string;
}

/** Answer of a building block command (`EditResult.blocks`). */
export interface BlocksOut {
  list: BlockInfo[];
  /** The user's building blocks file after a change to it (`.dotx`, base64): the host stores it. */
  store?: string;
  /** Preview picture (PNG, base64). */
  preview?: string;
  /** Name of the block an insert / F3 placed. */
  inserted?: string;
}

export type BuildingBlockCommand =
  /** Every block (document, user, library) → `blocks.list`. */
  | { cmd: 'buildingBlocks' }
  /** Save the selection as a block. */
  | { cmd: 'saveBuildingBlock'; spec: BlockSpec; store: 'document' | 'user' }
  /** Insert a block: at the caret or — by its gallery — as header / footer of `page`, or as cover page. */
  | { cmd: 'insertBuildingBlock'; key: string; page?: number }
  | { cmd: 'editBuildingBlock'; key: string; spec: BlockSpec }
  | { cmd: 'deleteBuildingBlock'; key: string }
  /** AutoText (F3): the block whose name is typed before the caret replaces it. */
  | { cmd: 'expandAutoText' }
  /** Picture of a block → `blocks.preview`. */
  | { cmd: 'buildingBlockPreview'; key: string }
  /** Load the user's building blocks file (`.dotx`, base64; empty: none). */
  | { cmd: 'loadBlockStore'; file: string };

/** Key of the user's building blocks file in the platform's offline store. */
export const BLOCK_STORE_KEY = 'blueoffice.building-blocks.dotx';

/** Galleries a block list shows for a ribbon gallery. */
export const BLOCK_GALLERY_GROUPS = {
  quickParts: [
    'docParts',
    'autoTxt',
    'custQuickParts',
    'custAutoTxt',
    'custom1',
    'custom2',
    'custom3',
    'custom4',
    'custom5',
    'eq',
    'bib',
    'watermarks',
  ],
  header: ['hdrs', 'custHdrs'],
  footer: ['ftrs', 'custFtrs'],
  pageNumber: [
    'pgNumT',
    'pgNumB',
    'pgNum',
    'pgNumMargins',
    'custPgNum',
    'custPgNumT',
    'custPgNumB',
  ],
  coverPage: ['coverPg', 'custCoverPg'],
  textBox: ['txtBox', 'custTxtBox'],
  table: ['tbls', 'custTbls'],
} as const satisfies Record<string, readonly string[]>;

export type BlockGalleryGroup = keyof typeof BLOCK_GALLERY_GROUPS;

/** Blocks of a ribbon gallery, the document's first, then the user's, then the library's. */
export function blocksOfGroup(list: readonly BlockInfo[], group: BlockGalleryGroup): BlockInfo[] {
  const galleries: readonly string[] = BLOCK_GALLERY_GROUPS[group];
  const order: Record<BlockStore, number> = { document: 0, user: 1, builtin: 2 };
  return list
    .filter((b) => galleries.includes(b.gallery))
    .sort((a, b) => order[a.store] - order[b.store]);
}
