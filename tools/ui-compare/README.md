# UI comparison kit (native apps vs the reference desktop apps)

How the native BlueDoc / BlueSheet / BlueSlide / BlueReader / BlueEditor windows are matched to the installed
reference apps by black-box measurement (owner decisions: `CLAUDE.md` naming section, ADR-0006 addendum,
`docs/ui/REPORT*.md`). Used by the main session and by parallel agents; `AGENT-BRIEF.md` is the common brief given
to every agent.

Scripts (PowerShell, Windows at 125 % DPI with Accessibility text size 110 %):

| Script | Use |
|---|---|
| `cap2.ps1 <title-substring> <out.png> [W H]` | PrintWindow capture; moves/resizes the window to 1500×1102 at 40,40 without activating it |
| `drive.ps1 <dir> <name> "x,y;x,y"` | launch BlueDoc on a copy of the sample, post clicks (client px), capture |
| `menu.ps1`, `type2.ps1`, `lat.ps1` | open a menu by posted click / typing latency via `BLUEOFFICE_NATIVE_PERF` / on-screen latency |
| `wopen.ps1 <file> <zoom>`, `wclose.ps1` | open / close a COPY of a document in the reference word processor via COM |

Rules that never change:
- Never send real keyboard/mouse input (SendInput, keybd_event, SetCursorPos…): the owner uses the PC. Post
  messages to our own windows; drive reference apps via COM / UI Automation on COPIES of files; capture with
  PrintWindow (`BLUEOFFICE_SHOW_NOACTIVATE=1` launches ours without stealing focus). Close reference windows you
  opened. Never capture the reference's recent-file lists or the owner's documents.
- Match layout, sizes, greys, palette and the meaning of accent colours; icon SHAPES are our own drawings — never
  traced or copied (Microsoft or Foxit). Never copy names, fonts, brand colours of the title bar.
- Every comparison produces a side-by-side image (reference top/left, ours bottom/right, ≤ 1600 px) that is shown
  to the owner.

Scripts that post mouse messages must be per-monitor DPI aware (`SetProcessDpiAwarenessContext(-4)`,
as `cap2.ps1` / `menu.ps1`): Windows scales the coordinates of mouse messages posted by a DPI-unaware
process (Windows PowerShell) to a per-monitor-aware window — at 125 % a click posted at x 209 arrives at
261 and lands on the next ribbon tab (BlueDoc round 4; `BLUEOFFICE_INPUT_LOG=1` prints each pointer
input of BlueDoc on stderr).
