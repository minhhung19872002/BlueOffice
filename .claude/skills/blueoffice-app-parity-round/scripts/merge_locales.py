"""Resolve a conflicted merge of packages/i18n/src/locales/*.json by deep-merging both sides.

Run from the repo root during a conflicted `git merge`: keeps every key of HEAD, adds keys only
MERGE_HEAD has (recursively), writes with the repo's 2-space JSON layout and checks that vi-VN and
en-US end up with the same key set.
"""
import json
import subprocess
import sys

FILES = ["packages/i18n/src/locales/en-US.json", "packages/i18n/src/locales/vi-VN.json"]


def show(rev, path):
    out = subprocess.run(["git", "show", f"{rev}:{path}"], capture_output=True, check=True).stdout
    return out.decode("utf-8")


def merge(a, b):
    out = dict(a)
    for k, v in b.items():
        if k in out and isinstance(out[k], dict) and isinstance(v, dict):
            out[k] = merge(out[k], v)
        elif k not in out:
            out[k] = v
    return out


def keys(o, p=""):
    for k, v in o.items():
        if isinstance(v, dict):
            yield from keys(v, p + k + ".")
        else:
            yield p + k


merged = {}
for path in FILES:
    raw = show("HEAD", path)
    m = merge(json.loads(raw), json.loads(show("MERGE_HEAD", path)))
    text = json.dumps(m, ensure_ascii=False, indent=2) + ("\n" if raw.endswith("\n") else "")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    merged[path] = set(keys(m))
a, b = (merged[p] for p in FILES)
if a != b:
    print("key sets differ:", sorted(a ^ b)[:20])
    sys.exit(1)
print("locales merged,", len(a), "keys each")
