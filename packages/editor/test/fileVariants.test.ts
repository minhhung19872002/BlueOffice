import { describe, expect, it } from 'vitest';
import { planOpen, saveName } from '../src/FileVariants';

describe('opening templates and slide shows (CORE-FMT-001)', () => {
  it('a template becomes an untitled document of the matching type', () => {
    expect(planOpen('dotx', 'Công văn.dotx')).toMatchObject({
      fromTemplate: true,
      saveAs: 'docx',
      name: 'Công văn.docx',
      show: false,
    });
    expect(planOpen('xltm', 'Bảng lương.xltm')).toMatchObject({
      fromTemplate: true,
      saveAs: 'xlsm',
    });
    // The content type decides, not the name.
    expect(planOpen('potx', 'đặt tên sai.pptx').saveAs).toBe('pptx');
  });

  it('"Edit template" opens the template itself', () => {
    const plan = planOpen('dotm', 'Mẫu.dotm', true);
    expect(plan).toMatchObject({ fromTemplate: false, name: 'Mẫu.dotm' });
    expect(plan.saveAs).toBeUndefined();
  });

  it('slide shows start the show; documents open as they are', () => {
    expect(planOpen('ppsm', 'Tự chạy.ppsm')).toMatchObject({ fromTemplate: false, show: true });
    expect(planOpen('docx', 'a.docx')).toMatchObject({ fromTemplate: false, show: false });
    expect(planOpen(undefined, 'a.odt')).toMatchObject({ fromTemplate: false, show: false });
  });

  it('saves under the extension of the variant', () => {
    expect(saveName('Báo cáo.doc', 'docx', 'docx')).toBe('Báo cáo.docx');
    expect(saveName('Báo cáo.docx', 'dotm', 'docx')).toBe('Báo cáo.dotm');
    expect(saveName('Ghi chú.txt', undefined, 'docx')).toBe('Ghi chú.docx');
  });
});
