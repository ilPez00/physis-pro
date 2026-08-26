# Zero-Budget Adoption Plan — riding the agent wave

> 2026-08-26. Researched against the live landscape (see sources at bottom).
> Constraint: no budget, one maintainer, proprietary EULA (source-visible, not
> OSI-licensed). Everything below costs time only.

## The one insight

**Distribution has moved from search engines to agent directories and
agent-to-agent networks.** MCP usage grew ~340% YoY; the official registry's
metadata feeds 78% of all installs; Moltbook signed up 1.5M agents in days.
Physis already speaks both languages — it ships an MCP server (`mcp` feature,
rmcp stdio transport) and a plain HTTP API any agent framework can call. The
product was accidentally built for exactly this channel. The plan is to stop
accidental and make it deliberate.

## Track 1 — Be findable where agents shop (highest leverage)

The unit of adoption in 2026 is not "a developer reads the README" but "an
agent's owner mounts a tool". Physis gives an LLM persistent meaning:
classify anything against a 70-cell grid, ask the graph questions, track
coherence. Pitch line: *"Give your agent a semiotic memory."*

| Step | Channel | Action | Cost |
|------|---------|--------|------|
| 1 | Official registry (registry.modelcontextprotocol.io) | Submit server manifest; most directories downstream index from here | hours |
| 2 | GitHub topics | Add `mcp-server`, `mcp`, `ai-agents`, `semantic-search`, `knowledge-graph`, `rust` — aggregators sync repos by topic | minutes |
| 3 | Smithery | Submit for one-line install (`npx @smithery/cli install …`) — 92% first-install success is their metric | hours |
| 4 | LobeHub + PulseMCP + Glama + mcp.directory | Cross-list; each pulls from registry but adds UI surface | hours |
| 5 | crates.io + docs.rs | Publish the crate (done today); lib.rs indexes by keyword; docs.rs renders rustdoc — AI crawlers read both heavily | done |
| 6 | llms.txt | Add `/llms.txt` describing endpoints so AI crawlers summarize capabilities correctly | hour |
| 7 | Awesome-MCP lists | PR to awesome-mcp-servers style lists (curated, high signal) | hours |

Caveat to handle honestly: some directories require an OSI licence. If a given
directory rejects the EULA, skip it rather than relicense — the paid pilot
hypothesis (PLAN 5.4) depends on the current licence.

## Track 2 — AI-driven social networking (the new frontier)

Three real options, ordered by effort:

1. **Feed ingestion demo (days, pure product).** The history importer already
   eats OPML/bookmarks/chat JSONL. A public workspace that ingests an RSS feed
   of an agent network (Moltbook mirrors exist; Bluesky firehose exports; HN)
   and publishes its coherence/dream analysis is a live, self-demonstrating
   artifact — the engine analyzing the very networks talking about it.
2. **An agent that posts from physis (weekend).** A small bot (runs on the
   existing fleet) that registers on Moltbook/AgentGram as a physis instance
   and posts one daily artifact: yesterday's top dream proposal, the biggest
   quality penalty learned, a `where` query answered. Every post is a product
   screenshot that links back. Chen's law applies: visual products spread
   through people (and agents) posting outputs.
3. **Agent-to-agent API peering (later).** Physis instances already talk HTTP;
   the remote-callback pattern exists in aion. Two physis nodes exchanging
   coherence snapshots over the existing API is a two-node social graph of
   engines. Only worth building once 1–2 produce inbound interest.

## Track 3 — Content that compounds (background)

- **Screenshot pipeline**: every dashboard (pulse, console quality tab, lineage
  edges) auto-generates shareable visuals. Script a `--screenshot` mode later;
  for now capture manually per release and attach to releases + social posts.
- **Benchmark/accuracy post**: the classification accuracy floor test exists
  (`classification --ignored accuracy_floor`). Publishing real numbers invites
  the roundup-blog ecosystem ("7 open-source AI projects…") that drives star
  spikes; those authors actively look for data-backed projects. One email per
  blogger, zero budget.
- **Integration recipes**: 30-line CrewAI/LangGraph tool wrappers around
  `/api/v1/classify` + `/api/v1/ask`. They get listed in those ecosystems'
  galleries; we inherit their distribution.

## What NOT to do (decided)

- No paid ads, no sponsorships, no launch platforms requiring exclusivity.
- No relicensing to satisfy directories.
- No engagement-farming bots (agent networks ban them; reputation is the asset).

## First 48 hours checklist

- [x] Crate published to crates.io (v0.1.10)
- [ ] GitHub topics added
- [ ] llms.txt committed
- [ ] Registry submission drafted (name, transport=stdio, command=`physis-pro mcp`)
- [ ] One screenshot set exported from /pulse + /console
- [ ] Smithery submission opened

## Sources

- explainx.ai — Top 10 MCP Server Directories & Registries (2026-05)
- mcp.directory — Best Free MCP Servers / deploy guides (2026-02…03)
- eL1fe/mcpdir — aggregation sources incl. official registry stats
- the-agent-report.com — AI Agent Landscape 2026 (57% production figure)
- openclawsocial.org — Moltbook growth timeline (1.5M agents, Jan–Feb 2026)
- github.com/agentgram/agentgram — open-source agent social network
- andrewchen.substack.com — 10 Years After Growth Hacking (channel decay,
  creator/visual distribution)
