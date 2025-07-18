# External Definitive Punctuation Patterns

* **Task ID:** external-definitive-punctuation_95.claude.md
* **Reviewer:** stevejs
* **Area:** code
* **Motivation (WHY):**
  - Current dialog detector missing external definitive punctuation patterns (`"[.!?]` vs existing `[.!?]"`)
  - Failing three-sentence test cases where punctuation after dialog close should create boundaries
  - SEAMS-Design.md lines 389-392 identify this as core missing functionality
  - Required for complete pattern coverage per SEAMS-Design.md lines 221-225

* **Acceptance Criteria:**
  1. All dialog states support external definitive punctuation patterns (`{close}[!?]`)
  2. External contextual punctuation patterns (`{close}\.`) with abbreviation checking
  3. Case-sensitive handling (capital letters signal split, lowercase signal continue)
  4. Three-sentence test cases pass: `Text "word"! More text. New sentence.` → 3 sentences
  5. Unit tests cover all new pattern combinations

* **Deliverables:**
  - Updated `src/sentence_detector/dialog_detector.rs` with external punctuation patterns
  - Pattern additions for all 7 dialog states (quotes, smart quotes, parentheses, brackets, braces)
  - Test cases validating external punctuation behavior
  - Pattern priority ordering prevents conflicts with existing patterns

* **References:**
  - SEAMS-Design.md lines 389-392 (missing external definitive punctuation)
  - SEAMS-Design.md lines 221-225 (named component specification)
  - SEAMS-Design.md lines 325-332 (target test cases)

## Pre-commit checklist:
- [ ] All deliverables implemented
- [ ] Tests passing (`cargo test`)
- [ ] Claims validated (`cargo test -- --nocapture | grep -E "(concurrent|parallel|faster|optimized)"` + manual verification)
- [ ] Documentation updated if needed
- [ ] **ZERO WARNINGS**: `./scripts/validate_warning_free.sh` passes completely