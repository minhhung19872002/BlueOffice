"""Resolve merge conflicts in the i18n locale JSON files by keeping both sides.

Tries, per hunk, the ways two sides that both added entries can be joined, and
keeps the first combination that parses as JSON without duplicate keys.
"""
import collections
import itertools
import json
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
NL = "\n"
BS = chr(92)


def net(t):
    d = 0
    ins = False
    esc = False
    for c in t:
        if ins:
            if esc:
                esc = False
            elif c == BS:
                esc = True
            elif c == '"':
                ins = False
        elif c == '"':
            ins = True
        elif c == "{":
            d += 1
        elif c == "}":
            d -= 1
    return d


def dup(pairs):
    k = [a for a, _ in pairs]
    d = [x for x, c in collections.Counter(k).items() if c > 1]
    assert not d, d
    return dict(pairs)


PAT = re.compile(r"<<<<<<< [^\n]*\n(.*?)=======\r?\n(.*?)>>>>>>> [^\n]*\n", re.S)


def variants(m):
    o, t = m.group(1), m.group(2)
    os_ = o.rstrip(NL)
    ts = t.rstrip(NL)
    yield os_.rstrip(",") + "," + NL + ts.rstrip(",") + NL
    yield os_.rstrip(",") + "," + NL + ts.rstrip(",") + "," + NL
    yield o + t
    n = net(o)
    if n > 0:
        close = NL.join("  " * (n - i) + "}" for i in range(n)) + "," + NL
        yield o + close + t


FILES = sys.argv[1:] or [
    "packages/i18n/src/locales/en-US.json",
    "packages/i18n/src/locales/vi-VN.json",
]
for f in FILES:
    s = open(f, encoding="utf-8", newline="").read()
    ms = list(PAT.finditer(s))
    print(f, len(ms), "hunks")
    if not ms:
        json.loads(s, object_pairs_hook=dup)
        continue
    ok = False
    err = None
    for combo in itertools.product(*[list(variants(m)) for m in ms]):
        out = s
        for m, c in reversed(list(zip(ms, combo))):
            out = out[: m.start()] + c + out[m.end() :]
        try:
            json.loads(out, object_pairs_hook=dup)
            ok = True
            break
        except Exception as e:  # noqa: BLE001
            err = e
    assert ok, err
    open(f, "w", encoding="utf-8", newline="").write(out)
    print("ok")
