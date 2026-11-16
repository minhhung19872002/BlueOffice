# Task (round 7): BlueSheet — remaining gaps, behaviour parity

Build dir: E:/bo-targets/sheet (reuse; E: is tight — clean your incremental/stale deps under 10 GB free). Start from
`work/desktop-performance` (`git reset --hard work/desktop-performance`; must contain "Merge BlueSheet round 6").
Read `docs/ui/REPORT-sheet.md` rounds 20–26.

1. Wheel latency to ~1 frame (it has stayed ~2 frames for 4 rounds): try, measure, keep what works —
   (a) build the cell display list only for the newly exposed band and reuse the rest (ring already exists),
   (b) DirectComposition / flip-model with `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT`, MaxFrameLatency 1, wait
   before input processing, (c) present immediately after input instead of on the next timer/paint, (d) check that
   WM_MOUSEWHEEL is handled without a deferred InvalidateRect→WM_PAINT round trip. Report each try's numbers.
2. Insert tab fits at 1500 px without scroll arrows (Tables/Sparklines groups ~50 px too wide — measure and fix).
3. Chart engine commands: Switch Row/Column, Select Data dialog (series list, edit ranges, category range), Move Chart
   (object in sheet / new chart sheet) — engine + native UI + TS `SheetCommand` types, with tests.
4. Format as Table: all 60 reference table styles (Light 1–21, Medium 1–28, Dark 1–11) with the reference's colours,
   77×56 px tiles, and the Table Design contextual tab (options check boxes, style gallery, resize, convert to range,
   total row functions).
5. Conditional Formatting submenus with previews (Highlight Cells Rules, Top/Bottom, Data Bars, Color Scales, Icon
   Sets) and Manage Rules dialog (list, edit, delete, order, stop if true) — engine CF exists.
6. Page Layout Themes / Colors / Fonts (built-in theme set + xlsx theme round-trip); Page Break Preview and Page
   Layout views.
7. Behaviour checks with posted input vs the reference: fill handle drag (series, copy, Ctrl), double-click fill
   handle, drag-move cells, Ctrl+Z/Y granularity, paste special dialog, Enter direction, autocomplete in a column.
8. Full side-by-side pass; record next rounds in `docs/ui/REPORT-sheet.md`.

Same rules as the common brief (images scratchpad\show\sheet-NN-*.png continuing after 12, SendMessage "main" after
each step; own icon shapes). Never send real input. Final exe: out/desktop/bluesheet-native.next.exe.
