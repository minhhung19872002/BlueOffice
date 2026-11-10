/* global BlueOfficeAddin, document */
// Sample add-in page of addins.spec.ts (CORE-ADDIN-001): reads the selection, inserts
// text in BlueDoc / writes a cell in BlueSheet, reports refused calls.
const $ = (id) => document.getElementById(id);
const show = (v) => {
  $('out').textContent = typeof v === 'string' ? v : JSON.stringify(v);
};
const failed = (e) => show(`error ${e.code}: ${e.message} [${(e.missing || []).join(',')}]`);

async function insert(bo, text) {
  if (bo.app === 'doc') await bo.doc.insertText(text);
  else if (bo.app === 'sheet') await bo.workbook.setValues(-1, 'B2', [[text]]);
  show('inserted');
}

BlueOfficeAddin.connect().then(
  (bo) => {
    $('status').textContent = `ready ${bo.app} ${bo.info.permissions.join(',')}`;
    document.body.dataset.ready = 'true';
    $('read-selection').onclick = () =>
      bo.selection.get().then((s) => show(s.app === 'doc' ? `selected: ${s.text}` : s), failed);
    $('insert').onclick = () => insert(bo, 'Từ add-in').catch(failed);
    $('read-document').onclick = () =>
      (bo.app === 'doc' ? bo.doc.text() : bo.workbook.getValues(-1, 'A1:C3')).then(
        (t) => show(`document: ${JSON.stringify(t)}`),
        failed,
      );
    $('store').onclick = () =>
      bo.storage
        .set('n', 1)
        .then(() => bo.storage.get('n'))
        .then((n) => show(`stored ${n}`), failed);
    bo.on('selectionChanged', () => {
      $('events').textContent += 'selection;';
    });
    bo.on('command', (e) => {
      $('events').textContent += `command:${e.command};`;
      if (e.command === 'greet') insert(bo, 'Xin chào!').catch(failed);
    });
  },
  (e) => {
    $('status').textContent = `connect failed: ${e.message}`;
  },
);
