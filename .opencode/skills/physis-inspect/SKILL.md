---
name: physis-inspect
description: Use existing Physis MCP for bounded repository or working-record evidence when cross-module changes, unclear dependencies, disagreement, repeated failures or plan drift justify inspection. Not for trivial local edits. Lexical evidence is not semantic certification.
---

Reuse existing physis_system MCP configured in .opencode/opencode.json. Existing generic physis skill checks capability claims; this workflow narrows optional orchestration inspection, not another implementation.

1. Discover connected catalog and call physis_system_physis_capabilities. Verify workspace root matches current checkout. Wrong root/unavailable: NOT MEASURED; do not search a different checkout silently.
2. physis_system_physis_history {query,limit:3} before substantial work; physis_system_physis_pack {query,budget:1200} retrieves lexical source windows; follow refs with physis_system_physis_read {target:"path:start-end",max_bytes:6000}. physis_system_physis_find {query,limit:5} locates evidence. physis_system_physis_inspect {} inventories only when needed.
3. Existing physis_system_physis_tasks {} and physis_system_physis_task {id} inspect Physis delegation records, NOT native OpenCode tasks. Reservations do not launch workers.
4. Working record: accept compact objective/task/findings/test/diff references from coordinator; native OpenCode session export contains actual history. Checkpoint JSON under .physis/opencode-orchestration is ignored by workspace inventory: coordinator must supply bounded summary, not ask pack to magically find it. Do not ingest full conversations or secret-bearing command logs.
5. Compare before/planned/actual paths and supplied diff against retrieved evidence. Report surprising dependencies only with source refs. This MCP is lexical; it DOES NOT expose semantic_delta, neighbors, graph centrality, or dependency scores. Those metrics remain null. Existing Rust modules are not automatically callable MCP tools. Never invent physis_delta or claim semantic validation from BM25.
6. Return {useful, representation:"lexical", findings:[{task_id,evidence,observation}], evidence_refs, unavailable_metrics}. Contradictory/empty evidence is a limitation, not a pass. Tests remain required.

Invoke once for unresolved structural questions; again only after meaningful new delta/failure evidence. Skip formatting/rename/isolated obvious fixes. Before Astra, prefer compact findings when information value warrants it, never mandatory dependency. Record final concise checked outcome with existing physis_remember only from ordinary human-supervised workflow, actor opencode; inspector itself is read-only.
