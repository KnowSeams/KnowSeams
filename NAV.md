# KnowSeams — Repository Navigation Guide

**`seams` v0.1.1** — High-throughput sentence extractor for Project Gutenberg texts with dialog-aware detection.

---

## Quick orientation

```
KnowSeams/
├── src/                    # Rust source (library + CLI binary)
├── tests/                  # Integration tests
├── benches/                # Criterion micro-benchmarks
├── benchmarks/             # Python competitor benchmarks
├── tasks/                  # Active tasks (open work)
├── completed_tasks/        # Closed tasks (history)
├── docs/                   # Design docs & process guides
├── exploration/            # Research artifacts, golden datasets
├── scripts/                # Validation & bootstrap helpers
└── .github/workflows/      # CI (warning-free gating)
```

Next task ID: see `tasks/.next_id` (currently **100**).

---

## Source map (`src/`)

| File | Lines | Role |
|------|------:|------|
| `main.rs` | 935 | CLI entry point: argument parsing, pipeline orchestration, debug modes |
| `sentence_detector/dialog_detector.rs` | 1174 | Core state machine — the heart of the project |
| `discovery.rs` | 537 | File discovery: sync, parallel (jwalk), streaming |
| `parallel_processing.rs` | 380 | Parallel file processing, stats aggregation |
| `restart_log.rs` | 261 | Skip-already-complete-file tracking |
| `sentence_detector/abbreviations.rs` | 167 | Abbreviation dictionary to suppress false boundaries |
| `sentence_detector/normalization.rs` | 136 | Text normalization (whitespace, ligatures) |
| `sentence_detector/mod.rs` | 39 | Module re-exports: `Span`, `DetectedSentenceBorrowed` |
| `incremental.rs` | 41 | Aux-file path generation & atomic writes |
| `lib.rs` | 26 | Crate public API surface (re-exports) |
| `bin/generate_gutenberg_sentences.rs` | 324 | Binary: batch sentence extraction tool |
| `bin/generate_boundary_tests.rs` | 235 | Binary: generate boundary test fixtures |

### Module dependency (simplified)

```
main.rs
  ├── discovery.rs          (finds *-0.txt files)
  ├── parallel_processing.rs (processes files, emits FileStats)
  │     └── sentence_detector/
  │           ├── dialog_detector.rs   (DialogStateMachine, SentenceDetectorDialog)
  │           ├── abbreviations.rs     (AbbreviationChecker)
  │           └── normalization.rs     (normalize_sentence)
  ├── incremental.rs        (aux file paths + writes)
  └── restart_log.rs        (RestartLog, should_process_file)
```

---

## Public API (`lib.rs` re-exports)

| Symbol | Source | Purpose |
|--------|--------|---------|
| `DetectedSentenceBorrowed<'a>` | `sentence_detector/mod.rs:26` | A detected sentence (borrowed slice + span) |
| `Span` | `sentence_detector/mod.rs:16` | Byte offset range |
| `FileStats` | `parallel_processing.rs:36` | Per-file sentence counts & timing |
| `generate_aux_file_path` | `incremental.rs:10` | `foo-0.txt` → `foo-0_seams.txt` |
| `create_complete_aux_file` | `incremental.rs:30` | Atomic aux-file write |
| `RestartLog` | `restart_log.rs:10` | Tracks processed files across runs |
| `should_process_file_restart` | `restart_log.rs:121` | Query restart log before processing |

---

## Key types in `dialog_detector.rs`

| Type | Line | Purpose |
|------|-----:|---------|
| `DialogStateMachine` | 219 | Processes text char-by-char, emits sentences |
| `SentenceDetectorDialog` | 1102 | Public façade over `DialogStateMachine` |
| `DialogState` (enum) | 166 | `Narrative`, `DoubleQuote`, `SingleQuote`, `Parenthetical` |
| `MatchType` (enum) | 190 | Pattern match classification |
| `TransitionType` (enum) | 201 | State transition category |
| `PositionTracker` | 106 | Maps byte positions → (line, col) |
| `BytePos` / `CharPos` / `OneBasedLine` / `OneBasedCol` | 16–31 | Newtype wrappers for position kinds |

---

## CLI flags (`seams <PATH> [OPTIONS]`)

| Flag | Description |
|------|-------------|
| `PATH` | Directory (scans recursively for `*-0.txt`) or single file |
| `--force` | Reprocess even files with complete `_seams.txt` |
| `--fail-fast` | Stop on first error |
| `-q / --quiet` | Suppress non-error output |
| `--stats-out FILE` | Write JSON stats (default: `run_stats.json`) |
| `--clear-restart-log` | Reset restart log; reprocess all files |
| `--max-cpus N` | Cap thread count |
| `--sentence-length-stats` | Compute min/max/mean/median/percentiles |
| `--debug` | Emit `_seams-debug.txt` TSV with state transitions |
| `--debug-text "TEXT"` | Debug a string directly on stdout |
| `--debug-stdin` | Debug text piped from stdin |

---

## Tests

| Path | What it covers |
|------|----------------|
| `tests/dialog_detector_tests.rs` | Dialog state machine correctness |
| `tests/pipeline_integration.rs` | End-to-end file pipeline |
| `tests/incremental_processing_integration.rs` | Skip-already-complete logic |
| `tests/overlapped_pipeline_integration.rs` | Discovery/processing overlap |
| `tests/parallel_discovery_benchmark.rs` | Parallel discovery timing |
| `tests/restart_functionality_integration.rs` | Restart log round-trip |
| `tests/stats_output_integration.rs` | JSON stats format |
| `tests/fail_fast_parallel_test.rs` | `--fail-fast` behaviour |
| `tests/boundary_rules/` | JSON fixtures for dialog open/end/narrative rules |
| `tests/integration/` | Shared fixtures and patterns |

Run all: `cargo test`

---

## Benchmarks

| Path | What it measures |
|------|-----------------|
| `benches/file_by_file_bench.rs` | Per-file throughput (Criterion) |
| `benches/detector_instantiation_bench.rs` | `SentenceDetectorDialog` cold-start cost |
| `benches/cache_removal_bench.rs` | Discovery without caching |
| `benchmarks/python_*_benchmark.py` | Competitor baselines (nupunkt, pysbd, spaCy) |
| `benchmarks/run_comparison.py` | Unified comparison runner |

Run Rust benches: `cargo bench`

---

## Docs

| File | Topic |
|------|-------|
| `docs/manual-commands.md` | **Start here for dev commands** |
| `docs/testing-strategy.md` | 10-second budget, I/O coverage |
| `docs/warning-free-compilation.md` | Zero-warning policy & feature gating |
| `docs/exploration-workflow.md` | explore/* branch workflow |
| `docs/boundary-test-harness.md` | How boundary JSON fixtures work |
| `docs/cli-ux-specification.md` | CLI UX intent |
| `docs/stats-output-format.md` | JSON stats schema |
| `SEAMS-Design.md` | Architecture & design decisions |
| `PRD.md` | Product requirements |
| `CLAUDE.md` | Claude/AI workflow guide |

---

## Validation

```bash
./scripts/validate_warning_free.sh   # must pass before any commit
cargo test                            # all tests green
cargo clippy -- -D warnings          # zero warnings enforced
```

CI: `.github/workflows/warning-free-validation.yml`

---

## Active tasks (open work) — `tasks/`

| ID | File | Topic |
|----|------|-------|
| 43 | `cli-ux-improvements_43` | CLI UX polish |
| 54 | `python-api-design-strategy_54` | Python API |
| 55 | `sentence-metadata-statistics_55` | Metadata stats |
| 59 | `stats-output-default-behavior-review_59` | Stats defaults |
| 65 | `duckdb-slim-output_65` | DuckDB output |
| 67 | `polars-pandas-support_67` | Polars/Pandas |
| 68 | `configurable-normalization-and-regex_68` | Config hooks |
| 69 | `python-benchmark-comparison_69` | Python benchmarks |
| 71 | `sentence-detection-throughput-analysis_71` | Throughput analysis |
| 75 | `configurable-file-pattern_75` | File glob config |
| 76 | `configurable-sentence-boundaries_76` | Boundary config |
| 77 | `restart-checkpointing-evaluation_77` | Restart eval |
| 79 | `customer-focused-readme_79` | README rewrite |
| 87 | `dialog-transition-bug-fix_87` | D→D transition bug |
| 90 | `dialog-state-specific-exit-patterns_90` | Exit patterns |
| 93 | `dialog-initial-state-detection_93` | Initial state |
| 96 | `dialog-to-dialog-transition-tests_96` | D→D tests |
| 99 | `cross-dialog-type-transitions_99` | Cross-type dialog |
| 42 | `python-module-spec_42` | Python module spec |

Completed tasks: `completed_tasks/` (98 files, IDs 1–98).
