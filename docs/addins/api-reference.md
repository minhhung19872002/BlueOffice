# BlueOffice add-in API reference (protocol v1)

An add-in is a web page that BlueDoc, BlueSheet or BlueSlide loads in a sandboxed frame of a task pane
(CORE-ADDIN-001, [ADR-0013](../adr/0013-web-addins.md)). It talks to the editor only through `postMessage`, using the
client library below. Administrators publish add-ins to the organisation catalogue
([deployment guide, Vietnamese](../deploy/addins.md)); users install them and consent to their permissions.

## Loading the client

```html
<script src="https://<blueoffice-server>/addin-sdk/v1.js"></script>
<script src="app.js"></script>
```

`v1.js` defines the global `BlueOfficeAddin` (`connect`, `AddinClient`, `AddinError`, `ERROR_CODES`,
`PROTOCOL_VERSIONS`, `validateManifest`). With a bundler, use the package `@blueoffice/addin-sdk` instead
(`import { connect } from '@blueoffice/addin-sdk'`).

You can also serve a copy of `v1.js` from your own origin (`<script src="addin-sdk.js">`). Do so when the editor
runs on a private or loopback address and the add-in on a public one: browsers block such loads (Private Network
Access).

Inline scripts are blocked by the frame's Content Security Policy: keep code in files on your own origin. Your server
must accept the policy (`Allow-CSP-From: <editor origin>` or `*`).

```js
const bo = await BlueOfficeAddin.connect();
console.log(bo.app, bo.info.permissions); // 'doc', ['selection', 'write-document']
const sel = await bo.selection.get();
if (bo.app === 'doc') await bo.doc.insertText(`[${sel.text}]`);
bo.on('selectionChanged', () => refresh());
bo.on('command', (e) => e.command === 'insertNo' && insertNumber());
```

## `connect(options?) → Promise<AddinClient>`

Sends `addin.hello` and resolves once the editor answers. Options: `target` (default `window.parent`), `source`
(default `window`), `targetOrigin` (default `'*'`), `timeoutMs` per call (default 30 000).

### `AddinClient`

| Member | Description |
|---|---|
| `app` | `'doc'`, `'sheet'` or `'slide'` |
| `info` | Handshake result: `protocol`, `app`, `addinId`, `version`, `permissions` (granted), `language` (`vi-VN` / `en-US`), `methods` (callable here), `hasDocument` |
| `has(permission)` | Whether a permission was granted |
| `call(method, ...params)` | Any method below by name |
| `on(type, handler)` | Subscribe to an event; returns the unsubscribe function |
| `close()` | Stop listening; pending calls reject |

Every call returns a promise that rejects with `AddinError { code, message, missing }` on failure.

## Permissions

| Permission | Grants |
|---|---|
| `read-document` | Reading the whole document (`doc.text`, `sheet.getValues`, `slide.shapes` …) and `documentChanged` events |
| `write-document` | Changing the document; each call is one undo step |
| `selection` | `selection.get()`, reading / setting the BlueDoc selection, `selectionChanged` events |
| `ui:ribbon` | Ribbon commands of the manifest and `command` events |
| `storage` | `storage.*` (this add-in's own key/value store on the user's device) |
| `network:<origin>` | The page may connect (`fetch`, WebSocket) to `<origin>` |

Raw engine commands (`doc.command`, `sheet.command`, `slide.command`) need both `read-document` and
`write-document`.

## Methods

The document methods are the BlueScript object API (`packages/script/bluescript.d.ts`), one host method each.
Positions in BlueDoc are a paragraph index and a UTF-16 offset in its text. In BlueSheet, `sheet` is a 0-based index or
`-1` for the active sheet, ranges are A1 addresses (`B2:C4`, `'Sheet 2'!A1`). BlueSlide coordinates are points.

### Add-in services

| Method | Client | Permission | Result |
|---|---|---|---|
| `addin.hello` | (by `connect`) | — | `HelloResult` |
| `addin.selection` | `selection.get()` | `selection` | doc: `{ app, anchor, focus, text }`; sheet: `{ app, sheet, sheetName, address, active, values }` (values up to 10 000 cells); slide: `{ app, slide, shapes: number[] }` |
| `storage.get` | `storage.get(key)` | `storage` | the value or `null` |
| `storage.set` | `storage.set(key, value)` | `storage` | — (≤ 1 MB in total, ≤ 500 keys, keys ≤ 200 chars) |
| `storage.remove` | `storage.remove(key)` | `storage` | — |
| `storage.keys` | `storage.keys()` | `storage` | `string[]` |

### BlueDoc (`bo.doc`)

| Method | Permission |
|---|---|
| `doc.info()`, `doc.text()`, `doc.paragraphs()`, `doc.paragraphStyle(i)`, `doc.find(query, options)` | `read-document` |
| `doc.selection()`, `doc.select(p, start, end)`, `doc.selectAll()` | `selection` |
| `doc.insertText(text)`, `doc.format(f)`, `doc.replace(q, with, options)`, `doc.appendParagraph(text, style)`, `doc.insertParagraphAfter(i, text, style)`, `doc.setParagraphText(i, text)`, `doc.setParagraphStyle(i, id)`, `doc.formatParagraph(i, f)`, `doc.setParagraphFormat(i, f)`, `doc.deleteParagraph(i)`, `doc.insertTable(rows, cols, values)`, `doc.insertPageBreak()` | `write-document` |

### BlueSheet (`bo.workbook`)

| Method | Permission |
|---|---|
| `sheet.list` (`sheets()`), `sheet.active` (`activeSheet()`), `sheet.getValues`, `sheet.getFormulas`, `sheet.charts` | `read-document` |
| `sheet.setValues`, `sheet.setFormulas`, `sheet.fill`, `sheet.format`, `sheet.clear`, `sheet.merge`, `sheet.borders`, `sheet.addChart`, `sheet.add` (`addSheet`), `sheet.rename` (`renameSheet`) | `write-document` |

### BlueSlide (`bo.presentation`)

| Method | Permission |
|---|---|
| `slide.info`, `slide.shapes`, `slide.getNotes` (`notes`) | `read-document` |
| `slide.addSlide`, `slide.deleteSlide`, `slide.duplicateSlide`, `slide.setNotes`, `slide.addTextBox`, `slide.addShape`, `slide.addTable`, `slide.addChart`, `slide.setText`, `slide.formatText`, `slide.setFill`, `slide.setLine`, `slide.move`, `slide.deleteShape` | `write-document` |

Methods of another application answer `methodNotFound`. Use `bo.call(name, ...args)` for methods without a typed
wrapper.

## Events

| Event | Sent when | Needs |
|---|---|---|
| `selectionChanged` | The user's selection changed | `selection` |
| `documentChanged` | The document changed | `read-document` |
| `command` (`{ command }`) | A ribbon command of the manifest with `"action": "event"` was clicked (the pane opens first; the event waits for the handshake) | `ui:ribbon` |

Events carry no document content: read what you need with a call.

## Error codes

| Code | Name | Meaning |
|---|---|---|
| -32600 | `invalidRequest` | Malformed message |
| -32601 | `methodNotFound` | Unknown method, or one of another application |
| -32602 | `invalidParams` | Parameters are not plain JSON, or a storage limit was hit |
| -32603 | `internal` | No answer in time, or the client was closed |
| -32001 | `permissionDenied` | A permission is missing (`error.missing` lists them) |
| -32002 | `rateLimited` | More than 20 calls per second (bursts of 60) |
| -32003 | `apiError` | The document API refused the call (bad address, no such paragraph …) |
| -32004 | `unsupportedVersion` | No common protocol version |
| -32005 | `noDocument` | No document is open |
| -32006 | `notReady` | A call before `addin.hello` |

## Wire format

For other languages or frameworks: every message is a JSON-RPC 2.0 object in an envelope.

```json
{ "tag": "bo-addin", "v": 1, "jsonrpc": "2.0", "id": 7, "method": "doc.insertText", "params": ["Hello"] }
{ "tag": "bo-addin", "v": 1, "jsonrpc": "2.0", "id": 7, "result": null }
{ "tag": "bo-addin", "v": 1, "jsonrpc": "2.0", "id": 8, "error": { "code": -32001, "message": "…", "data": { "missing": ["read-document"] } } }
{ "tag": "bo-addin", "v": 1, "jsonrpc": "2.0", "method": "event", "params": [{ "type": "selectionChanged" }] }
```

Requests go to `window.parent` with `targetOrigin '*'`; the first one must be `addin.hello` with
`[{ "protocols": [1] }]`. At most 16 parameters and 2 MB of JSON per message.

## Manifest

Schema: [`packages/addin-sdk/manifest.schema.json`](../../packages/addin-sdk/manifest.schema.json); an example and the
rules are in the [deployment guide](../deploy/addins.md#manifest). `validateManifest(json)` returns
`{ ok: true, manifest }` or `{ ok: false, errors: [{ path, message }] }` with the same rules the server applies.
