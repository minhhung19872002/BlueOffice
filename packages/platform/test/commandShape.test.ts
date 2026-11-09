import { describe, expect, it } from 'vitest';
import type { SheetCommand } from '../src/sheet';

// Hosts such as the collaboration layer take commands as `{ cmd: string; [k: string]: unknown }`.
// An `interface` intersected into a command type is not assignable to that index signature
// (type aliases are), which breaks `SheetHost` → `SheetLike` at type level. Members that do not
// fit are collected here; the assertion fails to compile when one appears.
type NotIndexable<T> = T extends unknown ? (T extends { [k: string]: unknown } ? never : T) : never;
type Offenders = NotIndexable<SheetCommand>;
const noOffenders: [Offenders] extends [never] ? true : false = true;

describe('command shapes', () => {
  it('every sheet command fits a generic `{ cmd: string; [k: string]: unknown }`', () => {
    expect(noOffenders).toBe(true);
  });
});
