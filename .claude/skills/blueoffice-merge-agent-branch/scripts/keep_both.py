"""Resolve conflict hunks in the given files by keeping both sides (ours then theirs).

Usage: bo_both.py [--show] file...
"""
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
PAT = re.compile(r"<<<<<<< [^\n]*\n(.*?)=======\r?\n(.*?)>>>>>>> [^\n]*\n", re.S)
show = "--show" in sys.argv
for p in [a for a in sys.argv[1:] if a != "--show"]:
    s = open(p, encoding="utf-8", newline="").read()
    if show:
        for m in PAT.finditer(s):
            print(p, "| OURS:", repr(m.group(1)[:240]), "| THEIRS:", repr(m.group(2)[:240]))
        continue
    s, n = PAT.subn(lambda m: m.group(1) + m.group(2), s)
    open(p, "w", encoding="utf-8", newline="").write(s)
    print(p, n, "hunks kept both")
