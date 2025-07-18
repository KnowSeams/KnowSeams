# Seams - Lightning-fast detection of natural breaks in dialog-heavy narrative text

[![License: MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg)](https://opensource.org/licenses/MIT)

Splits large English text corpora into meaningful sentences while preserving narrative flow and dialog structure.



## Narrative Sentence Splitting 

SEAMS excels at preserving narrative structure where other tools break dialog incorrectly:

**Input text:**
```
"Well, you can see him easily enough," said Mr. Hoad. "He's staying in
your village, I believe. He's a nephew of Squire Broderick's."

"What! Captain Forrester?" cried I.
```

**SEAMS output (3 sentences):**
1. `"Well, you can see him easily enough," said Mr. Hoad.`
2. `"He's staying in your village, I believe. He's a nephew of Squire Broderick's."`
3. `"What! Captain Forrester?" cried I.`

**Other tools break this into 6+ fragments:**
- **pysbd**: Breaks mid-dialog (`"He's staying in` + `your village, I believe.`)
- **nupunkt**: Splits attribution (`"What!"` + `Captain Forrester?"` + `cried I.`)
- Both fragment quotes and lose dialog structure across paragraph breaks

See [examples](docs/examples.md) for more cases demonstrating:
- **Dialog spanning paragraph separators** - Dialog attribution stays connected across paragraph breaks
- **Paragraph separators indicating end of never-closed quote** - Letter format with implicit quote boundaries

## Help break SEAMS!

**Found a mis-split in English narrative? Show us!**

No currently known mis-splits on 20K Project Gutenberg English texts. Python scripts in `exploration/` help search for potential examples.

If you discover a counter-example:

1. Grab the smallest passage that triggers the error
2. Paste it into a new GitHub issue
3. We'll reproduce it, fix it, and add the case to the public test corpus

Dialog-heavy text is where other sentence splitters fail - show us where SEAMS does too.

## Benchmarks

**Test Corpus:** 20,440 Project Gutenberg files (7.4 billion characters, 56 million sentences)  
**Test System:** Intel i9-13900KF (16 cores, 32 threads) running Linux 5.15 WSL2, 32GB RAM

| Benchmark (version) | Cores | End-to-end time | Speed-up vs nupunkt | Sentences / s | Sentence detection throughput | Total e2e throughput | Note |
|---------------------|:----:|---------------:|--------------------:|--------------:|-----------------------------:|--------------------:|------|
| **seams** | 32 | **6 s** | **59 ×** | **8.6 M** | 105.4 MB/s | **1176.2 MB/s** | line offsets included |
| seams-single-cpu | 1 | 1 m 31 s | 4 × | 611 k | **450.6 MB/s** | 90.4 MB/s | single-CPU baseline |
| nupunkt (0.5.1) | 1 | 6 m 23 s | 1 × | 179 k | 19.7 MB/s | 19.3 MB/s | pure-Python |

**Additional Results:** See [benchmarks/performance-results.md](benchmarks/performance-results.md) for results across different systems including macOS ARM64.

For complete benchmark methodology and comparison tools, see [benchmarks/](benchmarks/) and run `python run_comparison.py`.

## Quick Start

### Installation

**From crates.io:**
```bash
cargo install seams
```

**From source:**
```bash
git clone https://github.com/KnowSeams/KnowSeams.git
cd KnowSeams
cargo install --path .
```

### Basic Usage

Process all Project Gutenberg texts in a directory:
```bash
seams /path/to/gutenberg_texts
```

The tool will:
- Find all `*-0.txt` files recursively
- Extract sentences with boundary detection
- Write results to `*_seams2.txt` files alongside originals
- Generate processing statistics in `run_stats.json`

## Usage Examples

**Process a Project Gutenberg mirror:**
```bash
seams ~/gutenberg_mirror
```

**Reprocess all files (ignore existing _seams2.txt outputs):**
```bash
seams --overwrite-all ~/gutenberg_texts
```

**Run benchmark comparison:**
```bash
cd benchmarks
uv venv                                    # One-time setup
uv sync                                    # Install dependencies
source .venv/bin/activate                  # Per session
python run_analysis.py ~/gutenberg_texts  # Assumes location of a (likely partial) gutenberg mirror
```

**Debug sentence detection with state transitions:**
```bash
seams --debug-text 'He said "Hello world!" and left. She replied "Goodbye!" quickly.'
```

Output shows internal state machine transitions:
```
0	He said "Hello world!" and left.	(1,1,1,34)	Narrative	DialogDoubleQuote	Continue	 "	IndependentDialog[0]	He said "Hello worl
0	He said "Hello world!" and left.	(1,1,1,34)	DialogDoubleQuote	Narrative	Continue	!" a	DialogSoftEnd	llo world!" and left. S
1	She replied "Goodbye!" quickly.	(1,35,1,67)	Narrative	Narrative	Split	. S	NarrativeSentenceBoundary	" and left. She replied
1	She replied "Goodbye!" quickly.	(1,35,1,67)	Narrative	DialogDoubleQuote	Continue	 "	IndependentDialog[0]	he replied "Goodbye!"
1	She replied "Goodbye!" quickly.	(1,35,1,67)	DialogDoubleQuote	Narrative	Continue	!" q	DialogSoftEnd	 "Goodbye!" quickly.
```

### Output Format

For each input file `book-0.txt`, seams creates `book-0_seams2.txt` with:
```
1	This is the first sentence.	(1,1,1,32)
2	Here is the second sentence.	(1,33,2,15)
```

Format: `index<TAB>sentence<TAB>(start_line,start_col,end_line,end_col)`
- Line and column numbers are 1-based
- Sentences are normalized (line breaks removed, whitespace collapsed)
- Span coordinates refer to the original text

### Command Reference

```
seams [OPTIONS] [PATH]

Arguments:
  [PATH]  Directory to scan recursively for *-0.txt files, or single *-0.txt file to process

Options:
      --overwrite-all                   Reprocess all files, even those with complete _seams.txt files
      --fail-fast                       Stop processing immediately on first I/O, UTF-8, or detection error
      --no-progress                     Disable progress bars (useful for automation/CI)
  -q, --quiet                           Suppress all non-error output (implies --no-progress)
      --stats-out <FILE>                Write performance statistics to JSON file [default: run_stats.json]
      --clear-restart-log               Clear the restart log and reprocess all files
      --max-cpus <MAX_CPUS>             Limit processing to specified number of CPUs/threads
      --sentence-length-stats           Calculate and display sentence length statistics
      --debug-seams                     Generate debug TSV files with state transition details
      --debug-text <DEBUG_TEXT>         Debug sentence detection on provided text string
      --debug-stdin                     Debug sentence detection on text from stdin
  -h, --help                            Print help
  -V, --version                         Print version
```

## Performance

- **End-to-end throughput:** 1176 MB/s multi-threaded (complete pipeline: file discovery, reading, boundary detection, span tracking, normalization, and writing output)
- **Sentence detection:** 451 MB/s single-threaded (pure boundary detection + line coordinate tracking)
- **Single-threaded end-to-end:** 90 MB/s (baseline for fair comparison)
- **Parallel processing:** Uses available CPU cores for file enumeration and sentence splitting
- **Memory efficiency:** Memory-mapped files for large corpora
- **Incremental:** Skip already-processed files automatically

Performance scales with available CPU cores and I/O bandwidth. Actual throughput varies by:
- **Hardware:** CPU cores, storage speed, memory bandwidth
- **File characteristics:** Size distribution, text complexity
- **Workload:** Complete pipeline vs. raw boundary detection only


## Technical Details

**Algorithm:** DFA-based boundary detection using regex-automata with narrative-aware heuristics for dialog coalescing.

**Performance:** 23× faster than nupunkt single-threaded (451 MB/s vs 20 MB/s sentence detection). Multi-threaded end-to-end throughput reaches 1176 MB/s on test system.

**Architecture:** Two-stage pipeline with bounded parallelism (file enumeration + sentence splitting), async I/O with memory-mapped files.

For detailed design documentation, see [SEAMS-Design.md](SEAMS-Design.md).

## License

MIT License - see LICENSE file for details.

## Acknowledgments

Thanks to Project Gutenberg for providing the freely available corpus used for testing and benchmarking.