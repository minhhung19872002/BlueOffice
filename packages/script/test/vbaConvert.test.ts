import { describe, expect, it } from 'vitest';
import type { ScriptHost } from '../src/host';
import { inlineRunner } from '../src/runner';
import type { ScriptApp } from '../src/sandbox';
import { convertVba, TODO_MARK, type VbaWarningReason } from '../src/vbaConvert';
import { tokenize } from '../src/vbaConvert/lexer';
import { parseVba } from '../src/vbaConvert/parser';

// CORE-SCRIPT-003: VBA → BlueScript draft. Every draft must parse as JavaScript
// (the body of an async function, as the sandbox wraps a script).

const AsyncFunction = Object.getPrototypeOf(async () => undefined).constructor as new (
  ...args: string[]
) => (...args: unknown[]) => Promise<unknown>;

/** Throws a SyntaxError when `code` is not valid script source. */
const parses = (code: string) => new AsyncFunction(`"use strict";\n${code}`);

function convert(source: string, app: ScriptApp = 'sheet') {
  const r = convertVba(source, { app });
  expect(() => parses(r.code), r.code).not.toThrow();
  // Each warning has its TODO line in the draft, and the reverse.
  const todos = r.code.split('\n').filter((l) => l.includes(TODO_MARK)).length;
  expect(todos, r.code).toBeGreaterThanOrEqual(new Set(r.warnings.map((w) => w.source)).size);
  if (r.warnings.length === 0) expect(r.code).not.toContain(TODO_MARK);
  return r;
}

/** Draft lines, without indentation and blank lines. */
const lines = (code: string) =>
  code
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean);

/** The statements of the draft of `Sub t … End Sub` with `body`. */
function body(vba: string, app: ScriptApp = 'sheet') {
  const r = convert(`Sub t()\n${vba}\nEnd Sub`, app);
  const all = lines(r.code);
  const from = all.indexOf('async function t() {');
  const to = all.lastIndexOf('}');
  return { lines: all.slice(from + 1, to), warnings: r.warnings, code: r.code };
}

describe('convertVba: statements', () => {
  const cases: { name: string; vba: string; js: string[]; app?: ScriptApp }[] = [
    {
      name: 'Dim with types and an assignment',
      vba: 'Dim n As Long, s As String, ok As Boolean, v\nn = 5\ns = "a""b"\nv = n',
      js: [
        'let n = 0;',
        'let s = "";',
        'let ok = false;',
        'let v;',
        'n = 5;',
        's = "a\\"b";',
        'v = n;',
      ],
    },
    {
      name: 'Const in place',
      vba: 'Const RATE As Double = 0.1\nDim x\nx = RATE * 2',
      js: ['let x;', 'const RATE = 0.1;', 'x = RATE * 2;'],
    },
    {
      name: 'undeclared variables are declared',
      vba: 'total = total + 1',
      js: ['let total;', 'total = total + 1;'],
    },
    {
      name: 'If / ElseIf / Else',
      vba: 'Dim x As Long\nIf x > 1 Then\nx = 1\nElseIf x = 0 Then\nx = 2\nElse\nx = 3\nEnd If',
      js: [
        'let x = 0;',
        'if (x > 1) {',
        'x = 1;',
        '} else if (x == 0) {',
        'x = 2;',
        '} else {',
        'x = 3;',
        '}',
      ],
    },
    {
      name: 'single-line If with Else and a colon',
      vba: 'Dim x As Long, y As Long\nIf x = 1 Then y = 2: x = 3 Else y = 4',
      js: [
        'let x = 0;',
        'let y = 0;',
        'if (x == 1) {',
        'y = 2;',
        'x = 3;',
        '} else {',
        'y = 4;',
        '}',
      ],
    },
    {
      name: 'For / Next with Step and Exit For',
      vba: 'Dim i As Long, s As Long\nFor i = 10 To 1 Step -2\nIf i = 4 Then Exit For\ns = s + i\nNext i',
      js: [
        'let i = 0;',
        'let s = 0;',
        'for (i = 10; i >= 1; i += -2) {',
        'if (i == 4) {',
        'break;',
        '}',
        's = s + i;',
        '}',
      ],
    },
    {
      name: 'For with a computed step',
      vba: 'Dim i, d\nFor i = 1 To 9 Step d\nNext',
      js: ['let i;', 'let d;', 'for (i = 1; d >= 0 ? i <= 9 : i >= 9; i += d) {', '}'],
    },
    {
      name: 'For Each over an array',
      vba: 'Dim v, items\nitems = Array(1, 2, 3)\nFor Each v In items\nDebug.Print v\nNext',
      js: [
        'let v;',
        'let items;',
        'items = [1, 2, 3];',
        'for (v of items) {',
        'console.log(v);',
        '}',
      ],
    },
    {
      name: 'Do While / Loop and Exit Do',
      vba: 'Dim i As Long\nDo While i < 3\ni = i + 1\nIf i = 2 Then Exit Do\nLoop',
      js: ['let i = 0;', 'while (i < 3) {', 'i = i + 1;', 'if (i == 2) {', 'break;', '}', '}'],
    },
    {
      name: 'Do Until / Loop',
      vba: 'Dim i As Long\nDo Until i >= 3\ni = i + 1\nLoop',
      js: ['let i = 0;', 'while (!(i >= 3)) {', 'i = i + 1;', '}'],
    },
    {
      name: 'Do / Loop While (condition after the body)',
      vba: 'Dim i As Long\nDo\ni = i + 1\nLoop While i < 3',
      js: ['let i = 0;', 'do {', 'i = i + 1;', '} while (i < 3);'],
    },
    {
      name: 'Do / Loop Until',
      vba: 'Dim i As Long\nDo\ni = i + 1\nLoop Until i = 3',
      js: ['let i = 0;', 'do {', 'i = i + 1;', '} while (!(i == 3));'],
    },
    {
      name: 'While / Wend',
      vba: 'Dim i As Long\nWhile i < 3\ni = i + 1\nWend',
      js: ['let i = 0;', 'while (i < 3) {', 'i = i + 1;', '}'],
    },
    {
      name: 'Select Case with lists, ranges, Is and Else',
      vba:
        'Dim x As Long, s As String\nSelect Case x\nCase 1, 2\ns = "a"\nCase 3 To 5\ns = "b"\n' +
        'Case Is > 9\ns = "c"\nCase Else\ns = "d"\nEnd Select',
      js: [
        'let x = 0;',
        'let s = "";',
        '{',
        'const sel1 = x;',
        'if ((sel1 == 1) || (sel1 == 2)) {',
        's = "a";',
        '} else if (sel1 >= 3 && sel1 <= 5) {',
        's = "b";',
        '} else if (sel1 > 9) {',
        's = "c";',
        '} else {',
        's = "d";',
        '}',
        '}',
      ],
    },
    {
      name: 'operators: & Mod \\ ^ <> Not And Or',
      vba:
        'Dim a As Long, b As Long, s As String, ok As Boolean\ns = "n=" & a & b\na = a Mod 3 + b \\ 2\n' +
        'b = -a ^ 2\nok = Not a <> b And (a < b Or a >= 1)\ns = a & b',
      js: [
        'let a = 0;',
        'let b = 0;',
        'let s = "";',
        'let ok = false;',
        's = "n=" + a + b;',
        'a = a % 3 + Math.trunc(b / 2);',
        'b = -(a ** 2);',
        'ok = !(a != b) && (a < b || a >= 1);',
        's = String(a) + b;',
      ],
    },
    {
      name: 'number, date and constant literals',
      vba: 'Dim x, d, s\nx = &HFF + 1.5E+2 + 007 + 10#\nd = #1/31/2024#\ns = "a" & vbCrLf & vbTab\nx = Nothing',
      js: [
        'let x;',
        'let d;',
        'let s;',
        'x = 0xFF + 150 + 7 + 10;',
        'd = new Date("1/31/2024");',
        's = "a" + "\\n" + "\\t";',
        'x = null;',
      ],
    },
    {
      name: 'comments, Rem and line continuation',
      vba: "' first\nRem second\nDim x As Long ' trailing\nx = 1 + _\n    2",
      js: ['let x = 0;', '// first', '// second', '// trailing', 'x = 1 + 2;'],
    },
    {
      name: 'With block on a range',
      vba: 'With Range("A1")\n.Value = 1\n.Font.Bold = True\nEnd With',
      js: [
        '{',
        'const with1 = sheet.range("A1");',
        'await with1.setValue(1);',
        'await with1.format({ bold: true });',
        '}',
      ],
    },
    {
      name: 'arrays, ReDim Preserve and UBound',
      vba: 'Dim a() As String, n As Long\nReDim a(3)\na(0) = "x"\nReDim Preserve a(n)\nn = UBound(a)',
      js: [
        'let a = [];',
        'let n = 0;',
        'a = [];',
        'a[0] = "x";',
        'a.length = n + 1;',
        'n = a.length - 1;',
      ],
    },
    {
      name: 'string functions',
      vba:
        'Dim s As String, n As Long\nn = Len(s)\ns = Left(s, 2) & Right$(s, 1) & Mid(s, 2, 3)\n' +
        's = UCase(Trim(s))\nn = InStr(s, "x")\ns = Replace(s, "a", "b")\nn = CLng("12") + Int(1.5)',
      js: [
        'let s = "";',
        'let n = 0;',
        'n = String(s).length;',
        's = String(s).slice(0, 2) + vbaRight(s, 1) + vbaMid(s, 2, 3);',
        's = String(s).trim().toUpperCase();',
        'n = vbaInStr(s, "x");',
        's = String(s).replaceAll("a", "b");',
        'n = Math.round(Number("12")) + Math.floor(1.5);',
      ],
    },
    {
      name: 'MsgBox and Debug.Print go to the console',
      vba: 'MsgBox "Done " & 3, vbInformation, "Title"\nMsgBox Prompt:="x"\nDebug.Print "a"; 1, 2',
      js: ['console.log("Done " + 3);', 'console.log("x");', 'console.log("a", 1, 2);'],
    },
    {
      name: 'Exit Sub',
      vba: 'Dim x As Long\nIf x = 0 Then Exit Sub\nx = 1',
      js: ['let x = 0;', 'if (x == 0) {', 'return;', '}', 'x = 1;'],
    },
    {
      name: 'names that are reserved in JavaScript',
      vba: 'Dim class As Long, sheet As Long\nclass = 1\nsheet = class',
      js: ['let class_ = 0;', 'let sheet_ = 0;', 'class_ = 1;', 'sheet_ = class_;'],
    },
    {
      name: 'names are case-insensitive',
      vba: 'Dim Total As Long\ntotal = TOTAL + 1',
      js: ['let Total = 0;', 'Total = Total + 1;'],
    },
  ];
  it.each(cases)('$name', ({ vba, js, app }) => {
    const r = body(vba, app);
    expect(r.lines).toEqual(js);
    expect(r.warnings).toEqual([]);
  });

  it('procedures become async functions; the first macro is called', () => {
    const r = convert(
      [
        'Option Explicit',
        'Private total As Long',
        'Sub Main()',
        '  Dim r As Long',
        '  r = Twice(4)',
        '  Report r, "x"',
        '  Call Report(r)',
        'End Sub',
        'Function Twice(ByVal n As Long) As Long',
        '  If n < 0 Then Exit Function',
        '  Twice = n * 2',
        'End Function',
        'Private Sub Report(value, Optional label As String = "n")',
        '  total = total + value',
        '  Debug.Print label, value',
        'End Sub',
      ].join('\r\n'),
    );
    expect(r.warnings).toEqual([]);
    expect(r.procedures).toEqual(['Main', 'Twice', 'Report']);
    expect(lines(r.code)).toEqual([
      'let total = 0;',
      '// Option Explicit',
      'async function Main() {',
      'let r = 0;',
      'r = await Twice(4);',
      'await Report(r, "x");',
      'await Report(r);',
      '}',
      'async function Twice(n) {',
      'let TwiceResult = 0;',
      'if (n < 0) {',
      'return TwiceResult;',
      '}',
      'TwiceResult = n * 2;',
      'return TwiceResult;',
      '}',
      'async function Report(value, label = "n") {',
      'total = total + value;',
      'console.log(label, value);',
      '}',
      'await Main();',
    ]);
  });

  it('a loose snippet without a procedure converts too', () => {
    const r = convert('x = 2\nRange("B2").Value = x * 2');
    expect(lines(r.code)).toEqual(['let x;', 'x = 2;', 'await sheet.range("B2").setValue(x * 2);']);
  });
});

describe('convertVba: object model', () => {
  const cases: { name: string; app: ScriptApp; vba: string; js: string[] }[] = [
    {
      name: 'Range value and formula',
      app: 'sheet',
      vba:
        'Dim v\nRange("A1").Value = 5\nRange("B1") = "x"\nv = Range("A1").Value\nRange("C1").Formula = "=A1*2"\nv = Range("C1").Formula\n' +
        'If Range("A1").Value <> "" And Not IsEmpty(Range("A2")) Then v = "n=" & Range("A1") & Len(Range("A1").Value)',
      js: [
        'let v;',
        'await sheet.range("A1").setValue(5);',
        'await sheet.range("B1").setValue("x");',
        'v = (await sheet.range("A1").values)[0][0];',
        'await sheet.range("C1").setFormulas([["=A1*2"]]);',
        'v = (await sheet.range("C1").formulas)[0][0];',
        'if (((await sheet.range("A1").values)[0][0] ?? "") != "" && ![null, ""].includes((await sheet.range("A2").values)[0][0])) {',
        'v = "n=" + ((await sheet.range("A1").values)[0][0] ?? "") + String((await sheet.range("A1").values)[0][0] ?? "").length;',
        '}',
      ],
    },
    {
      name: 'Cells with 1-based indexes',
      app: 'sheet',
      vba: 'Dim r As Long\nCells(1, 2).Value = 1\nCells(r, r + 1) = Cells(r, 1) * 2\nCells(3, "C").Value = 0\nRange(Cells(1, 1), Cells(r, 2)).ClearContents\nRange("A1", "B2").Merge',
      js: [
        'let r = 0;',
        'await sheet.cell(0, 1).setValue(1);',
        'await sheet.cell(r - 1, r + 1 - 1).setValue((await sheet.cell(r - 1, 0).values)[0][0] * 2);',
        'await sheet.range("C3").setValue(0);',
        'await sheet.range(sheet.cell(0, 0).address + ":" + sheet.cell(r - 1, 1).address).clear("contents");',
        'await sheet.range("A1:B2").merge();',
      ],
    },
    {
      name: 'ActiveSheet, Worksheets and a typed variable',
      app: 'sheet',
      vba:
        'Dim ws As Worksheet, n As Long, k\nSet ws = Worksheets("Data")\nws.Range("A1").Value = ActiveSheet.Name\n' +
        'ws.Name = "New"\nn = Worksheets.Count\nSet ws = ThisWorkbook.Sheets(2)\nSet ws = Sheets(k)\nSet ws = Worksheets.Add',
      js: [
        'let ws;',
        'let n = 0;',
        'let k;',
        'ws = await workbook.sheet("Data");',
        'await ws.range("A1").setValue(sheet.name);',
        'await ws.rename("New");',
        'n = (await workbook.sheets).length;',
        'ws = await workbook.sheet(1);',
        'ws = await workbook.sheet(vbaKey(k));',
        'ws = await workbook.addSheet();',
      ],
    },
    {
      name: 'range formatting',
      app: 'sheet',
      vba:
        'With Range("A1:C1")\n.Font.Bold = True\n.Font.Size = 14\n.Font.Color = RGB(255, 0, 0)\n.Font.Underline = False\n' +
        '.Interior.Color = vbYellow\n.NumberFormat = "0.00"\n.HorizontalAlignment = xlCenter\n.WrapText = True\nEnd With',
      js: [
        '{',
        'const with1 = sheet.range("A1:C1");',
        'await with1.format({ bold: true });',
        'await with1.format({ fontSize: 14 });',
        'await with1.format({ fontColor: vbaRgb(255, 0, 0) });',
        'await with1.format({ underline: "none" });',
        'await with1.format({ fillColor: "#ffff00" });',
        'await with1.format({ numFmt: "0.00" });',
        'await with1.format({ hAlign: "center" });',
        'await with1.format({ wrap: true });',
        '}',
      ],
    },
    {
      name: 'For Each over the sheets; loop bound read once',
      app: 'sheet',
      vba: 'Dim ws As Worksheet, i As Long\nFor Each ws In Worksheets\nDebug.Print ws.Name\nNext ws\nFor i = 1 To Worksheets.Count\nNext',
      js: [
        'let ws;',
        'let i = 0;',
        'for (ws of await workbook.sheets) {',
        'console.log(ws.name);',
        '}',
        '{',
        'const end1 = (await workbook.sheets).length;',
        'for (i = 1; i <= end1; i++) {',
        '}',
        '}',
      ],
    },
    {
      name: 'Selection typing and formatting in a document',
      app: 'doc',
      vba:
        'Selection.TypeText "Hello"\nSelection.TypeText Text:="x"\nSelection.TypeParagraph\nSelection.Font.Bold = True\n' +
        'Selection.Font.Name = "Arial"\nSelection.WholeStory\nSelection.InsertBreak Type:=wdPageBreak',
      js: [
        'await doc.insertText("Hello");',
        'await doc.insertText("x");',
        'await doc.insertText("\\n");',
        'await doc.format({ bold: true });',
        'await doc.format({ font: "Arial" });',
        'await doc.selectAll();',
        'await doc.insertPageBreak();',
      ],
    },
    {
      name: 'ActiveDocument paragraphs and text',
      app: 'doc',
      vba:
        'Dim s As String, n As Long, p As Paragraph\ns = ActiveDocument.Range.Text\nn = ActiveDocument.Paragraphs.Count\n' +
        's = ActiveDocument.Paragraphs(2).Range.Text\nActiveDocument.Paragraphs(1).Range.Text = "T"\n' +
        'For Each p In ActiveDocument.Paragraphs\np.Range.Font.Italic = True\np.Alignment = wdAlignParagraphCenter\nNext\n' +
        'n = ActiveDocument.Words.Count',
      js: [
        'let s = "";',
        'let n = 0;',
        'let p;',
        's = await doc.text;',
        'n = (await doc.paragraphs).length;',
        's = (await doc.paragraph(1)).text;',
        'await (await doc.paragraph(0)).setText("T");',
        'for (p of await doc.paragraphs) {',
        'await p.format({ italic: true });',
        'await p.setFormat({ align: "center" });',
        '}',
        'n = (await doc.info).wordCount;',
      ],
    },
    {
      name: 'Find.Execute replace all',
      app: 'doc',
      vba: 'With ActiveDocument.Content.Find\n.Execute FindText:="a", ReplaceWith:="b", Replace:=wdReplaceAll, MatchCase:=True\nEnd With',
      js: ['{', 'await doc.replace("a", "b", { matchCase: true });', '}'],
    },
    {
      name: 'slides and shapes',
      app: 'slide',
      vba:
        'Dim sld As Slide, shp As Shape, n As Long\nn = ActivePresentation.Slides.Count\nSet sld = ActivePresentation.Slides(1)\n' +
        'For Each shp In sld.Shapes\nIf shp.HasTextFrame Then shp.TextFrame.TextRange.Text = UCase(shp.TextFrame.TextRange.Text)\nNext\n' +
        'Set shp = sld.Shapes.AddTextbox(msoTextOrientationHorizontal, 10, 20, 300, 40)\nshp.TextFrame.TextRange.Font.Size = 24\n' +
        'shp.Fill.ForeColor.RGB = RGB(0, 0, 255)\nsld.Shapes("Title 1").Delete\nsld.Shapes(2).TextFrame.TextRange.Text = "x"',
      js: [
        'let sld;',
        'let shp;',
        'let n = 0;',
        'n = (await presentation.slides).length;',
        'sld = presentation.slide(0);',
        'for (shp of await sld.shapes) {',
        'if (shp.text !== null) {',
        'await shp.setText((shp.text ?? "").toUpperCase());',
        '}',
        '}',
        'shp = await sld.addTextBox("", { x: 10, y: 20, w: 300, h: 40 });',
        'await shp.format({ size: 24 });',
        'await shp.setFill(vbaRgb(0, 0, 255));',
        'await vbaItem(await sld.shapes, "Title 1").delete();',
        'await (await sld.shapes)[1].setText("x");',
      ],
    },
  ];
  it.each(cases)('$name', ({ vba, js, app }) => {
    const r = body(vba, app);
    expect(r.lines).toEqual(js);
    expect(r.warnings).toEqual([]);
  });
});

describe('convertVba: what cannot be converted', () => {
  const cases: {
    name: string;
    vba: string;
    reason: VbaWarningReason;
    construct?: string;
    app?: ScriptApp;
  }[] = [
    {
      name: 'On Error',
      vba: 'On Error Resume Next',
      reason: 'errorHandling',
      construct: 'On Error',
    },
    { name: 'GoTo', vba: 'GoTo done', reason: 'jump', construct: 'GoTo' },
    { name: 'label', vba: 'done:', reason: 'jump', construct: 'done:' },
    {
      name: 'host switch',
      vba: 'Application.ScreenUpdating = False',
      reason: 'notNeeded',
      construct: 'Application.ScreenUpdating',
    },
    { name: 'DoEvents', vba: 'DoEvents', reason: 'notNeeded' },
    {
      name: 'unknown member',
      vba: 'Range("A1").Select',
      reason: 'unknownMember',
      construct: 'range.Select',
    },
    { name: 'unknown root', vba: 'Selection.Copy', reason: 'unknownName', construct: 'Selection' },
    {
      name: 'unknown function',
      vba: 'x = Environ("TEMP")',
      reason: 'unknownName',
      construct: 'Environ',
    },
    {
      name: 'unknown constant',
      vba: 'Range("A1").HorizontalAlignment = xlDistributed',
      reason: 'unknownName',
    },
    {
      name: 'object of unknown type',
      vba: 'Dim d As Object\nd.Add "k", 1',
      reason: 'unknownName',
      construct: 'd.Add',
    },
    { name: 'New', vba: 'Dim c\nSet c = New Collection', reason: 'unsupportedExpression' },
    {
      name: 'CreateObject',
      vba: 'Set fso = CreateObject("Scripting.FileSystemObject")',
      reason: 'unknownName',
    },
    {
      name: 'InputBox',
      vba: 'x = InputBox("Name?")',
      reason: 'interactive',
      construct: 'InputBox',
    },
    { name: 'MsgBox result', vba: 'x = MsgBox("Sure?", vbYesNo)', reason: 'interactive' },
    {
      name: 'file I/O',
      vba: 'Open "a.txt" For Output As #1',
      reason: 'unsupportedStatement',
      construct: 'Open',
    },
    {
      name: 'Like',
      vba: 'ok = "abc" Like "a*"',
      reason: 'unsupportedExpression',
      construct: 'like',
    },
    {
      name: 'multi-dimensional array',
      vba: 'Dim a(2, 2)\na(1, 1) = 5',
      reason: 'unsupportedExpression',
    },
    { name: 'broken expression', vba: 'x = (1 +', reason: 'syntax' },
    { name: 'stray End If', vba: 'End If', reason: 'syntax' },
    { name: 'conditional compilation', vba: '#If VBA7 Then', reason: 'unsupportedStatement' },
    {
      name: 'document selection in a workbook',
      vba: 'Selection.TypeText "x"',
      reason: 'unknownName',
    },
    { name: 'wrong application', vba: 'Range("A1").Value = 1', reason: 'unknownName', app: 'doc' },
    {
      name: 'Exit For out of a Do loop',
      vba: 'For i = 1 To 2\nDo\nExit For\nLoop\nNext',
      reason: 'unsupportedStatement',
    },
  ];
  it.each(cases)('$name → TODO + warning', ({ vba, reason, construct, app }) => {
    const r = body(vba, app);
    expect(r.warnings.length).toBeGreaterThan(0);
    const w = r.warnings[0]!;
    expect(w.reason).toBe(reason);
    if (construct) expect(w.construct).toBe(construct);
    expect(w.line).toBeGreaterThan(1);
    // The original line is kept, marked.
    expect(r.code).toContain(`// ${TODO_MARK} ${w.source}`);
    expect(vba.replace(/\s+/g, ' ')).toContain(w.source.replace(/\s+/g, ' '));
  });

  it('keeps the structure of a block whose header cannot be converted', () => {
    const r = body(
      [
        'If Application.Version > 12 Then',
        '  x = 1',
        'Else',
        '  x = 2',
        'End If',
        'Do While Not rs.EOF',
        '  n = n + 1',
        '  Exit Do',
        'Loop',
        'For Each c In Selection',
        '  n = n + 1',
        'Next',
        'With CreateObject("x")',
        '  .Run',
        'End With',
        'Select Case Application.Name',
        '  Case "a"',
        '    n = 0',
        'End Select',
      ].join('\n'),
    );
    expect(r.warnings.map((w) => [w.line, w.reason])).toEqual([
      [2, 'unknownMember'],
      [7, 'unknownName'],
      [11, 'unknownName'],
      [14, 'unknownName'],
      [15, 'unknownName'],
      [17, 'unknownMember'],
    ]);
    // Inner statements are still converted.
    expect(r.lines.filter((l) => l === 'n = n + 1;')).toHaveLength(2);
    expect(r.lines).toContain('x = 2;');
    expect(r.lines).toContain('break;');
  });

  it('reports unsupported blocks line by line and ByRef parameters', () => {
    const r = convert(
      [
        'Type Point',
        '  x As Long',
        'End Type',
        'Declare Function GetTickCount Lib "kernel32" () As Long',
        'Property Get Name() As String',
        '  Name = "x"',
        'End Property',
        'Sub Swap(ByRef a, ByRef b)',
        '  t = a: a = b: b = t',
        'End Sub',
      ].join('\n'),
    );
    expect(r.warnings.map((w) => [w.line, w.reason])).toEqual([
      [1, 'unsupportedStatement'],
      [2, 'unsupportedStatement'],
      [3, 'unsupportedStatement'],
      [4, 'unsupportedStatement'],
      [5, 'unsupportedStatement'],
      [6, 'unsupportedStatement'],
      [7, 'unsupportedStatement'],
      [8, 'byRef'],
      [8, 'byRef'],
    ]);
    expect(r.code).toContain('async function Swap(a, b) {');
    // Needs arguments: not called by the draft.
    expect(r.code).not.toContain('await Swap(');
  });

  it('reports blocks that are not closed', () => {
    const r = convert(
      'Sub a()\nx = 1\nIf x Then\nFor i = 1 To 3\ny = i\nEnd Sub\nSub b()\nEnd Sub',
    );
    expect(r.warnings.map((w) => w.construct)).toEqual(['missing Next', 'missing End If']);
    expect(r.procedures).toEqual(['a', 'b']);
  });
});

describe('convertVba: robustness', () => {
  const malformed = [
    '',
    '\n\n',
    'Sub',
    'Sub (',
    'Sub a(\nEnd',
    'End Sub\nEnd Function\nNext\nLoop\nWend\nElse\nCase 1\nEnd With\nEnd Select',
    'If Then Else',
    'If x Then y Else Else z',
    'For Each In\nNext',
    'For i = To\nNext',
    'Select Case\nCase\nCase Is\nCase 1 To\nEnd Select',
    'x = "unterminated',
    'x = #not a date',
    'x = 1 +* 2',
    'x = ((((1',
    'x.y.z(1)(2)(3) = .a',
    'Call',
    'Set',
    'Let = 5',
    'Dim',
    'Dim a(, As',
    'Const',
    'Const x',
    'ReDim',
    'ReDim x',
    'With\n.a = 1\n..b\nEnd With',
    '.a = 1',
    'Exit',
    'Debug.Print',
    'MsgBox',
    'a: b: c: d',
    '10 x = 1\n20 GoTo 10',
    ': : :',
    '_\n_\n_',
    "' only a comment _\ncontinued",
    'Rem',
    'x = &H',
    'x = 1e',
    'x = [A1]',
    'x = a!b',
    'Sub a(a, a)\nEnd Sub\nSub a()\nEnd Sub\nDim a',
    'Const k = 1\nConst k = 2',
    'Function f()\nf = f + 1\nEnd Function\nSub f()\nEnd Sub',
    'Dim await, yield, let, static, arguments, eval\nawait = 1: yield = 2',
    'x = "\u2028" \' \u2028 \u2029 comment\r\ny = 1\u2028z = 2',
    'Sub a()\n' + 'If x Then\n'.repeat(200),
    'x = ' + '('.repeat(300) + '1' + ')'.repeat(300),
    'x = ' + '-'.repeat(50) + '1',
    'x = Not Not Not - - + 1 ^ - 2 ^ 3',
    'x = a ^ b ^ c: y = -a ^ -b: z = (a + b) ^ (c - d) ^ Cells(1, 1)',
    'Range("A1", ).Value = \nCells().Value = 1\nCells(1).x = 2\nWorksheets(1, 2).Name = 3',
    '\u0000\u0001\ufffd @ ` ~ | { } ? $ %',
  ];
  it.each(malformed.map((m, i) => [i, m] as const))(
    'malformed input %i does not throw',
    (_i, vba) => {
      for (const app of ['doc', 'sheet', 'slide'] as const) {
        expect(() => tokenize(vba)).not.toThrow();
        expect(() => parseVba(vba)).not.toThrow();
        convert(vba, app);
      }
    },
  );

  it('random token soup always gives a valid draft', () => {
    const words = (
      'Sub Function End If Then Else ElseIf For Each In To Step Next Do Loop While Until Wend Select Case Is ' +
      'With Dim Const As Long String Set Let Call Exit ReDim Preserve Not And Or Mod Xor New Nothing True ' +
      'Range Cells ActiveSheet Worksheets Selection ActiveDocument Paragraphs ActivePresentation Slides Shapes ' +
      'Value Font Bold Text Count Name MsgBox Debug Print x y i n ws ( ) , . = <> < > + - * / \\ ^ & : _ ' +
      '"s" 1 2.5 #1/2/2024# \' \n \n \n On Error GoTo Application Optional ByRef ByVal'
    ).split(' ');
    let seed = 12345;
    const rnd = () => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed / 0x7fffffff;
    };
    for (let round = 0; round < 400; round++) {
      const n = 5 + Math.floor(rnd() * 60);
      const vba = Array.from({ length: n }, () => words[Math.floor(rnd() * words.length)]!).join(
        ' ',
      );
      const app = (['doc', 'sheet', 'slide'] as const)[round % 3]!;
      const r = convertVba(vba, { app });
      expect(() => parses(r.code), `${vba}\n----\n${r.code}`).not.toThrow();
    }
  });

  it('nothing is dropped: every source statement is code, a comment or a TODO', () => {
    const vba = [
      'Sub Report()',
      "  ' Fill the header",
      '  Application.ScreenUpdating = False',
      '  Range("A1").Value = "Total"',
      '  Range("A1").Select',
      '  On Error GoTo failed',
      '  Cells(2, 1).Value = WorksheetFunction.Sum(Range("B1:B9"))',
      'failed:',
      '  MsgBox "Done"',
      'End Sub',
    ].join('\n');
    const r = convert(vba);
    expect(lines(r.code)).toEqual([
      'async function Report() {',
      '// Fill the header',
      `// ${TODO_MARK} Application.ScreenUpdating = False`,
      'await sheet.range("A1").setValue("Total");',
      `// ${TODO_MARK} Range("A1").Select`,
      `// ${TODO_MARK} On Error GoTo failed`,
      `// ${TODO_MARK} Cells(2, 1).Value = WorksheetFunction.Sum(Range("B1:B9"))`,
      `// ${TODO_MARK} failed:`,
      'console.log("Done");',
      '}',
      'await Report();',
    ]);
    expect(r.warnings.map((w) => w.line)).toEqual([3, 5, 6, 7, 8]);
  });

  it('truncates an oversized source with a warning', () => {
    const r = convertVba(`x = 1\n${"' pad\n".repeat(80_000)}`, { app: 'sheet' });
    expect(r.warnings.at(-1)).toMatchObject({ reason: 'syntax', construct: 'source' });
    expect(() => parses(r.code)).not.toThrow();
  });
});

describe('convertVba: the draft runs in the sandbox', () => {
  it('a converted workbook macro makes the expected API calls', async () => {
    const vba = [
      'Sub Fill()',
      '  Dim i As Long, total As Long',
      '  For i = 1 To 3',
      '    Cells(i, 1).Value = i * 10',
      '    total = total + Cells(i, 1).Value',
      '  Next i',
      '  If total > 50 Then',
      '    Range("B1").Value = "big " & total',
      '    Range("B1").Font.Bold = True',
      '  End If',
      '  Range("C1").Value = Right("abcdef", 2) & Mid("abcdef", 2, 2) & InStr("abc", "c")',
      '  MsgBox "Total: " & total',
      'End Sub',
    ].join('\n');
    const r = convert(vba);
    expect(r.warnings).toEqual([]);
    const cells = new Map<string, unknown>();
    const calls: string[] = [];
    const host: ScriptHost = {
      app: 'sheet',
      call: async (method, args) => {
        const [, address, value] = args as [number, string, unknown];
        if (method === 'sheet.fill') cells.set(address, value);
        if (method === 'sheet.getValues') return [[cells.get(address) ?? null]];
        calls.push(`${method} ${address}`);
        return null;
      },
    };
    const logs: string[] = [];
    const res = await inlineRunner().run(r.code, host, { log: (_l, text) => logs.push(text) }).done;
    expect(res.error).toBeUndefined();
    expect(res.ok).toBe(true);
    expect(Object.fromEntries(cells)).toEqual({
      A1: 10,
      A2: 20,
      A3: 30,
      B1: 'big 60',
      C1: 'efbc3',
    });
    expect(calls).toContain('sheet.format B1');
    expect(logs).toEqual(['Total: 60']);
  });
});
