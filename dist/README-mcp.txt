Physis — MCP server (standalone)

Exposes the Physis engine as an MCP service on 127.0.0.1:9876.
This is the same engine that powers Praxis, so it serves both the
"Physis MCP" and "Praxis MCP" roles.

Run:
    ./physis-pro serve

Then point an MCP client (Claude Desktop, an agent, etc.) at
http://127.0.0.1:9876 (or the SSE/streamable endpoint your client expects).
Tools include: classify text against the semiotic grid, list/query the
ontology, run the dream loop, and inspect coherence/quality state.

State persists under ~/.physis-core/. Classifier uses the deterministic
RandomProjectionEmbedder (no model download required).
