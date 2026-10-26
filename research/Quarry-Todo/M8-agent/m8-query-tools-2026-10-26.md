---
id: "m8-query-tools-2026-10-26"
status: "done"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-27T09:00:00.000Z"
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
- [x] A 100k-triangle scene tree comes back under 4,000 tokens.
- [x] Asking for vertices of one object returns a page, not the whole mesh.
- [x] Every query has a token estimate in the response envelope.

## Done 2026-10-27

- Every `query.*` reply is checked by the registry: it carries a `tokens` estimate (characters / 4), and anything over 4,000 is refused with "Narrow it: use offset and limit, a smaller depth, or ask about one object."
- `query.scene(depth?, offset, limit?)` pages top-level objects (100 by default) with `total` and `more`; new `query.object(object)` (transform in Euler degrees, bounds, counts, skin, marker, collision, properties) and `query.mesh(object, part: vertices|faces, from, count?)` (100 by default, 1,000 max, values rounded to 0.1 mm). Selection, skeleton, clip, images and history queries already existed.
- Tested (`crates/quarry-core/tests/query.rs`): 450 spheres, over 100k triangles: the scene tree is under 4,000 tokens with `more`; vertices come 20 at a time from index 10; a 1,000-object page is refused with the hint.
