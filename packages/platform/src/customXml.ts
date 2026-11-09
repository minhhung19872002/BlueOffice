/**
 * Custom XML parts and content control data binding (ECMA-376 Part 1, §15.2.4, §15.2.5,
 * §17.5.2.6) — the BlueDoc XML mapping pane's engine commands (`customXml*`, mirroring
 * `bo_engine::custom_xml::CustomXmlCommand`).
 */

/** An element or attribute of a part (mirrors `bo_model::custom_xml::xpath::OutlineNode`). */
export interface CustomXmlNode {
  /** Qualified name as written (`@name` for an attribute). */
  name: string;
  /** Binding path (every step with its position). */
  xpath: string;
  /** Depth below the document element. */
  depth: number;
  /** Value of a leaf element or attribute. */
  value?: string;
}

/** A custom XML part (mirrors `bo_edit::custom_xml::CustomXmlInfo`). */
export interface CustomXmlPartInfo {
  index: number;
  partName: string;
  itemId?: string;
  /** Qualified name of the document element. */
  root: string;
  /** Namespace of the document element (`''`: none). */
  namespace: string;
  schemaRefs: string[];
  /** Not well-formed: no nodes listed. */
  broken: boolean;
  nodes: CustomXmlNode[];
  /** `w:prefixMappings` for the node paths. */
  prefixMappings: string;
  /** Content controls bound to this part. */
  boundControls: number;
}

/**
 * Custom XML commands. Errors come back in `EditResult.error` as message keys: `badXml`,
 * `badPart`, `badXPath`, `noNode`, `notLeaf`, `noControl`, `locked`, `notBindable`,
 * `tooLarge`, `protected`.
 */
export type CustomXmlCommand =
  | { cmd: 'customXmlParts' | 'customXmlUnbind' }
  | { cmd: 'customXmlAdd'; xml: string }
  | { cmd: 'customXmlReplace'; index: number; xml: string }
  | { cmd: 'customXmlRemove'; index: number }
  | {
      cmd: 'customXmlSetValue';
      index: number;
      xpath: string;
      prefixMappings: string;
      value: string;
    }
  | { cmd: 'customXmlBind'; index: number; xpath: string; prefixMappings: string };
