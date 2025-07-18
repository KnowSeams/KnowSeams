# Dialog-to-Dialog Transition Tests (Test-First)

* **Task ID:** dialog-to-dialog-transition-tests_96.claude.md
* **Reviewer:** stevejs
* **Area:** tests
* **Motivation (WHY):**
  - Current implementation only supports Dialog->Dialog transitions for double quotes, not other dialog types
  - SEAMS-Design.md mentions zero-character separators for bracket pairs (`)(`, `][`, `}{`)
  - Inconsistent behavior: `"first" "second"` works but `'first' 'second'` doesn't
  - Test-first approach: create failing tests to document expected behavior before implementation
  - Need clear distinction between Split vs Continue vs Dialog->Dialog transition handling

* **Acceptance Criteria:**
  1. Create failing unit tests for Dialog->Dialog transitions across ALL dialog states
  2. Test both SPLIT transitions (sentence boundary) and CONTINUE transitions (same sentence)
  3. Cover all dialog type combinations: quotes, smart quotes, parentheses, brackets, braces
  4. Tests should initially FAIL (documenting current bug) 
  5. Include zero-character separator cases and space-separated cases
  6. Clear distinction: Split/Continue is about sentence boundaries, D->D is about state transitions

* **Deliverables:**
  - New test function `test_dialog_to_dialog_transitions_all_states()` in `tests/dialog_detector_tests.rs`
  - Test cases for all 7 dialog states with both split and continue transitions
  - Zero-character separator tests for bracket-like dialog types
  - Space-separated transition tests for quote-like dialog types
  - Initial test run showing failures (proving bug exists)

* **References:**
  - SEAMS-Design.md lines 115-118 (zero-character separators)
  - Current double quote Dialog->Dialog patterns in dialog_detector.rs lines 494-495
  - Existing `dialog_double_to_dialog_hard/soft` patterns as reference implementation

## Test Case Examples:

### SPLIT Transitions (Dialog->Dialog + Sentence Boundary):
```rust
// Previous dialog ends with sentence punctuation + next starts with capital = SPLIT
("Text (This is one.)(This starts new.) More.", 3, "Round paren D->D split"),
("Text 'First sentence!' 'Second sentence.' More.", 3, "Single quote D->D split"),
("Text [Previous sentence.] [New sentence starts.] More.", 3, "Square bracket D->D split"),
```

### CONTINUE Transitions (Dialog->Dialog + Same Sentence):
```rust
// No sentence punctuation + next starts with lowercase = CONTINUE
("Text (first)(second) more.", 1, "Round paren D->D continue"),
("Text 'hello' 'world' more.", 1, "Single quote D->D continue"),
("Text [item][another] more.", 1, "Curly brace D->D continue"),
```

### Zero-Character Separators (Brackets):
```rust
// Immediate transitions without space
("Text (first)(Second) more.", 2, "Round paren D->D zero-char split"),
("Text [first][second] more.", 1, "Square bracket D->D zero-char continue"),
("Text {Done.}{Next task.} more.", 2, "Curly brace D->D zero-char split"),
```

### Cross-Dialog-Type Transitions:
```rust
// Mixed dialog types
("Text 'done.' \"Next task.\" more.", 2, "Single->Double quote D->D split"),
("Text (item) [another] more.", 1, "Paren->Bracket D->D continue"),
```

**Key Distinction:**
- **Dialog->Dialog Transition**: Moving from one dialog state to another dialog state
- **Split vs Continue**: Whether a sentence boundary is created at the transition point
- **Current Bug**: Only double quotes support D->D transitions; other dialog types don't

## Pre-commit checklist:
- [ ] All deliverables implemented
- [ ] Tests passing (`cargo test`)
- [ ] Claims validated (`cargo test -- --nocapture | grep -E "(concurrent|parallel|faster|optimized)"` + manual verification)
- [ ] Documentation updated if needed
- [ ] **ZERO WARNINGS**: `./scripts/validate_warning_free.sh` passes completely