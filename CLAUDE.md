Claude Workflow Guide

Scope → This guide applies to the Gutenberg Sentence Extractor repository and complements the main PRD. It explains how Claude (or any LLM automation) should propose, document, and commit granular development steps while turning the repo into a Rust‑learning resource.

⸻

1 Philosophy
	•	One thought → one task → one validated commit.  Each atomic improvement must compile, be covered by tests, and land as its own git commit.
	•	Explain why, not just how.  Inline comments focus on rationale—trade‑offs, Rust idioms, async pitfalls—assuming a reader familiar with systems concepts but new to Rust.
	•	Active task management.  Active tasks stay in /tasks; completed tasks move to /completed_tasks/ during the completion commit.
	•	Human in the loop.  Claude drafts tasks; reviewer stevejs approves, amends, or rejects them before merge.

⸻

2 Task Lifecycle

flowchart LR
    idea["Next engineering step"] -->|draft| claude[Claude writes task file]
    claude --> pr[Pull Request + commit]
    pr --> stevejs[Review by stevejs]
    stevejs -->|approve| main

2.1 Exploration Tasks
	•	For research/investigation work, see docs/exploration-workflow.md
	•	Uses explore/<task-id> branches with git finalize automation
	•	Results cherry-picked to main, branches preserved as immutable tags

2.2 When drafting a task Claude must:
	1.	Ensure atomic scope—it must pass tests in isolation.
	2.	Keep active tasks in /tasks; move completed tasks to /completed_tasks/ during completion commit.
	3.	Name the new task file <semantic-name>_<index>.<username>.md, where <semantic-name> is a concise, kebab-case description, <index> is from /tasks/.next_id (increment after use), and <username> prevents multi-user conflicts.
	4.	Commit message should begin with feat: / fix: / docs: etc., include a brief summary, and end with (see tasks/<file>).
	5.	Task ID workflow: read next_id=$(cat tasks/.next_id), create task with that ID, then echo $((next_id + 1)) > tasks/.next_id

⸻

2.3 Warning-Free Build Requirement

**ZERO WARNINGS ACROSS ALL SCENARIOS** — Every development scenario documented in docs/manual-commands.md MUST produce zero warnings.

	•	**Comprehensive Coverage**: All build configurations, test scenarios, feature combinations, and code quality checks must be warning-free.
	•	**No dead_code Suppression**: NEVER use #[allow(dead_code)] — if code isn't used, either use it or remove it.
	•	**Validation Script**: Use scripts/validate_warning_free.sh to verify all scenarios are warning-free.
	•	**Automatic Enforcement**: CI pipeline blocks merges with any warnings.
	•	**Public API Usage**: CLI must actually use all public API functions to prevent false positive dead code warnings.

**Validation Command:**
```bash
./scripts/validate_warning_free.sh
```

This script tests every scenario from docs/manual-commands.md and reports any warnings found. ALL scenarios must pass with zero warnings.

⸻

3 Task File Schema

Each file lives under /tasks/ and follows this template:

# <Task Title>

* **Task ID:** <same as filename>
* **Reviewer:** stevejs
* **Area:** <code|docs|tests|build>
* **Motivation (WHY):**
  - <bullet‑point rationale>
* **Acceptance Criteria:**
  1. <unit tests|integration tests pass>
  2. <behavioural description>
* **Deliverables:**
  - <list of rust files / modules / docs>
* **References:**
  - PRD sections, related tasks, docs links

## Pre-commit checklist:
- [ ] All deliverables implemented
- [ ] Tests passing (`cargo test`)
- [ ] Claims validated (`cargo test -- --nocapture | grep -E "(concurrent|parallel|faster|optimized)"` + manual verification)
- [ ] Documentation updated if needed
- [ ] **ZERO WARNINGS**: `./scripts/validate_warning_free.sh` passes completely

(Claude auto‑fills the template when generating the task.)

⸻

3.1 Implementation Claim Validation

Before committing, validate that code matches any claims made in comments or documentation:
	•	**Concurrency claims** — If code comments mention "concurrent", "parallel", or "batch processing", ensure implementation actually uses concurrent primitives (join_all, spawn, channels) rather than sequential loops.
	•	**Performance claims** — If comments mention speed improvements, throughput gains, or syscall reduction, ensure benchmarks or measurements support the claim.
	•	**Async claims** — If functions are marked async and claim non-blocking behavior, verify they don't block on synchronous operations.

Use `rg` to spot-check claims before commit:
```bash
# Check for concurrency claims near sequential code
rg -A5 -B5 "concurrent|parallel" src/ | rg "for.*in"

# Check for performance claims without validation
rg "faster|optimized|throughput|reduces.*syscalls" src/
```

⸻

3.2 Context Management & Work Boundaries

**Prime Directive: When in doubt, ASK.** The UI shows users when context is low.

**Scope Creep Prevention:**
- **Core Question**: "HAVE WE ALLOWED SCOPE CREEP?" — Ask this at every implementation decision point
- **Minimal Viable Implementation**: Make the framework/feature work correctly in the *simplest* way first
- **Scope Classification**: When scope expansion is detected, ASK user to classify as:
  - **EXTRANEOUS SCOPE** → Remove from current task
  - **DEFERRED SCOPE** → Move to follow-up task  
  - **ESSENTIAL SCOPE** → Keep but acknowledge expansion
- **Decision Template**: "I've implemented [minimal core]. I'm considering adding [additional work]. Should this be DEFERRED to a follow-up task or is it ESSENTIAL for minimal viable completion?"

**Context Management Guidelines:**
- **Early in conversation**: Can handle process-adjacent work (new tests, minor docs updates)
- **Mid-conversation**: Focus on core feature work; ask before process improvements
- **Complex/long conversation**: Ask before any process-adjacent work
- **Uncertain about context usage**: Ask user to check UI and advise on next steps

**Feature vs Process Improvement Boundaries:**
- **Feature work**: Implement features WITH all required process steps (tests, docs, validation, benchmarks)
- **Process improvement work**: Change the development process itself (testing strategy, workflow updates, tooling creation)
- **Rule**: Don't mix process improvements with feature development, but features must complete all process requirements

**Work Chunking Strategy:**
1. **Minimal viable completion first** — Get core functionality working with simplest implementation
2. **Regular scope audits** — At each expansion point, ask "Is this minimal viable completion?"
3. **Separate process improvements** — Handle workflow/strategy changes in dedicated sessions  
4. **Ask before context-heavy additions** — When adding tests/docs would push context > 60%, ask first
5. **Summarize at boundaries** — Preserve state when approaching context limits

⸻

4 Coding Conventions

Topic	Guideline	WHY
Edition	Rust 2021	Widest stable baseline; async‑await mature.
Async runtime	Tokio 1.x	Matches PRD; ecosystem standard; IOCP/epoll under hood.
File IO	tokio::fs & optional memmap2	Non‑blocking default; mmap for benchmark parity.
Sentence Detection	regex-automata crate	High-performance DFA with 149.8 MiB/s throughput.
Progress	Not implemented	Simplifies CLI and removes unused dependencies.
Lock file	Commit Cargo.lock	Reproducible builds and tutorial consistency.
Docs folder	/docs/ for high‑level explainers	Separates narrative docs from code comments.
Tests	cargo test in CI	Ensures every task is validated.
Testing strategy	See docs/testing-strategy.md	10-second budget optimization, I/O test coverage, context management.
Process improvements	See docs/process-improvement-workflow.md	Workflow for changing development process itself.
Warning-free builds	See docs/warning-free-compilation.md	Feature gating and compilation unit ownership for zero-warning builds.

Inline WHY comments use this style:

// WHY: using BufWriter lowers syscalls and boosts throughput on network filesystems


⸻

5 Next‑Step Selection Heuristics

Claude chooses the next task by scanning open tasks & git diff:
	1.	Red state first.  Fix failing tests or clippy warnings before new features.
	2.	Shortest critical path.  Prioritise tasks unblocking others (e.g., DFA loader before progress bars).
	3.	Knowledge debt.  Add docs when unfamiliar Rust design is introduced.

⸻

6 Example Commit Sequence

Commit	Task	Sample Message
d4e5b3c	dfa-loader_1	feat: load sentence DFA at startup (see tasks/dfa-loader_1.md)
c3a9fed	async-reader_2	feat: async buffered read for source files (see tasks/async-reader_2.md)
…	…	…

⸻

This document is version‑controlled. Amend via PRs when conventions evolve.