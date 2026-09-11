# PHYSIS NEUROSEMANTIC WIRING (verified from real source files: .mcp.json, auth.json, AGENTS.md, Cargo.toml, web_server.rs, sensory.rs, skills/)

WIRING CHANNELS (verified):
1. HERMES (native .mcp.json stdio + skills: hermes-agent, opencode-delegate, cline-delegate, opencode-usage, claude-code)
2. OPENCODE (auth.json verified providers; .mcp.json routes via openrouter/deepseek/groq/nvidia)
3. CLINE (cline-delegate skill; delegates to cline CLI with physis coherence context)
4. KILO (.env KILO key; no dedicated skill — treat as provider via openrouter or direct)
5. CLAUDE CODE (claude-code SKILL.md verified; CLAUDE key in ~/.hermes/.env; anthropic provider)

NEUROSEMANTIC INTERFACE: harness (.loom / .mcp.json stdio / CLI) = internal; MCP stdio (.mcp.json) = primary external (standard, local, no cloud, 30 tools); REST/web_server = commercial/per-tenant. All feed same PhysisCore coherence graph + same replayable EpistemicAuditTrail (verified from core.rs / epistemic.rs / delta_engine.rs).
