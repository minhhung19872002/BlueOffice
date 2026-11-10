/**
 * Reads entries of a ZIP archive (APPNOTE 6.3.x: end of central directory,
 * central directory headers, local headers) — enough for OOXML packages:
 * stored and deflated entries, no ZIP64, no encryption. Inflating uses the
 * platform's `DecompressionStream('deflate-raw')`.
 */

interface Entry {
  method: number;
  compressedSize: number;
  localOffset: number;
}

const u16 = (b: Uint8Array, o: number) => b[o]! | (b[o + 1]! << 8);
const u32 = (b: Uint8Array, o: number) =>
  (b[o]! | (b[o + 1]! << 8) | (b[o + 2]! << 16) | (b[o + 3]! << 24)) >>> 0;

/** Central directory of `zip`: entry name → where its data is. */
function directory(zip: Uint8Array): Map<string, Entry> {
  const out = new Map<string, Entry>();
  // End of central directory record: signature 0x06054b50, at most 64 KiB of comment after it.
  let eocd = -1;
  for (let i = zip.length - 22; i >= 0 && i >= zip.length - 22 - 0xffff; i--) {
    if (u32(zip, i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) return out;
  const count = u16(zip, eocd + 10);
  let p = u32(zip, eocd + 16);
  const dec = new TextDecoder();
  for (let n = 0; n < count && p + 46 <= zip.length; n++) {
    if (u32(zip, p) !== 0x02014b50) break;
    const nameLen = u16(zip, p + 28);
    const name = dec.decode(zip.subarray(p + 46, p + 46 + nameLen));
    out.set(name, {
      method: u16(zip, p + 10),
      compressedSize: u32(zip, p + 20),
      localOffset: u32(zip, p + 42),
    });
    p += 46 + nameLen + u16(zip, p + 30) + u16(zip, p + 32);
  }
  return out;
}

async function inflateRaw(data: Uint8Array): Promise<Uint8Array> {
  const ds = new DecompressionStream('deflate-raw');
  const writer = ds.writable.getWriter();
  // Errors surface on the reading side.
  void writer.write(data as BufferSource).catch(() => undefined);
  void writer.close().catch(() => undefined);
  const chunks: Uint8Array[] = [];
  const reader = ds.readable.getReader();
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    const chunk = value as Uint8Array;
    chunks.push(chunk);
    total += chunk.length;
  }
  const out = new Uint8Array(total);
  let at = 0;
  for (const c of chunks) {
    out.set(c, at);
    at += c.length;
  }
  return out;
}

/** The entries `names` of `zip` (absent or unreadable entries are left out). */
export async function unzipEntries(
  zip: Uint8Array,
  names: string[],
): Promise<Map<string, Uint8Array>> {
  const dir = directory(zip);
  const out = new Map<string, Uint8Array>();
  for (const name of names) {
    const e = dir.get(name);
    if (!e || e.localOffset + 30 > zip.length || u32(zip, e.localOffset) !== 0x04034b50) continue;
    const start = e.localOffset + 30 + u16(zip, e.localOffset + 26) + u16(zip, e.localOffset + 28);
    const data = zip.subarray(start, start + e.compressedSize);
    if (e.method === 0) out.set(name, data);
    else if (e.method === 8) {
      try {
        out.set(name, await inflateRaw(data));
      } catch {
        // A damaged entry: treated as absent.
      }
    }
  }
  return out;
}
