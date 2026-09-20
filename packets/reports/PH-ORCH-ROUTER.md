# PH-ORCH-ROUTER — orchestrator could not delegate (provider quota)

**Status:** implementation done; tests green. Independent review not performed.

## Symptom

Every `orchestrate_delegate` call returned
`{"status":"human","reason":"provider_or_execution_error"}` at `phase:
prompting`. Reproduced for `scout` and `critic`.

## Root cause

`.opencode/orchestration.json` pointed every role at
`commandcode/*`. Direct probe:

```
$ opencode run --model commandcode/deepseek/deepseek-v4-flash --agent plan "Reply OK"
Error: Command Code API error 429: You've reached your weekly usage limit for your plan.
       Your limit resets at 2026-09-23T13:39:29.984Z. ... RATE_LIMITED: rate_limit_error
```

The router's `unavailable()` (`clients/opencode-orchestration/router.mjs:78`)
only matched unknown-model wording, so a 429 was classified
`provider_or_execution_error` and the run stayed pending for a human instead of
falling back to the next configured model. With no non-`commandcode` family
configured, there was nothing to fall back to.

## Fix

1. `clients/opencode-orchestration/router.mjs` — `unavailable()` now also treats
   rate limit / quota / overload / payment-required as a reason to try the next
   candidate. The retry still requires no tool use before the failure, so a
   provider that failed mid-task is never replayed.
2. `.opencode/orchestration.json` — repointed every role at providers verified
   live this session, with distinct families for independence:
   `deepseek/deepseek-v4-flash` + `deepseek/deepseek-v4-pro` (deepseek),
   `mistral/codestral-latest` (mistral), `kilo/cohere/north-mini-code:free`
   (kilo). `architect` is `deepseek/deepseek-v4-pro`.
3. `worker_bash_allow` populated with the gate commands (`cargo *`, `rg *`,
   `git status/diff/log/add/commit`, `node --test*`, …) so a worker can run its
   own acceptance tests. It was `[]` → every worker command asked.
4. `clients/opencode-orchestration/router.test.mjs` — the fake provider catalog
   assumed a `commandcode/` prefix; generalized to any provider. Added
   `rate-limited candidate falls back before any tool use`.

## Verification (native)

```
node --test clients/opencode-orchestration/router.test.mjs
  # tests 23   # pass 23   # fail 0
```

Model liveness probed directly with `opencode run`:
`deepseek/deepseek-v4-flash` OK, `deepseek/deepseek-v4-pro` OK,
`mistral/codestral-latest` OK, `kilo/cohere/north-mini-code:free` OK.
Rejected: `groq/*` (no access), `google/*` (invalid key), `cerebras/*` (payment
required), `scaleway/*` (forbidden), `nvidia/deepseek-ai/deepseek-v4-flash`
(end of life).

## What I could not verify

- **The running OpenCode session still holds the old policy.** The plugin reads
  `.opencode/orchestration.json` once at startup, so live delegation in this
  session would still use `commandcode` until OpenCode is restarted. The fix is
  proven at the unit level, not by a real delegation.
- **`commandcode` is only rate-limited, not dead.** Its weekly window resets
  2026-09-23; it can be restored to the role lists then.
- **No independent review.**
