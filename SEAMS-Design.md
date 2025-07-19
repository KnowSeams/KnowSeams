# SEAMS Design: Complete Pattern Recognition for Dual State Tracking

## Core Concept: What is a SEAM?

A **SEAM** is the **WHOLE PATTERN** that represents an **actual transition point** in text where sentence/dialog state changes can occur. The structure is:

```
<previous-kind><sep><next-kind>
```

Where:
- **previous-kind**: End signifier of preceding content (optional punctuation)
- **sep**: Separator (typically 1 character, varies in complexity, can be zero in rare circumstances)  
- **next-kind**: Beginning signifier of following content (optional punctuation)

## Text Processing: Consumption vs Recognition

### Normal Text Consumption
Regular text flows without pattern matching:
- `word <space> more` → **Not a SEAM** - just advance through text
- `she walked quickly` → **Not a SEAM** - continue consuming characters

### SEAM Recognition  
Patterns match only at **actual potential transition points**:
- `word! <space> more` → **SEAM** - potential boundary (interjection vs sentence end)
- `word. <space> Next` → **SEAM** - potential boundary (abbreviation vs sentence end)
- `"dialog" <space> Next` → **SEAM** - dialog exit transition point

**Efficiency**: Regex advances through normal text until it encounters a SEAM pattern, then determines the appropriate state transition. In Dialog states the active Regex advance to the dialog close ignoring all potential sentence seams and dialog open transitions.

## Dual State Tracking

We track **two independent state dimensions**:

### 1. Content Type States
- **Narrative (N)**: Regular prose text
- **Dialog (D)**: Text within paired punctuation (We do not recurse.)

### 2. Sentence Boundary Action at Seam  
- **Continue**: Same sentence continues
- **Split**: New sentence begins

### State Transition Matrix

| From | To | Sentence Action | Example SEAM |
|------|----|-----------------|--------------|
| N | N | Continue | `Dr. <space> Smith` |
| N | N | Split | `word. <space> Next` |
| N | D | Continue | `word <space> "dialog` |
| N | D | Split | `word. <space> "Dialog` |
| D | D | Continue | `"text" <space> "more"` |
| D | D | Split | `"text." <space> "More"` |
| D | N | Continue | `"word" <space> continued` |
| D | N | Split | `"word." <space> Next` |

## SEAM Components

### Punctuation Classification

#### Separation Signifiers (Potential Boundaries)
**All separation signifiers create potential SEAMS** that require pattern-based disambiguation:

- `!` `?` (potential sentence enders)
- `.` (potential sentence ender - **most complex due to abbreviations**)

#### Continuation Signifiers (Always Maintain Flow)  
- `,` `;` `:` (definitive connectors - always continue)
- `-` `—` (dashes)
- `...` `…` (ellipsis)

#### SEAM Pattern Efficiency Strategy

**Two Categories of Potential Boundaries:**

1. **NOT STOPPED** (not a SEAM - regex doesn't match):
   - `etc. and` → Pattern skips because lowercase follows
   - `vs. other` → Pattern skips because lowercase follows  
   - **No analysis needed** - abbreviations always followed by lowercase never create SEAM patterns

2. **STOPPED and DISAMBIGUATED** (SEAM pattern matches, then check abbreviation list):
   - `Dr. Hall` → Pattern matches, check abbreviation list → **continue** (is abbreviation)
   - `word. Next` → Pattern matches, check abbreviation list → **split** (not abbreviation)

**Efficiency Goal**: SEAM regex patterns skip most abbreviations automatically, only requiring abbreviation list checking when capitals follow periods.

#### Dialog Paired Signifiers

| Type | Open | Close | 
|------|------|-------|
| Double Quote | `"` | `"` |
| Single Quote | `'` | `'` |
| Smart Double | `“` | `”` |
| Smart Single | `‘` | `’` |
| Parentheses | `(` | `)` |
| Square Brackets | `[` | `]` |
| Curly Braces | `{` | `}` |

### Separator Patterns

#### Simple Separators
- **Single space**: ` ` (most common)
- **Double space**: `  ` (rare variation, same behavior as single space)
- **Tab**: `\t` (formatting/alignment)

#### Line Break Separators  
- **Unix line break**: `\n` (formatting to page)
- **Windows line break**: `\r\n` (formatting to page)

#### Paragraph Break Separators
- **Unix paragraph**: `\n\n` (strong boundary signal)
- **Windows paragraph**: `\r\n\r\n` (strong boundary signal)  
- **Mixed paragraph**: `\n` + spaces, etc. (various formatting patterns)

#### Zero-Character Separators
**For strongly signaled open vs close pairs**:
- **Brackets/Parentheses**: `)(`, `][`, `}{` (immediate D→D transition)
- **NOT standard quotes**: `""` or `''` transitions require whitespace separation

#### Paragraph Break Behavior
**Paragraph breaks provide strong sentence boundary signals** but can be **overridden by continuation signifiers**:

- **Default**: Paragraph breaks stop unpaired/mis-paired dialog from continuing indefinitely
- **Override**: Strong continuation signals (dash, colon) can bridge paragraph breaks
- **Use case**: Narrative description ending with `:` or `—` to introduce dialog across paragraph boundary

#### Open-Ended vs Specific Whitespace
**Decision needed**: Use specific patterns vs `[ \t\n\r]*` type open-ended matching for separators.

### Content Type Indicators

#### Character Set Definitions (Complete Coverage)

**Start Characters** `[A-Z"'""'([{0-9]`:
- Capital letters `[A-Z]` (sentence starts)
- Dialog openers `["'""'([{]` (dialog starts)  
- Numbers `[0-9]` (sentence starts)

**NOT(Start Characters)** `[^A-Z"'""'([{0-9]`:
- Lowercase letters `[a-z]` (strong continuation signal)
- Punctuation `[.!?,:;-]` (depends on context)
- Other characters (continuation signal)

#### Case Signal Priority
**NOT(Start_character) after separator => VERY STRONG continuation signal**
- `word." <space> more` → continue (lowercase 'm')
- `word <space> More` → depends on punctuation context
- `word. <space> more` → split despite lowercase (period overrides)

## SEAM Pattern Architecture

### Pattern Completeness Requirement

**At a specific postiion in the Seam every possible character must fall into exactly one regex pattern.** Each regex must determine both:
1. **Content Type Transition**: N→N, N→D, D→N, D→D
2. **Sentence Boundary Action**: Continue or Split

### Dialog State Recursion

When in Dialog state, we seek:
1. **Paired Close**: Matching close for current dialog type
2. **Nested Dialog**: New dialog opens before current closes
3. **Sentence Boundaries**: Within dialog content

## Complete SEAM Pattern Matrix

### Dialog Exit Patterns (D→N and D→D)

| Previous End | Sep | Next Begin | Content Transition | Sentence Action | Example SEAM |
|--------------|-----|------------|-------------------|-----------------|--------------|
| `{close}` | ` ` | `[A-Z]` | D→N | Split | `"word" <space> Next` |
| `{close}` | ` ` | `[a-z]` | D→N | Continue | `"word" <space> continued` |
| `{close}` | ` ` | `{open}` | D→D | Continue/Split* | `"word" <space> "more"` |
| `{close}{sep_punct}` | ` ` | `[A-Z]` | D→N | Split | `"word". <space> Next` |
| `{close}{sep_punct}` | ` ` | `[a-z]` | D→N | Split | `"word"! <space> next` |
| `{close}{sep_punct}` | ` ` | `{open}` | D→D | Split | `"word"! <space> "More"` |
| `{close}{cont_punct}` | ` ` | Any | D→N | Continue | `"word", <space> more` |

*Continue/Split for D→D depends on next content capitalization

### Narrative Patterns (N→N and N→D)

| Previous End | Sep | Next Begin | Content Transition | Sentence Action | Example SEAM |
|---------------|-----|------------|-------------------|-----------------|--------------|
| `word`        | ` ` | `[A-Z]` | N→N | Continue | `word <space> More` |
| `word`        | ` ` | `[a-z]` | N→N | Continue | `word <space> more` |
| `word`        | ` ` | `{open}` | N→D | Continue | `word <space> "dialog` |
| `{sep_punct}` | ` ` | `{start}` | N→N | Split | `word. <space> Next` |
| `{sep_punct}` | ` ` | `{not-start}` | N→N | Continue | `word! <space> next` |
| `{sep_punct}` | ` ` | `{open}` | N→D | Split | `word. <space> "Dialog` |
| `{cont_punct}`| ` ` | `{not-open}` | N→N | Continue | `word, <space> more` |
| `{cont_punct}`| ` ` | `{open}`| N→D | Continue | `word, <space> (more` |


### Key Pattern Rules

1. **Likely separation punctuation** (`!?`) **sentence split absent prev continuation char or not-start-char for next**
2. **Period** (`.`) **requires abbreviation disambiguation** before forcing split
3. **Continuation punctuation** (`,;:-`) **always maintains sentence flow**  
4. **NOT(Start_character) after separator** provides **VERY STRONG continuation signal**
5. **Dialog transitions** follow same punctuation rules as narrative punctuation can be before or after close char.
6. **Every character** must match exactly one pattern (**complete coverage**)

## Regex Implementation Strategy

### Named Pattern Composition for Maximum Efficiency

**Goal**: Pattern match directly determines complete state transition with minimal runtime analysis.

```rust
// Base character set definitions with complementary coverage
let start_chars = r"[A-Z\"\'\"\'(\[\{0-9]";          // Sentence/dialog starters
let not_start_chars = r"[^A-Z\"\'\"\'(\[\{0-9]";     // NOT(start) = continuation signal
let dialog_opens = r"[\"\'\"\'(\[\{]";               // Dialog opening chars
let definitive_sep = r"[!?]";                        // Always separating (context-dependent)
let contextual_sep = r"\.";                          // Period - needs abbreviation check
let continuation_punct = r"[,:;]";                   // Always continuing
let separator = r"[ \t]+";                           // Standard whitespace separator
let hard_separator = r"(?:\r\n\r\n|\n\n)";          // Hard document separator

// Named pattern components for composition
let external_definitive = format!("{close}{definitive_sep}");      // "!  or ?
let external_contextual = format!("{close}{contextual_sep}");      // ".
let external_continuation = format!("{close}{continuation_punct}"); // ",  ;  :
let no_external_punct = format!("{close}");                        // "

// Dialog state patterns - each pattern maps directly to specific state transition
let dialog_patterns = vec![
    // D→N Split: External definitive + start chars (punctuation + case both signal split)
    (format!("{external_definitive}({separator}){start_chars}"), DialogEnd),
    
    // D→N Continue: External definitive + NOT(start chars) (lowercase overrides punctuation)
    (format!("{external_definitive}({separator}){not_start_chars}"), DialogSoftEnd),
    
    // D→N Context: External contextual + start chars (period needs abbreviation check)
    (format!("{external_contextual}({separator}){start_chars}"), DialogContextualEnd),
    
    // D→N Context: External contextual + NOT(start chars) (period + lowercase)
    (format!("{external_contextual}({separator}){not_start_chars}"), DialogContextualSoftEnd),
    
    // D→N Continue: External continuation + any (comma/semicolon/colon always continue)
    (format!("{external_continuation}({separator})."), DialogSoftEnd),
    
    // D→N Split: No external + start chars (case signals split)
    (format!("{no_external_punct}({separator}){start_chars}"), DialogEnd),
    
    // D→N Continue: No external + NOT(start chars) (case signals continue)
    (format!("{no_external_punct}({separator}){not_start_chars}"), DialogSoftEnd),
    
    // D→D Transition: Any external + dialog opener (secondary case analysis needed)
    (format!("{close}[.!?,:;]*({separator}){dialog_opens}"), DialogToDialog),
    
    // Hard separator: Always creates sentence boundary regardless of state
    (format!("({hard_separator})"), HardSeparator),
];
```

### Pattern Coverage Verification

**Complementary Set Coverage**: `start_chars ∪ not_start_chars = ALL_CHARACTERS`

**Pattern Completeness**: Every possible SEAM must match exactly one pattern.

**Efficiency**: Each pattern match directly determines:
1. **Content Type Transition** (D→N, D→D, etc.)
2. **Sentence Boundary Action** (Split, Continue, or Context-check)

### Direct Pattern-to-Transition Mapping

**Test Matrix**: Each pattern directly determines complete state transition:

1. `"word" Next` → `no_external_punct + separator + start_chars` → **DialogEnd** (D→N Split)
2. `"word" next` → `no_external_punct + separator + not_start_chars` → **DialogSoftEnd** (D→N Continue)  
3. `"word"! Next` → `external_definitive + separator + start_chars` → **DialogEnd** (D→N Split)
4. `"word"! next` → `external_definitive + separator + not_start_chars` → **DialogSoftEnd** (D→N Continue)
5. `"word", more` → `external_continuation + separator + any` → **DialogSoftEnd** (D→N Continue)
6. `"word" "more"` → `close + separator + dialog_opens` → **DialogToDialog** (D→D)
7. `"word. Next"` → `external_contextual + separator + start_chars` → **DialogContextualEnd** (D→N Split if not abbreviation)
8. `"word. next"` → `external_contextual + separator + not_start_chars` → **DialogContextualSoftEnd** (D→N Continue if not abbreviation)

### Named Component Benefits

1. **Compositional Clarity**: `external_definitive + separator + not_start_chars` clearly expresses intent
2. **Complementary Coverage**: `start_chars` and `not_start_chars` guarantee complete character coverage
3. **Direct Efficiency**: Pattern match immediately yields specific `MatchType` with minimal analysis
4. **Maintainability**: Named components can be reused across different dialog states (quotes, parentheses, etc.)

**Runtime Efficiency**: `Pattern Recognition → Direct State Transition` (no intermediate decisions)

## Current Implementation Gap

### Missing Pattern Coverage

**Current**: Only handles external continuation punctuation `[,:;]*`
**Missing**: External definitive punctuation `[.!?]` with case-sensitive handling

### Implementation Requirements

1. **Complete Named Component Library**
   ```rust
   // Must define ALL components with complementary coverage
   let external_definitive = format!("{close}[!?]");     // Excludes period (contextual)
   let external_contextual = format!("{close}\\.");       // Period only (needs abbreviation check)
   let external_continuation = format!("{close}[,:;]");   // Always continue
   let no_external_punct = format!("{close}");           // Direct to case analysis
   ```

2. **Pattern Mutual Exclusivity**
   - Each character sequence matches exactly one pattern
   - Complementary character sets ensure 100% coverage
   - Priority ordering prevents pattern conflicts

3. **Direct State Transition Mapping**
   ```rust
   // Each pattern maps directly to specific MatchType
   external_definitive + separator + start_chars → DialogEnd
   external_definitive + separator + not_start_chars → DialogSoftEnd
   external_contextual + separator + start_chars → DialogContextualEnd
   // etc.
   ```

## Implementation Validation

### Target Test Cases

**Three-Sentence Cases** (Currently failing - missing external definitive patterns):
- `Text "word"! More text. New sentence.` → DialogEnd + DialogEnd = 3 sentences
- `Text 'word'? More text. New sentence.` → DialogEnd + DialogEnd = 3 sentences

**Interjection Cases** (Should work with new patterns):
- `Text "word"! next sentence.` → DialogSoftEnd = 2 sentences (lowercase overrides)
- `Text "word"? she continued.` → DialogSoftEnd = 2 sentences (lowercase overrides)

**Coverage Verification**: Every possible `{close}{external_punct?}{separator}{next_char}` combination must match exactly one pattern.

---

## CURRENT IMPLEMENTATION ANALYSIS

*This section documents the existing implementation in `src/sentence_detector/dialog_detector.rs` to distinguish what we have vs what we should implement.*

### Current Separator Patterns (Line 309-311)
```rust
let soft_separator = r"[ \t]+";           // Only spaces/tabs within same line  
let line_boundary = r"(?:\r?\n)";         // single newline for line-end patterns
let hard_separator = r"(?:\r\n\r\n|\n\n)"; // double newline (Windows or Unix)
```

**Current State**: Uses specific patterns, not open-ended. Includes tabs in `soft_separator`.

### Current Dialog State Types (Line 166-176)
```rust
pub enum DialogState {
    Narrative,
    DialogDoubleQuote,
    DialogSingleQuote, 
    DialogSmartDoubleOpen,
    DialogSmartSingleOpen,
    DialogParenthheticalRound,
    DialogParenthheticalSquare,
    DialogParenthheticalCurly,
    Unknown,
}
```

**Current State**: All major dialog types implemented. No zero-character separator patterns.

### Current Pattern Structure (Line 461-464)
```rust
// Example: Dialog Double Quote patterns
let dialog_hard_end = format!("{sentence_end_punct}{double_quote_close}({soft_separator})[{sentence_starts}]");
let dialog_soft_end_punctuated = format!("{sentence_end_punct}{double_quote_close}({soft_separator}){not_sentence_starts}");
let dialog_continuation = format!("{not_sentence_end_punct}{double_quote_close}({soft_separator}){dialog_open_chars}");
let dialog_soft_end_unpunctuated = format!("{not_sentence_end_punct}{double_quote_close}({soft_separator}){not_dialog_openers}");
let dialog_hard_end = format!("{sentence_end_punct}{double_quote_close}{optional_punctuation_after_dialog_close}({soft_separator})[{sentence_starts}]");
let dialog_soft_end_punctuated = format!("{sentence_end_punct}{double_quote_close}{optional_punctuation_after_dialog_close}({soft_separator}){not_sentence_starts}");
let dialog_continuation = format!("{not_sentence_end_punct}{double_quote_close}{optional_punctuation_after_dialog_close}({soft_separator}){dialog_open_chars}");
let dialog_soft_end_unpunctuated = format!("{not_sentence_end_punct}{double_quote_close}{optional_punctuation_after_dialog_close}({soft_separator}){not_dialog_openers}");

```

**Current State**: 
- ✅ **Implemented**: External continuation punctuation `[,:;]*` patterns
- ✅ **Implemented**: External definitive punctuation `[.!?]` patterns (Task 95)
- ✅ **Implemented**: Zero-character separator patterns for bracket pairs (Task 97)
- ✅ **Implemented**: Complete Dialog→Dialog transitions for all 7 dialog states (Task 97)
- ✅ **Implemented**: Pattern generation via `generate_dialog_patterns!` macro (Task 98)

### Recently Implemented (v0.1.1)

**✅ Gap 1: External Definitive Punctuation** (Task 95)
- `"word"! More` → Now correctly produces 3 sentences in repro examples ✅
- `"word"? Next` → Now correctly produces 3 sentences in repro examples ✅

**✅ Gap 2: Zero-Character Dialog Transitions** (Task 97)  
- `(first)(second)` → Now handles D→D transition correctly ✅
- `[one][two]` → Now handles D→D transition correctly ✅
- All 6 bracket-style dialog types support zero-character separators ✅

**✅ Gap 3: Pattern Architecture Improvement** (Task 98)
- Implemented: `generate_dialog_patterns!` macro for consistent pattern generation ✅
- All 7 dialog states use unified pattern structure ✅
- Eliminated ~250+ lines of pattern duplication ✅

### Current Implementation Status

**Complete Dialog→Dialog Coverage**: 20/20 D→D transition patterns working
- Split transitions: 7/7 dialog states ✅
- Continue transitions: 7/7 dialog states ✅  
- Zero-character separators: 6/6 bracket-style dialogs ✅

**Remaining Scope**: Cross-dialog-type transitions (e.g., `'text' "more"`) deferred to task 99 pending corpus validation.