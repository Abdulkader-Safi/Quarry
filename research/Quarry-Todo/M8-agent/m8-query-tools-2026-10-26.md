---
id: "m8-query-tools-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m8", "mcp"]
order: 67
---

# M8 · Compact query tools with paging

FR-MCP-02 and NFR-08. An agent should never have to dump the whole scene to ask one question.

**Build**
- `query.scene`, `query.object`, `query.mesh`, `query.selection`, `query.skeleton`, `query.clip`, `query.images`, `query.history`.
- Compact JSON, paging (`offset`, `limit`), and full vertex lists only when asked by object and range.
- Hard 4,000-token cap by default with a "narrow the query" hint when hit.

**Done when**
- [ ] A 100k-triangle scene tree comes back under 4,000 tokens.
- [ ] Asking for vertices of one object returns a page, not the whole mesh.
- [ ] Every query has a token estimate in the response envelope.
