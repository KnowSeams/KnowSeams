# Cross-Dialog-Type Transitions

* **Task ID:** cross-dialog-type-transitions_99.claude.md
* **Reviewer:** stevejs
* **Area:** code
* **Motivation (WHY):**
  - Add support for cross-dialog-type D→D transitions (e.g., `'text' "more"`, `(item) [another]`)
  - Currently all dialog states only transition to same-type or generic dialog states
  - Would improve text processing for mixed dialog notation styles
  - New feature beyond basic D→D transition bug fix completed in task 97

* **Acceptance Criteria:**
  1. Find at least one real example of cross-dialog-type transitions in Gutenberg corpus (20K+ files)
  2. Single→Double quote D→D transitions work (e.g., `'done.' "Next task."`)
  3. Parentheses→Bracket transitions work (e.g., `(item) [another]`)
  4. Quote→Parentheses transitions work (e.g., `"text" (aside)`)
  5. All cross-type combinations properly create sentence boundaries when appropriate
  6. Cross-type transitions support both hard (sentence boundary) and soft (continuation) variants

* **Deliverables:**
  - **FIRST**: Search English Gutenberg corpus for real examples of cross-dialog-type patterns
  - Enhanced pattern matching for cross-dialog-type transitions (only if examples found)
  - Test cases covering all major cross-type combinations (only if examples found)
  - Documentation of supported cross-type transition patterns (only if examples found)

* **References:**
  - Task 97 (Dialog-to-Dialog transitions) - completed basic same-type D→D transitions
  - Task 98 (Dialog pattern macro) - provides infrastructure for consistent pattern generation
  - Original cross-type test cases were removed as they represent new functionality

## Implementation Strategy:

### Phase 1: Corpus Analysis (REQUIRED FIRST)
Search the English Gutenberg corpus (20K+ files) for naturally occurring cross-dialog-type transitions:
- Look for patterns like `'text.' "More text"` (single→double quotes)
- Look for patterns like `(aside) [note]` (parentheses→brackets)  
- Look for patterns like `"speech" (thought)` (quotes→parentheses)
- **IF NO EXAMPLES FOUND**: Abandon this task as unnecessary feature creep
- **IF EXAMPLES FOUND**: Proceed with implementation using real examples as test cases

### Phase 2: Implementation (Only if Phase 1 finds examples)
- Current Status: Basic D→D transitions work for all 7 dialog states (same-type transitions)
- Macro infrastructure supports consistent pattern generation across dialog types
- `dialog_open_chars` character class includes all dialog opener types
- Investigation needed to determine why cross-type transitions don't work currently

**Scope:** This is a NEW FEATURE, not a bug fix. Basic D→D functionality is complete.

## Pre-commit checklist:
- [ ] Corpus analysis completed - real examples found or task abandoned
- [ ] All deliverables implemented (if proceeding)
- [ ] Tests passing (`cargo test`)
- [ ] Claims validated (`cargo test -- --nocapture | grep -E "(concurrent|parallel|faster|optimized)"` + manual verification)
- [ ] Documentation updated if needed
- [ ] **ZERO WARNINGS**: `./scripts/validate_warning_free.sh` passes completely