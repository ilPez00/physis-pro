#!/usr/bin/env python3
"""Release security/privacy audit for the coding-agent corpus.

Scans every string field of the released corpus for material that must not
ship: credentials, tokens, SSH material, hostnames, usernames, absolute
paths, project names, emails, URLs, IPs. Also verifies that fields the papers
describe as redacted actually are.

Redaction claims in the papers: `benchmarks/behaviour/corpus-redacted.json.gz`
is described as carrying hashed targets and no raw command text or tool
output. This audits that claim rather than trusting it.

  python3 research/release_audit.py
"""
import gzip, json, re, sys
from collections import Counter, defaultdict

REPO = "/home/gio/dev/physis-pro"
CORPUS = f"{REPO}/benchmarks/behaviour/corpus-redacted.json.gz"

PATTERNS = {
    "openai/llm key":   re.compile(r"\b(sk-[A-Za-z0-9_\-]{16,}|sk-proj-[A-Za-z0-9_\-]{16,})"),
    "anthropic key":    re.compile(r"\bsk-ant-[A-Za-z0-9_\-]{16,}"),
    "groq key":         re.compile(r"\bgsk_[A-Za-z0-9]{20,}"),
    "github token":     re.compile(r"\b(gh[pousr]_[A-Za-z0-9]{20,})"),
    "aws key":          re.compile(r"\b(AKIA|ASIA)[A-Z0-9]{16}\b"),
    "slack token":      re.compile(r"\bxox[baprs]-[A-Za-z0-9\-]{10,}"),
    "google api key":   re.compile(r"\bAIza[A-Za-z0-9_\-]{35}\b"),
    "huggingface":      re.compile(r"\bhf_[A-Za-z0-9]{20,}"),
    "nvidia key":       re.compile(r"\bnvapi-[A-Za-z0-9_\-]{20,}"),
    "bearer header":    re.compile(r"[Bb]earer\s+[A-Za-z0-9._\-]{20,}"),
    "private key block": re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    "email":            re.compile(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b"),
    "url":              re.compile(r"\bhttps?://[^\s\"'<>]+"),
    "ipv4":             re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b"),
    "abs unix path":    re.compile(r"(?<![\w\-])/(?:home|Users|root|mnt|var|etc|opt)/[\w.\-/]*"),
    "win path":         re.compile(r"\b[A-Za-z]:\\\\?[\w.\\\-]+"),
    "ssh target":       re.compile(r"\b[\w.\-]+@[\w.\-]+:[\w./~\-]+"),
}

findings = defaultdict(list)
field_values = defaultdict(Counter)


def scan_string(s, where):
    for name, rx in PATTERNS.items():
        for m in rx.finditer(s):
            findings[name].append((where, m.group()[:80]))


with gzip.open(CORPUS, "rb") as fh:
    raw = fh.read()
doc = json.loads(raw)

# ---------- 1. raw text sweep (catches anything the structure misses) ----------
scan_string(raw.decode("utf-8", "replace"), "<raw corpus text>")

# ---------- 2. structured field inventory ----------
for si, s in enumerate(doc.get("sessions", [])):
    for k, v in s.items():
        if k == "actions":
            continue
        if isinstance(v, str):
            field_values[k][v] += 1
            scan_string(v, f"session[{si}].{k}")
        elif isinstance(v, list):
            for item in v:
                if isinstance(item, str):
                    field_values[k][item] += 1
                    scan_string(item, f"session[{si}].{k}[]")
    for ai, a in enumerate(s.get("actions", [])):
        for k, v in a.items():
            if isinstance(v, str):
                field_values[f"action.{k}"][v] += 1
                scan_string(v, f"session[{si}].action[{ai}].{k}")

# ---------- 3. does a field described as redacted actually hash? ----------
def looks_hashed(v):
    return bool(re.fullmatch(r"[0-9a-f]{8,64}", v))

print("=" * 72)
print("RELEASE AUDIT — benchmarks/behaviour/corpus-redacted.json.gz")
print("=" * 72)
print(f"sessions: {len(doc.get('sessions', []))}  actions: {doc.get('actions')}")

print("\n--- credential / secret patterns ---")
hard = [k for k in PATTERNS if k not in ("url", "ipv4", "abs unix path",
                                          "win path", "email", "ssh target")]
for name, hits in sorted(findings.items()):
    tag = "CRITICAL" if name in hard else "note"
    uniq = sorted({h[1] for h in hits})
    print(f"  [{tag:8s}] {name:20s} {len(hits):6d} hits, {len(uniq)} distinct")
    if name in hard and hits:
        for w, m in hits[:5]:
            print(f"             e.g. {w}: {m}")
    elif uniq and len(uniq) <= 8:
        for u in uniq:
            print(f"             e.g. {u}")

print("\n--- redaction claims ---")
tgt = field_values.get("action.target", Counter())
hashed = sum(c for v, c in tgt.items() if looks_hashed(v))
empty = sum(c for v, c in tgt.items() if v == "")
total = sum(tgt.values())
verdict = "OK" if hashed + empty == total else "LEAK"
print(f"  action.target      : {total} values, {hashed} hashed "
      f"({100.0 * hashed / max(total,1):.1f}%), {empty} empty (absent), "
      f"{total - hashed - empty} neither -> {verdict}")
if hashed + empty != total:
    for v, c in tgt.most_common():
        if not looks_hashed(v) and v != "":
            print(f"      NOT hashed and not empty: {v!r} x{c}")

print("\n--- free-text-ish fields (highest leak risk) ---")
for fld in ("project", "machine", "corpus", "harness", "session", "seen_on"):
    vals = field_values.get(fld)
    if not vals:
        continue
    print(f"\n  {fld}: {len(vals)} distinct")
    for v, c in vals.most_common(14):
        print(f"      {v!r} x{c}")

print("\n--- action vocabulary (should be schema, not content) ---")
for fld in ("action.kind", "action.tool"):
    vals = field_values.get(fld)
    if vals:
        print(f"  {fld}: {len(vals)} distinct; top: "
              f"{[v for v, _ in vals.most_common(6)]}")

print("\n" + "=" * 72)
crit = sum(len(findings[k]) for k in hard)
print(f"VERDICT: {'FAIL — credential-class material present' if crit else 'no credential-class material found'}")
print(f"  credential-class hits: {crit}")
print("  NOTE: URL/IP/path/email counts above are informational; project and")
print("  machine fields must be judged by hand — the scanner cannot tell a")
print("  harmless token from a real project name.")
sys.exit(1 if crit else 0)
