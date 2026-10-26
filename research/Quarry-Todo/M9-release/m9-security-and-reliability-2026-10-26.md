---
id: "m9-security-and-reliability-2026-10-26"
status: "todo"
priority: "medium"
assignee: "safi"
dueDate: null
created: "2026-10-26T09:57:15.000Z"
modified: "2026-10-26T09:57:15.000Z"
labels: ["m9", "nfr", "security"]
order: 76
---

# M9 · No-network proof and crash reliability

NFR-03 and NFR-04.

**Do**
- Prove no network: test or trace showing no outbound connections, no telemetry, no update check; MCP local socket only.
- Fuzz or hand-corrupt project files, images and settings: never crash, always a plain error.
- Crash test: kill mid-edit, worst case loses the last 2 minutes, original file intact.
- Confirm the agent cannot write outside the project folder (repeat the M8 checks on a packaged build).

**Done when**
- [ ] The no-network check runs in CI or is documented as a release step.
- [ ] A corrupt file of each kind opens or fails with a message, never a panic.
- [ ] No secret or absolute user path is written into a project file.
