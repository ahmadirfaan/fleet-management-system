Design and ship a small but complete real-time system — backend, frontend, and everything in between — using the AI tooling of your choice. We evaluate what you shipped and whether you can defend every decision behind it at a high-level, conceptual level.

- **Timeline**: 7 working days
- **Scope**: End-to-End
- **AI Usage**: 100% allowed

---

## B. Table of Contents
1. What This Assessment Measures
2. The Problem — Mine Fleet Live Tracker
3. Scope & Deliverables
4. AI Tooling — Our Expectations
5. How We Evaluate

---

## 1. What This Assessment Measures
*Read this first. It changes how you should approach the challenge.*

### Context — The Work This Role Involves
We are runs several production systems; the Fleet Management System (FMS) is the one this assessment is based on. The work involves shipping production features end-to-end across these systems at a pace that is only achievable when a senior engineer is driving AI tooling effectively. Daily use of Claude Code, Cursor, Copilot, or an equivalent is expected. That is the job.

But using AI heavily does not mean "vibe coding." The code that reaches production must be maintainable by teammates, debuggable at 2 AM on-call, and extensible six months from now by someone who has never read it before. That bar does not move just because an LLM wrote the first draft.

What this assessment really measures is a specific judgment-and-taste skill set:

**1. Architectural Judgment**
You can look at a problem and choose the right shape for the solution — which pieces should be separate, which should be together, what talks to what, and where the boundaries live. You can explain why your shape is better than two plausible alternatives.

**2. Conceptual Depth**
You must genuinely understand async, ownership, concurrency, backpressure, state, and side-effects. This is non-negotiable. If you cannot reason about how your code will behave under load, you cannot catch an AI when it is confidently wrong — and you cannot ship production code, regardless of which tool generated the first draft.

**3. Maintainability Discipline**
Your code reads like a careful human wrote it. Names mean something. Modules have one job. Errors are typed. Failure paths exist. There is no dead code, no commented-out experiments, no AI boilerplate that nobody understands anymore.

**4. AI-as-Collaborator Fluency**
You can decompose a problem into instructions an AI can execute well, spot when the output is subtly wrong, and rewrite what doesn't meet your bar. You treat the AI as a fast junior pair — useful, supervised, never trusted blindly.

**5. Domain Curiosity & Failure Imagination**
We give you a brief, not a spec. The real mining domain has failure modes the brief will never list: GPS drift in a pit, sensor faults vs. sensor noise, trucks stuck between states, mixed-up load reports. You go looking for edge cases, not waiting for them. Curiosity about the problem space is part of the role, not an extra.

**The One-Line Test**
If a teammate opens your submission six months after you've left the team, can they fix a bug in it without rewriting everything first? That's the bar. Every decision you make in this challenge should move toward "yes."

---

## 2. The Problem — Mine Fleet Live Tracker
One operator. Five haul trucks. One screen. Real time.

### Context
A dispatcher in a mine control room needs to know, right now, where every haul truck is, what it's doing, and whether anything is wrong.

Build a simplified fleet dashboard: real-time vehicle telemetry flowing from a simulator into a backend, then pushed out to an operator's dashboard — all running on a reviewer's laptop.

Simulate 5 haul trucks in an open-pit mine, cycling through realistic states: `loading zone → haul road → crusher → return → idle`. Each truck emits GPS, speed, engine RPM, load status, and fuel level at a rate you choose (single-digit Hz is plenty).

The backend ingests, stores current state, detects unsafe conditions, and pushes updates to the dashboard. The dashboard shows the fleet on a map in real time and lets the operator drill into any truck's recent history.

### For context — what we actually run
FMS System ingests telemetry from real haul trucks across real mines. Our production stack is Rust on the backend, React + TypeScript on the frontend, Protobuf on the wire, CesiumJS for 3D geospatial, and SSE for pushing state to browsers. You are welcome to align with any of this — but only the Rust backend is required. Everything else is your decision.

### The Only Required Choice: BACKEND - RUST
The backend must be written in Rust. Framework, runtime, crates, and internal structure are all yours to pick. This is the one stack constraint in the entire challenge.

### Everything Else Is Your Call
Simulator language, wire format, transport, frontend framework, map library, state model, storage, endpoint shape, testing stack — all of it. Pick what you believe produces the best outcome for this problem under these constraints.

### The Bar for Every Decision
Every choice you make outside "Rust backend" must be defended in the README with a strong reason: what the option buys, what it costs, and why it is the right fit for this problem. A deliberate, well-argued decision is a strong positive signal. A random pick with no reasoning is a strong negative signal — even if the choice happens to be good. We evaluate the reasoning as much as the result.

---

## 3. Scope & Deliverables
Fixed time, variable scope. Pitch-style: we give you the shape, you scope and ship.

### Ready-to-Run is Part of the Deliverable
Reviewers will run your submission, not just read it. Make sure it comes up cleanly on a fresh machine with only the instructions you provide. A single-command bring-up (docker compose up, a make dev target, a justfile, or a shell script) is strongly preferred. At minimum: list prerequisites with versions, the exact commands in order, and any config needed. No hidden env vars, no local-only paths.

### Required — Backend & Data

**1. Rust Backend Service**
Ingests telemetry from the simulator, holds current fleet state, pushes updates to connected clients, and serves reads for current state and per-vehicle history. The framework, in-memory state model, streaming transport, and endpoint shape are yours to pick and justify. Rust is the one fixed constraint — everything inside it is your decision.

**2. Vehicle Simulator**
A separate process that emits telemetry for ~5 haul trucks cycling through realistic mine states with plausible GPS tracks. Language, emission rate, and state machine are yours — but the behavior must look like five different trucks doing different things, not five copies of the same truck.

**3. Wire Contract**
A deliberately designed schema for telemetry messages, vehicle state, and any aggregate snapshots. Format (Protobuf, JSON, MsgPack, Cap'n Proto, or otherwise) is your choice and must be defended. Naming conventions, optional/required choices, and a short evolution story (how you'd add a field in 6 months without breaking clients) should be conscious choices.

**4. Health Classification**
Detect unsafe or unhealthy conditions from the telemetry stream — sustained over-rev, unsafe speed under load, excessive idle time, fuel anomalies, or anything else a dispatcher should see. Rule-based is enough. Thresholds and categories are yours to pick and defend.

### Required — Frontend & Ship

**5. Operator Dashboard**
A live map of the fleet with status-coloured markers, drill-down into a selected vehicle (current stats and recent trail), and a fleet summary panel. Frontend framework, map library, and update model are your call. The UI should feel responsive — updates must not cause visible re-render storms or UI jank. How you achieve that is your decision; we will read the code for how you thought about it.

**6. History View**
A per-vehicle path over time with a way to scrub through it and see how state and speed evolved. Visual treatment is yours.

**7. Tests**
Meaningful tests across backend and frontend — at least one failure path, one concurrency case, and one boundary condition. Happy-path-only tests do not count.

**8. README**
The README is part of the evaluation, not a footnote. It must cover:
- Architecture overview — one diagram, one paragraph. What runs where, how data flows.
- How to run it — exact commands, prerequisites, and versions. Meet the "ready to run" bar defined above. One-command bring-up (Docker Compose, Make, etc.) is strongly preferred.
- Stack decisions with rationale — for every choice outside "Rust backend": what you picked, why it beats the obvious alternatives, and what you would pick differently at 10× scale.
- AI usage log — honest and specific. See Section 04.
- What you would do next — the two or three things you consciously did not ship, and why.

### Optional — Stretch
These are adjacent to the real stack. They signal curiosity and alignment. Half-finished stretch items count against you — if you ship one, it must genuinely work end-to-end:
- Tauri desktop shell
- 3D terrain
- Geofence alerts
- Alt transport (Zenoh / QUIC)
- ONNX inference in the loop
- Offline-first resilience

### What "Done" Looks Like
A working end-to-end system that survives being poked. A reviewer can install it, run it, and try to break it. We will actively try — both system-level failures (simulator dying mid-run, client disconnecting, burst traffic, malformed messages) and domain-level edge cases (GPS dropouts, sensor noise that shouldn't trigger alerts, trucks stuck between state transitions, load telemetry that disagrees with itself). The brief above describes the shape of the problem. It does not list every edge case — finding them is part of what we evaluate. Show that you thought about the domain, not just the happy path. Broken features in your submission count against you, not for you. Ship less, but ship it working.

### On Pace
We do not expect you to finish this as fast as possible. Use the two weeks you have. Turn the extra time into deeper thinking, stronger edge-case handling, a cleaner README, and better tests. We'd rather see the best quality you can shape in two weeks than a rushed submission delivered early.

---

## 4. AI Tooling — Our Expectations
*This is the single most important section of this document. Read it carefully.*

Yes, use AI. Use it aggressively. Claude Code, Cursor, Copilot, Aider, ChatGPT — use whatever you use at work. We are not evaluating whether AI was used. We are evaluating whether you understood what you shipped, whether the architecture reflects your judgment, and whether the code meets our production bar.

### What "AI Agent Manager" Means to Us
The role this assessment targets is not "person who prompts AI and pastes the output." It is "senior engineer who directs AI toward production-grade outcomes." The distinction matters because one of those is replaceable by the AI itself within a year, and the other is what we are actually evaluating.

### What Strong AI Collaboration Looks Like
- You decomposed the problem yourself before prompting.
- You generated code in small, reviewable increments.
- You rewrote AI output when it did not meet your taste.
- You caught at least one case where the AI was subtly wrong.
- You can explain every architectural decision on a whiteboard.

### Required: AI Usage Log in the README
A short, honest log. Not a confessional, not a showcase. Just the truth. At minimum:
a) What you delegated to AI — e.g., "Claude generated the initial Protobuf-to-Rust wiring and the SSE handler skeleton."
b) What you wrote or heavily rewrote yourself — e.g., "The state-machine for truck lifecycle, the backpressure handling, and the React update batching were mine."
c) One concrete case where the AI was wrong — what it suggested, how you caught it, what you did instead. This is the single highest-signal paragraph in the whole submission.

---

## 5. How We Evaluate
Weighted scoring across eight dimensions. Architecture, maintainability, conceptual ownership, and domain thinking are weighted highest — that reflects the role.

| Signal | Weight | What We Read For |
| :--- | :--- | :--- |
| **Architectural Judgment** | 20% | The shape of the system, and whether it looks like the result of deliberate thought. We read the code for where things live, how they talk to each other, and where the seams are — and we read the README for whether you can defend this shape against the obvious alternatives. |
| **Code Maintainability** | 15% | Whether your code reads like someone cared about the next person to open it. We don't prescribe a style; we read for whether one exists. |
| **Production Readiness** | 15% | How the system behaves when things go wrong, and whether someone on-call could figure out what happened. Posture toward failure matters more than any specific library or pattern. |
| **Conceptual Depth** | 15% | The system handles concurrent work, shared state, and live streams of data. We read for whether the code is correct under those conditions — or whether it merely happens to work on the happy path. We also read for whether you understand why your code behaves the way it does, not just that it appears to. |
| **Domain Exploration & Edge Cases** | 10% | The brief is a shape, not a spec. We read for what you discovered by thinking about the real-world context these trucks operate in — and whether you can explain why each edge case you surfaced matters to a dispatcher. Exploration is part of the signal. |
| **Decision Defense (README)** | 10% | For every meaningful choice: what you picked, why it beats the alternatives you considered, and what you'd pick differently at 10× scale. Tradeoffs articulated beat outcomes described. |
| **Honest AI Usage** | 8% | Specific, concrete, honest. The "where the AI was wrong" paragraph carries more weight than anything else in this slot. |
| **Scoping & Commit History** | 7% | What you chose to ship working versus what you left out, and whether your commit history reads like someone thinking across two weeks rather than a single dump at the end. |

### Hard Fails
Any one of these ends the review regardless of other strengths:
- The reviewer cannot bring the system up end-to-end on a fresh machine using only your documented instructions.
- You cannot explain or walk through your own code when asked.
- No AI usage log — or a fake one.
- A single "initial commit" containing the entire project.
- Obvious copy-paste of someone else's public solution.

---