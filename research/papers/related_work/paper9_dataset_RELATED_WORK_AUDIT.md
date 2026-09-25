# P9 related-work audit

## Searches run
- "SWE-chat dataset coding agent trajectories fine-grained tool calls" — useful.
- "SWE-agent trajectories dataset tool call level action outcomes" — useful.
- "coding agent telemetry dataset per-action failure labels multiple harnesses" — useful.

## Comparators (as reported by their authors)

| Corpus | Sessions / scale | Per-action records | Outcome per action | Normalised taxonomy | Multi-harness |
|---|---|---|---|---|---|
| SWE-chat (arXiv:2604.20779) | 6,000 sessions; 355k tool calls; 2.7M events; 200+ repos | yes | **no** (turn/session: pushback 44%, code survival 44%) | no | several agents, real users |
| SWE-agent `.traj` | per benchmark instance | yes (thought/action/observation) | **no** (task-level resolution) | no | single harness |
| Thought–action–result study (arXiv:2506.18824) | 120 trajectories; 2,822 interactions; 3 agents | yes | **no** (task outcome) | manual action categories | 3 agents, benchmark |
| MAST taxonomy applied to production telemetry (arXiv:2503.13657) | 639,381 steps; 23,624 runs | aggregates only | **run-level** LLM-judge mode | 14 modes | single platform |
| AFTraj-2K (AgentForesight) | 2,276 trajectories | yes | **decisive error step only** | no | multi-domain |
| coding-agent failure atlas | 120 synthetic traces | yes | synthetic | 12 modes | synthetic |
| **This corpus** | 402 sessions; 75,933 actions | yes | **yes, every action** | **871-way** | **4 harnesses, 3 machines** |

Also noted from the SWE-chat comparison table: SWE-smith-trajectories,
CoderForge-Preview, SERA, nex-agi-agent-sft, SWE-rebench-openhands-trajectories,
Bouzenia & Pradel agent trajectories, Multi-SWE-bench_trajs, Agent Data Protocol,
AIDev — trajectory releases whose released outcome signal is task-level.

## TraceLab
A corpus published under the name **TraceLab could not be located** in this
pass. If one exists, P9 has not positioned against it and the omission is ours.
Recorded rather than guessed at.

## Novelty claim this supports (narrow)
Not "we collected coding-agent traces" — SWE-chat is far larger. The defensible
claim is: **per-action outcome labels**, one **normalised action-kind taxonomy
across four harnesses**, **retained session structure**, and the demonstrated
**value of action-kind granularity** over tool-level granularity (AUC 0.627–0.751
vs 0.446–0.525). P9's abstract and related-work section now state this and
explicitly concede scale.
