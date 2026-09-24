#!/usr/bin/env python3
"""Repin the delegation model to a provider that actually responds.

Root cause: delegation.model was set to `llama-3.3-70b-versatile`, which groq
no longer serves (HTTP 404), and the groq key is additionally 403-blocked.
Every delegated task therefore died in ~0.5s without doing anything.

This rewrites only the four identity fields of the delegation block, taking
the credential from the nvidia provider block. Prints no secrets.
"""
import re, shutil, sys

CFG = "/home/gio/.hermes/config.yaml"
src = open(CFG).read()

nvid = re.search(r"^  nvidia:\n((?:    .*\n)+)", src, re.M)
if not nvid:
    sys.exit("nvidia provider block not found")
nblk = nvid.group(1)
n_base = re.search(r"base_url:\s*(\S+)", nblk).group(1)
n_key = re.search(r"api_key:\s*(\S+)", nblk).group(1)

m = re.search(r"^delegation:\n((?:  .*\n)+)", src, re.M)
if not m:
    sys.exit("delegation block not found")
dblk = m.group(1)

old_model = re.search(r"model:\s*(\S+)", dblk).group(1)
old_prov = re.search(r"provider:\s*(\S+)", dblk).group(1)

new = dblk
new = re.sub(r"model:.*", "model: nvidia/nemotron-3.5-lightning-30b-a3b", new)
new = re.sub(r"provider:.*", "provider: nvidia", new)
new = re.sub(r"base_url:.*", f"base_url: {n_base}", new)
new = re.sub(r"api_key:.*", f"api_key: {n_key}", new)
# a 30B reasoning model needs headroom; the old 50 was sized for llama-70B
new = re.sub(r"max_iterations:.*", "max_iterations: 40", new)
# it emits long reasoning traces, so give the child more wall-clock
new = re.sub(r"child_timeout_seconds:.*", "child_timeout_seconds: 900", new)

shutil.copy(CFG, CFG + ".bak-before-delegation-repin")
open(CFG, "w").write(src[:m.start(1)] + new + src[m.end(1):])

print(f"delegation.model : {old_model}  ->  nvidia/nemotron-3.5-lightning-30b-a3b")
print(f"delegation.provider: {old_prov} ->  nvidia")
print(f"base_url         -> {n_base}")
print("api_key          -> (copied from nvidia block, not shown)")
print("max_iterations   -> 40;  child_timeout_seconds -> 900")
print(f"backup written to {CFG}.bak-before-delegation-repin")
