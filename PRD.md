Product Requirements Document (PRD)

1 Purpose

Provide a high-throughput CLI tool that scans a local mirror of Project Gutenberg, tokenises each “*-0.txt” UTF-8 text into sentences using a sentence-boundary detector built from a startup-generated sentence boundary detector (DFA), and writes an auxiliary file alongside each source containing normalised sentences and span metadata. The tool must support fully asynchronous I/O and be safe to re-run incrementally.

2 Glossary

Term	Definition
DFA	Deterministic Finite Automaton compiled at startup from sentence-boundary patterns.
Sentence normalisation	Removal of intra-sentence hard line breaks (\n, \r\n) with whitespace collapsed to a single space. Everything else remains byte-exact.
Aux file	<orig>_seams.txt written next to its source.
Span	(start_line, start_col, end_line, end_col)—all one-based, columns measured in Unicode scalar values (characters).

3 Scope

3.1 In Scope
	•	Discover every file matching the glob **/*-0.txt under a user-supplied root.
	•	For each source file:
	•	Skip if a complete aux file already exists, unless --overwrite_all.
	•	Overwrite a partial aux file.
	•	Produce per-run statistics and aggregate them in run_stats.json.
	•	Provide a --fail_fast switch to abort on first error; otherwise continue and log.
	•	Use memory-mapped I/O for high-performance file processing with parallel execution.

3.2 Out of Scope (Now)
	•	Distributed processing across hosts.
	•	Optimising the DFA beyond linear-time guarantees.
	•	Sentence boundary training—assume rule set is externally supplied.

4 Functional Requirements

ID	Requirement
F-1	CLI accepts: root_dir, --overwrite_all, --fail_fast, --quiet, --stats_out.
F-2	Recursively locate and stream-process every *-0.txt.
F-3	At startup, compile sentence-boundary patterns into a high-performance DFA stored in memory for all worker tasks.
F-4	Read each file with memory-mapped I/O for optimal performance with parallel processing.
F-5	Detect sentence boundaries with the DFA, producing: index<TAB>sentence<TAB>(start_line,start_col,end_line,end_col) per line.
F-6	Normalise sentences by removing hard line breaks; treat \r\n as single break; preserve all other bytes.
F-7	Write results via async buffered writer to <path>_seams.txt.
F-8	Generate per-file stats (chars processed, sentences, wall-clock ms) and aggregate into run_stats.json with total chars/sec.
F-9	Skip processing when aux file exists and completes without truncation; detect partial files by trailing newline + EOF.
F-10	Respect --fail_fast: abort entire run on first I/O/UTF-8/DFA error.

5 Non-Functional Requirements

Category	Requirement
Tech Stack	Rust (2021 edition); sentence DFA via regex-automata crate; built with cargo --release. See architecture documentation for implementation details.
Performance	Demonstrably faster than established Python alternatives (pysbd, spaCy sentencizer) on equivalent hardware; throughput benchmarks documented with hardware specifications.
Scalability	Should saturate multiple cores using Tokio’s work-stealing scheduler.
Portability	Linux (x86-64, aarch64) and Windows 10+.
Reliability	Deterministic output; idempotent reruns.
Observability	Structured logs (JSONLines) and run_stats.json. Progress bars not currently implemented.

6 Error Handling
	•	Log all recoverable errors with context.
	•	For non---fail_fast runs, continue after logging, mark file as failed in stats.

7 CLI & Config

seams <root_dir>
    [--overwrite_all]   # overwrite even complete aux files
    [--fail_fast]       # abort on first error
    [--stats_out <path>]# default: run_stats.json in CWD


8 Acceptance Criteria
	1.	Unit tests covering DFA boundary detection and normalisation rules.
	2.	Golden-file tests on at least five Gutenberg texts; diff is empty.
	3.	Throughput benchmark meets performance NFR on reference hardware.
	4.	run_stats.json includes per-file counts plus aggregate chars/sec.
	5.	Re-run without --overwrite_all touches zero unchanged aux files.
	6.	--fail_fast causes exit code ≠ 0 after first injected UTF-8 error.

9 Milestones

Milestone	Deliverable	Target Date
M1	CLI skeleton; file discovery; async read/write
M2	DFA pattern compilation; integration tests
M3	Stats aggregation; mmap mode; perf bench
M4	Docs, CI, release v0.1

10 Risks & Mitigations

Risk	Likelihood	Impact	Mitigation
DFA patterns change frequently	M	M	Hot-reload DFA per file batch when patterns change.
Very large files cause memory spikes	L	M	Stream processing and bounded buffers.
Windows newline variants mishandled	M	L	Normalisation unit tests on mixed CRLF data.

11 Python Module Requirements

The seams functionality shall be available as a Python module to enable programmatic usage:

Category	Requirement
Package	Separate PyPI package (knowseams or variant) with native extensions via PyO3/maturin
API Style	Class-based primary API (SentenceDetector, FileDiscovery) with functional convenience wrappers
Core Features	Full sentence detection with span metadata, file processing, async corpus processing
Performance	DFA reuse across operations, async I/O pipeline, both buffered and mmap backends
Distribution	Python wheels for Linux (x86_64/aarch64), Windows, macOS
Dependencies	Depends on seams crate published to crates.io as pure Rust library

Python API design TBD - see task python-api-design-strategy_54.stevejs.md for current design evaluation of CLI wrapper vs native bindings approaches.

12 Open Questions
	1.	Which concrete DSL or regex subset will define the sentence-boundary spec?
	2.	Should stats include per-sentence length histograms? → See task sentence-metadata-statistics_55.stevejs.md
	3.	Progress bars feature removed - simplifies CLI and removes unused dependency
	4.	Future: move from DFA to PDA for cross-paragraph context?

13 Open Source Publication Requirements

13.1 Publication Goals
	•	Technical Showcase: Demonstrate high-performance text processing implementation
	•	Performance Leadership: Demonstrate superior throughput vs. established Python alternatives (pysbd, spaCy sentencizer) with hardware-contextualized benchmarks validated on every checkin
	•	Cold Start Performance: Showcase superior initialization performance compared to existing solutions
	•	Learning Documentation: Document Rust implementation journey as adaptation/problem-solving demonstration
	•	Narrative Processing Optimization: Highlight specialization for narrative sentences requiring spans from original documents

13.2 Repository Structure Requirements

ID	Requirement
R-1	Top-level README optimized for target audience (see separate task)
R-2	LICENSE file (MIT license)
R-3	CONTRIBUTING.md with minimal guidelines: "fork it, ping me"
R-4	Architecture documentation explaining async approach and sentence detection algorithm design
R-5	Performance benchmarks validated on every checkin with hardware specifications and comparative methodology documented
R-6	Example workflows for narrative analysis pipeline integration

13.3 CLI-First User Experience

ID	Requirement
UX-1	Simple installation via `cargo install seams`
UX-2	Comprehensive `--help` documentation with examples
UX-3	Helpful error messages with actionable suggestions
UX-4	Shell completion support (bash/zsh/fish)
UX-5	Progress feedback that works well in CI/pipeline contexts
	(see docs/cli-ux-specification.md for detailed requirements)

13.4 Release Strategy

ID	Requirement
REL-1	Semantic versioning (SemVer) for breaking changes
REL-2	v0.1 release: feature complete but rough edges acceptable
REL-3	Cross-platform binaries for macOS, Windows, Linux (x86_64/aarch64)
REL-4	Crate publication to crates.io
REL-5	Release notes documenting performance characteristics and breaking changes

13.5 Documentation Requirements

ID	Requirement
DOC-1	Performance benchmarks with every-checkin validation
DOC-2	Cold start performance measurements and explanation
DOC-3	Architecture decisions documentation (async design, DFA choice)
DOC-4	AI collaboration development process (see docs/ai-collaboration.md)
DOC-5	Benchmark methodology documentation (see docs/benchmark-methodology.md)
DOC-6	Integration examples for narrative analysis workflows

13.6 Quality Assurance

ID	Requirement
QA-1	Local testing covers all functionality (no GitHub CI required)
QA-2	Cross-platform testing on author's available hardware
QA-3	Performance regression detection through local benchmarks (every checkin)
QA-4	Documentation accuracy verification