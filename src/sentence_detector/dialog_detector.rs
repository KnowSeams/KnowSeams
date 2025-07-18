// WHY: Dialog-aware sentence detection with state machine for coalescing and sophisticated boundaries
// Moved from tests to official implementation with dual API support

use anyhow::Result;
use regex_automata::{meta::Regex, Input};
use std::collections::HashMap;
use tracing::{debug, info};

use super::{DetectedSentenceBorrowed, Span, AbbreviationChecker};

// Type-safe position wrappers to prevent byte/char and 0/1-based confusion

/// 0-based byte position in source text
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct BytePos(pub usize);

/// 0-based character position in source text
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct CharPos(pub usize);

/// 1-based line number for output spans
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct OneBasedLine(pub usize);

/// 1-based column number for output spans
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct OneBasedCol(pub usize);

// Conversion implementations

impl From<BytePos> for usize {
    fn from(pos: BytePos) -> Self {
        pos.0
    }
}

impl From<CharPos> for usize {
    fn from(pos: CharPos) -> Self {
        pos.0
    }
}

impl From<OneBasedLine> for usize {
    fn from(line: OneBasedLine) -> Self {
        line.0
    }
}

impl From<OneBasedCol> for usize {
    fn from(col: OneBasedCol) -> Self {
        col.0
    }
}

impl BytePos {
    pub fn new(pos: usize) -> Self {
        BytePos(pos)
    }
    
    pub fn advance(&self, offset: usize) -> Self {
        BytePos(self.0 + offset)
    }
}

impl CharPos {
    pub fn new(pos: usize) -> Self {
        CharPos(pos)
    }
}

impl OneBasedLine {
    pub fn new(line: usize) -> Option<Self> {
        if line > 0 {
            Some(OneBasedLine(line))
        } else {
            None
        }
    }
    
    pub fn first() -> Self {
        OneBasedLine(1)
    }
}

impl OneBasedCol {
    pub fn new(col: usize) -> Option<Self> {
        if col > 0 {
            Some(OneBasedCol(col))
        } else {
            None
        }
    }
    
    pub fn first() -> Self {
        OneBasedCol(1)
    }
}

// PHASE 1: Incremental Position Tracking
// WHY: Eliminate O(N²) behavior from repeated byte_to_char_pos and char_to_line_col scans
#[derive(Debug)]
pub struct PositionTracker {
    current_byte_pos: usize,
    current_char_pos: usize,
    current_line: usize,
    current_col: usize,
    text_bytes: Vec<u8>,
}

impl PositionTracker {
    pub fn new(text: &str) -> Self {
        Self {
            current_byte_pos: 0,
            current_char_pos: 0,
            current_line: 1,
            current_col: 1,
            text_bytes: text.as_bytes().to_vec(), // TODO: Should use &[u8] to avoid copy, but requires lifetime parameter
        }
    }
    
    /// Advance incrementally to target byte position, updating char/line/col counters
    pub fn advance_to_byte(&mut self, target_byte_pos: BytePos) -> Result<(CharPos, OneBasedLine, OneBasedCol), String> {
        if target_byte_pos.0 < self.current_byte_pos {
            return Err(format!("Cannot seek backwards: current {} > target {}", self.current_byte_pos, target_byte_pos.0));
        }
        
        if target_byte_pos.0 > self.text_bytes.len() {
            return Err(format!("Target byte position {} exceeds text length {}", target_byte_pos.0, self.text_bytes.len()));
        }
        
        // Advance incrementally from current position to target
        while self.current_byte_pos < target_byte_pos.0 {
            let byte = self.text_bytes[self.current_byte_pos];
            
            // Check if this byte starts a UTF-8 character
            if (byte & 0x80) == 0 || (byte & 0xC0) == 0xC0 {
                // This is either ASCII (0xxxxxxx) or start of multi-byte (11xxxxxx)
                self.current_char_pos += 1;
                
                if byte == b'\n' {
                    self.current_line += 1;
                    self.current_col = 1;
                } else {
                    self.current_col += 1;
                }
            }
            // Continuation bytes (10xxxxxx) don't increment char_pos or col
            
            self.current_byte_pos += 1;
        }
        
        Ok((
            CharPos::new(self.current_char_pos),
            OneBasedLine::new(self.current_line).unwrap(),
            OneBasedCol::new(self.current_col).unwrap(),
        ))
    }
    
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// Internal representation for dialog state machine
#[derive(Debug, Clone)]
pub struct DialogDetectedSentence {
    pub start_byte: BytePos,  // Added for O(1) borrowed API
    pub end_byte: BytePos,    // Added for O(1) borrowed API
    pub start_line: OneBasedLine,
    pub start_col: OneBasedCol,
    pub end_line: OneBasedLine,
    pub end_col: OneBasedCol,
}

#[derive(Debug, Clone)]
pub enum MatchType {
    NarrativeGestureBoundary,
    NarrativeToDialog,  // New: N→D transition that creates sentence boundary AND enters dialog
    DialogOpen,
    DialogEnd,
    DialogSoftEnd,
    HardSeparator,
}

/// Debug transition type for tracking sentence boundary decisions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionType {
    Continue,  // Pattern matched but no sentence boundary created
    Split,     // Pattern matched and sentence boundary created
}

/// Debug information captured during state transition analysis
/// WHY: Zero-cost abstraction - only collected when debug mode is explicitly enabled
#[derive(Debug, Clone)]
pub struct DebugTransitionInfo {
    pub sentence_index: usize,
    pub state_before: DialogState,
    pub state_after: DialogState,
    pub transition_type: TransitionType,
    pub matched_pattern: String,
    pub pattern_name: String,
    pub seam_text: String, // The actual SEAM that was analyzed
}

pub struct DialogStateMachine {
    state_patterns: HashMap<DialogState, Regex>,
    state_pattern_mappings: HashMap<DialogState, Vec<(MatchType, DialogState)>>,
    abbreviation_checker: AbbreviationChecker,
}

impl DialogStateMachine {
    
    /// Helper to get human-readable pattern name from pattern ID and state
    /// WHY: Provides meaningful debug output instead of just pattern IDs
    fn get_pattern_name(&self, state: &DialogState, pattern_id: usize) -> String {
        match state {
            DialogState::Narrative => match pattern_id {
                0..=6 => format!("NarrativeToDialog[{pattern_id}]"),
                7..=13 => format!("DialogOpen[{}]", pattern_id - 7),
                14..=20 => format!("IndependentDialog[{}]", pattern_id - 14),
                21 => "NarrativeLineBoundary".to_string(),
                22 => "NarrativeSentenceBoundary".to_string(),
                23 => "NarrativeHardBoundary".to_string(),
                24 => "HardSepDialogStart".to_string(),
                25 => "HardSepNarrativeStart".to_string(),
                26 => "HardSepEOF".to_string(),
                _ => format!("Unknown[{pattern_id}]"),
            },
            DialogState::DialogDoubleQuote => match pattern_id {
                0 => "HardSepDialogStart".to_string(),
                1 => "HardSepNarrativeStart".to_string(),
                2 => "HardSepEOF".to_string(),
                3 => "DialogToDialogHard".to_string(),
                4 => "DialogToDialogSoft".to_string(),
                5 => "DialogHardEnd".to_string(),
                6 => "DialogSoftEnd".to_string(),
                7 => "DialogContinuationBefore".to_string(),
                8 => "DialogContinuationAfter".to_string(),
                9 => "DialogUnpunctuatedHardEnd".to_string(),
                10 => "DialogUnpunctuatedSoftEnd".to_string(),
                _ => format!("Unknown[{pattern_id}]"),
            },
            DialogState::DialogSingleQuote |
            DialogState::DialogSmartDoubleOpen |
            DialogState::DialogSmartSingleOpen |
            DialogState::DialogParenthheticalSquare |
            DialogState::DialogParenthheticalCurly => match pattern_id {
                0 => "HardSepDialogStart".to_string(),
                1 => "HardSepNarrativeStart".to_string(),
                2 => "HardSepEOF".to_string(),
                3 => "DialogHardEnd".to_string(),
                4 => "DialogSoftEnd".to_string(),
                5 => "DialogContinuationBefore".to_string(),
                6 => "DialogContinuationAfter".to_string(),
                7 => "DialogUnpunctuatedHardEnd".to_string(),
                8 => "DialogUnpunctuatedSoftEnd".to_string(),
                _ => format!("Unknown[{pattern_id}]"),
            },
            DialogState::DialogParenthheticalRound => match pattern_id {
                0 => "HardSepDialogStart".to_string(),
                1 => "HardSepNarrativeStart".to_string(),
                2 => "HardSepEOF".to_string(),
                3 => "DialogHardEnd".to_string(),
                4 => "DialogSoftEnd".to_string(),
                5 => "DialogContinuationBefore".to_string(),
                6 => "DialogNonSentencePunct".to_string(),
                7 => "DialogUnpunctuatedHardEnd".to_string(),
                8 => "DialogUnpunctuatedSoftEnd".to_string(),
                _ => format!("Unknown[{pattern_id}]"),
            },
            DialogState::Unknown => format!("UnknownState[{pattern_id}]"),
        }
    }
    
    /// Unified hard separator analysis - determines both rejection and target state
    /// Returns (should_reject, target_state) based on backward and forward context
    fn analyze_hard_separator_context(&self, text: &str, separator_start_byte: usize, matched_text: &str) -> (bool, DialogState) {
        // Backward scan: check if separator should be rejected due to internal punctuation
        let should_reject = self.should_reject_hard_separator_internal(text.as_bytes(), separator_start_byte);
        
        // Forward scan: determine target state from consumed character
        let target_state = if let Some(last_char) = matched_text.chars().last() {
            match last_char {
                // Double quote ASCII
                '"' => DialogState::DialogDoubleQuote,
                // Single quote ASCII  
                '\'' => DialogState::DialogSingleQuote,
                // Round parenthesis
                '(' => DialogState::DialogParenthheticalRound,
                // Square bracket
                '[' => DialogState::DialogParenthheticalSquare,
                // Curly brace
                '{' => DialogState::DialogParenthheticalCurly,
                // Smart double open quote
                '\u{201C}' => DialogState::DialogSmartDoubleOpen,
                // Smart single open quote
                '\u{2018}' => DialogState::DialogSmartSingleOpen,
                // Everything else -> Narrative
                _ => DialogState::Narrative,
            }
        } else {
            // No consumed character (hard_sep_eof case)
            DialogState::Narrative
        };
        
        (should_reject, target_state)
    }

    /// Check if hard separator should be rejected due to preceding internal punctuation
    /// O(1) operation - scans backward to find meaningful punctuation (typically 1-5 bytes)
    fn should_reject_hard_separator_internal(&self, text_bytes: &[u8], separator_start_byte: usize) -> bool {
        if separator_start_byte == 0 {
            return false;
        }
        
        // WHY: Walk backward to find meaningful punctuation, skipping whitespace and closing delimiters
        // Expected to exit early (1-5 iterations) in typical cases
        let mut pos = separator_start_byte;
        let scan_limit = separator_start_byte.saturating_sub(20); // Safety guard
        
        while pos > scan_limit {
            pos -= 1;
            let byte = text_bytes[pos];
            
            // Skip whitespace bytes - continue scanning
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                continue;
            }
            
            // Found non-whitespace - check if it's a UTF-8 continuation byte
            if (byte & 0xC0) == 0x80 {
                // This is a continuation byte (10xxxxxx), keep going to find start byte
                continue;
            }
            
            // This is either ASCII (0xxxxxxx) or multi-byte start (11xxxxxx)
            if byte < 0x80 {
                // ASCII character - check what type it is
                match byte {
                    // Terminal punctuation - allow hard separator (don't reject)
                    b'.' | b'?' | b'!' => return false,
                    
                    // Closing delimiters - accept hard separator (they are terminal)
                    b'"' | b'\'' | b')' | b']' | b'}' => return false,
                    
                    // Internal punctuation - reject hard separator (coalesce)
                    b',' | b';' | b':' | b'-' | b'/' | 
                    b'(' | b'[' | b'{' => return true,
                    
                    // Other characters (letters, digits, etc.) - treat as continuation, reject separator
                    _ => return false,
                }
            } else {
                // Multi-byte UTF-8 character - decode from start byte
                let remaining_bytes = &text_bytes[pos..separator_start_byte.min(pos + 4)];
                if let Ok(utf8_str) = std::str::from_utf8(remaining_bytes) {
                    if let Some(ch) = utf8_str.chars().next() {
                        match ch {
                            // Terminal punctuation - allow hard separator
                            '.' | '?' | '!' => return false,
                            
                            // Smart closing quotes - accept hard separator (they are terminal)
                            '\u{201D}' | '\u{2019}' => return false,
                            
                            // Internal punctuation (em/en dash, smart opening quotes) - reject separator
                            '\u{2014}' | '\u{2013}' | '\u{201C}' | '\u{2018}' => return true,
                            
                            // Ellipsis - treat as continuation, reject separator
                            '\u{2026}' => return true,
                            
                            // Other Unicode - treat as continuation, reject separator
                            _ => return false,
                        }
                    }
                }
                // If decode fails, conservatively reject (coalesce)
                return true;
            }
        }
        
        // No meaningful punctuation found in scan window - treat as continuation, reject separator
        true
    }

    /// Helper function to negate a character class
    fn negate_char_class(char_class: &str) -> String {
        format!("[^{}]", &char_class[1..char_class.len()-1])
    }

    pub fn new() -> Result<Self> {
        let mut state_patterns = HashMap::new();
        let mut state_pattern_mappings = HashMap::new();
        
        // Compositional pattern components
        let sentence_end_punct = r"[.!?]";
        let soft_separator = r"[ \t]+";  // Only spaces/tabs within same line  
        let line_boundary = r"(?:\r?\n)";  // single newline for line-end patterns
        let hard_separator = r"(?:\r\n\r\n|\n\n)";   // double newline (Windows or Unix)
        
        // SOLUTION: Split sentence start chars to prevent overlaps
        let non_dialog_sentence_start_chars = r"[A-Z]";  // Only capital letters for narrative boundaries
        let dialog_open_chars = r"[\x22\x27\u{201C}\u{2018}\(\[\{]";  // All dialog opening characters
        
        let dialog_prefix_whitespace = r"[ \t\n]";  // Space, tab, or newline
        
        // NEW APPROACH: Explicit patterns for each dialog character type
        // Distinguish between sentence-ending and non-sentence-ending punctuation
        let sentence_ending_punct = r"[.!?]";  // These create sentence boundaries
        let non_sentence_ending_punct = r"[,:;]";  // These do NOT create sentence boundaries
        
        // Individual dialog characters
        let double_quote_char = r"\x22";         // "
        let single_quote_char = r"\x27";         // '
        let smart_double_open = r"\u{201C}";     // “
        let smart_single_open = r"\u{2018}";     // ‘
        let round_paren_open = r"\(";            // (
        let square_bracket_open = r"\[";         // [
        let curly_brace_open = r"\{";            // {
        
        // N→N narrative sentence boundaries
        let narrative_sentence_boundary = format!("{sentence_ending_punct}({soft_separator}){non_dialog_sentence_start_chars}");
        let narrative_line_boundary = format!("{sentence_ending_punct}{line_boundary}{non_dialog_sentence_start_chars}");
        let narrative_hard_boundary = format!(r"{sentence_ending_punct}\s*{hard_separator}\s*{non_dialog_sentence_start_chars}");
        
        // N→D transitions WITH sentence boundary (sentence punct + space + specific dialog char)
        let narrative_to_double_quote_boundary = format!("{sentence_ending_punct}({soft_separator}){double_quote_char}");
        let narrative_to_single_quote_boundary = format!("{sentence_ending_punct}({soft_separator}){single_quote_char}");
        let narrative_to_smart_double_boundary = format!("{sentence_ending_punct}({soft_separator}){smart_double_open}");
        let narrative_to_smart_single_boundary = format!("{sentence_ending_punct}({soft_separator}){smart_single_open}");
        let narrative_to_round_paren_boundary = format!("{sentence_ending_punct}({soft_separator}){round_paren_open}");
        let narrative_to_square_bracket_boundary = format!("{sentence_ending_punct}({soft_separator}){square_bracket_open}");
        let narrative_to_curly_brace_boundary = format!("{sentence_ending_punct}({soft_separator}){curly_brace_open}");
        
        // N→D transitions WITHOUT sentence boundary (non-sentence punct + space + specific dialog char)
        let narrative_to_double_quote_no_boundary = format!("{non_sentence_ending_punct}({soft_separator}){double_quote_char}");
        let narrative_to_single_quote_no_boundary = format!("{non_sentence_ending_punct}({soft_separator}){single_quote_char}");
        let narrative_to_smart_double_no_boundary = format!("{non_sentence_ending_punct}({soft_separator}){smart_double_open}");
        let narrative_to_smart_single_no_boundary = format!("{non_sentence_ending_punct}({soft_separator}){smart_single_open}");
        let narrative_to_round_paren_no_boundary = format!("{non_sentence_ending_punct}({soft_separator}){round_paren_open}");
        let narrative_to_square_bracket_no_boundary = format!("{non_sentence_ending_punct}({soft_separator}){square_bracket_open}");
        let narrative_to_curly_brace_no_boundary = format!("{non_sentence_ending_punct}({soft_separator}){curly_brace_open}");
        
        // Independent dialog starts (whitespace + dialog char, not after punctuation)
        let double_quote_independent = format!("{dialog_prefix_whitespace}{double_quote_char}");
        let single_quote_independent = format!("{dialog_prefix_whitespace}{single_quote_char}");
        let smart_double_independent = format!("{dialog_prefix_whitespace}{smart_double_open}");
        let smart_single_independent = format!("{dialog_prefix_whitespace}{smart_single_open}");
        let round_paren_independent = format!("{dialog_prefix_whitespace}{round_paren_open}");
        let square_bracket_independent = format!("{dialog_prefix_whitespace}{square_bracket_open}");
        let curly_brace_independent = format!("{dialog_prefix_whitespace}{curly_brace_open}");
        
        // Hard separator patterns that consume next character to avoid off-by-one error
        let not_dialog_open_chars = Self::negate_char_class(dialog_open_chars);
        let hard_sep_dialog_start = format!("{hard_separator}[{dialog_open_chars}]");
        let hard_sep_narrative_start = format!("{hard_separator}{not_dialog_open_chars}");
        let hard_sep_eof = hard_separator.to_string();
        
        
        // Dialog closing characters
        let double_quote_close = r"\x22";      // "
        let single_quote_close = r"\x27";      // '
        let smart_double_close = r"\u{201D}";  // "
        let smart_single_close = r"\u{2019}";  // '
        let round_paren_close = r"\)";         // )
        let square_bracket_close = r"\]";      // ]
        let curly_brace_close = r"\}";         // }
        
        
        // Dialog ending patterns: HARD_END (sentence boundary) vs SOFT_END (continue sentence)
        // Create unified character class by merging the character sets (not using | operator)
        let unified_sentence_start_chars = "[A-Z\\x22\\x27\\u{201C}\\u{2018}\\(\\[\\{]".to_string();
        let not_all_sentence_start_chars = "[^A-Z\\x22\\x27\\u{201C}\\u{2018}\\(\\[\\{]".to_string();
        
        // DialogDoubleQuote state: ONLY " can exit, not any other dialog delimiter
        let dialog_hard_double_end = format!("{sentence_end_punct}{double_quote_close}({soft_separator}){non_dialog_sentence_start_chars}");  // ." The
        let dialog_soft_double_end = format!("{sentence_end_punct}{double_quote_close}({soft_separator}){not_all_sentence_start_chars}");  // ." the
        
        // Dialog->Dialog transitions with 2-character patterns: <quote><space><open-delimiter><sentence-starter-or-not>
        let not_non_dialog_sentence_start_chars = Self::negate_char_class(non_dialog_sentence_start_chars);
        let dialog_double_to_dialog_hard = format!("{double_quote_close}({soft_separator}){dialog_open_chars}{non_dialog_sentence_start_chars}");  // " (The
        let dialog_double_to_dialog_soft = format!("{double_quote_close}({soft_separator}){dialog_open_chars}{not_non_dialog_sentence_start_chars}");  // " (note
        
        // EXTERNAL DEFINITIVE PUNCTUATION PATTERNS: Handle definitive punctuation after close
        // WHY: Dialog close chars never appear in abbreviations, so external periods need no abbreviation check
        // Pattern: close + definitive_punct + space + capital → Split (external definitive + case signals split)
        let dialog_double_external_hard_end = format!("{double_quote_close}[.!?]({soft_separator}){non_dialog_sentence_start_chars}");  // "! The, "? The, ". The
        // Pattern: close + definitive_punct + space + lowercase → Continue (external definitive but lowercase overrides)
        let dialog_double_external_soft_end = format!("{double_quote_close}[.!?]({soft_separator}){not_all_sentence_start_chars}");  // "! the, "? the, ". the
        
        // CONTINUATION PUNCTUATION PATTERNS: Handle continuation punctuation before and after close
        // Pattern: continuation_punct + close + space + any → Continue (before close)
        let dialog_double_continuation_before_end = format!("{non_sentence_ending_punct}{double_quote_close}({soft_separator}){unified_sentence_start_chars}");  // ," The
        // Pattern: close + continuation_punct + space + any → Continue (after close)  
        let dialog_double_continuation_after_end = format!("{double_quote_close}{non_sentence_ending_punct}({soft_separator}){unified_sentence_start_chars}");  // ", The
        
        // UNPUNCTUATED PATTERNS: No continuation punctuation → Split
        let dialog_double_unpunctuated_split_end = format!("{double_quote_close}({soft_separator}){non_dialog_sentence_start_chars}");  // " The
        let dialog_double_unpunctuated_continue_end = format!("{double_quote_close}({soft_separator}){not_all_sentence_start_chars}");  // " the
        
        // NARRATIVE STATE - Mutually exclusive patterns
        let narrative_patterns = vec![
            // N→D with sentence boundary (highest priority - create sentence break)
            narrative_to_double_quote_boundary.as_str(),       // PatternID 0
            narrative_to_single_quote_boundary.as_str(),       // PatternID 1
            narrative_to_smart_double_boundary.as_str(),       // PatternID 2
            narrative_to_smart_single_boundary.as_str(),       // PatternID 3
            narrative_to_round_paren_boundary.as_str(),        // PatternID 4
            narrative_to_square_bracket_boundary.as_str(),     // PatternID 5
            narrative_to_curly_brace_boundary.as_str(),        // PatternID 6
            
            // N→D without sentence boundary (continue current sentence)
            narrative_to_double_quote_no_boundary.as_str(),    // PatternID 7
            narrative_to_single_quote_no_boundary.as_str(),    // PatternID 8
            narrative_to_smart_double_no_boundary.as_str(),    // PatternID 9
            narrative_to_smart_single_no_boundary.as_str(),    // PatternID 10
            narrative_to_round_paren_no_boundary.as_str(),     // PatternID 11
            narrative_to_square_bracket_no_boundary.as_str(),  // PatternID 12
            narrative_to_curly_brace_no_boundary.as_str(),     // PatternID 13
            
            // Independent dialog starts
            double_quote_independent.as_str(),                 // PatternID 14
            single_quote_independent.as_str(),                 // PatternID 15
            smart_double_independent.as_str(),                 // PatternID 16
            smart_single_independent.as_str(),                 // PatternID 17
            round_paren_independent.as_str(),                  // PatternID 18
            square_bracket_independent.as_str(),               // PatternID 19
            curly_brace_independent.as_str(),                  // PatternID 20
            
            // N→N narrative boundaries
            narrative_line_boundary.as_str(),                  // PatternID 21
            narrative_sentence_boundary.as_str(),              // PatternID 22
            narrative_hard_boundary.as_str(),                  // PatternID 23
            
            // Hard separator patterns (consume next character to avoid off-by-one)
            hard_sep_dialog_start.as_str(),                   // PatternID 24
            hard_sep_narrative_start.as_str(),                // PatternID 25  
            hard_sep_eof.as_str(),                            // PatternID 26
        ];
        let narrative_mappings = vec![
            // N→D with sentence boundary (create sentence break + enter dialog)
            (MatchType::NarrativeToDialog, DialogState::DialogDoubleQuote),        // PatternID 0
            (MatchType::NarrativeToDialog, DialogState::DialogSingleQuote),        // PatternID 1
            (MatchType::NarrativeToDialog, DialogState::DialogSmartDoubleOpen),    // PatternID 2
            (MatchType::NarrativeToDialog, DialogState::DialogSmartSingleOpen),    // PatternID 3
            (MatchType::NarrativeToDialog, DialogState::DialogParenthheticalRound), // PatternID 4
            (MatchType::NarrativeToDialog, DialogState::DialogParenthheticalSquare), // PatternID 5
            (MatchType::NarrativeToDialog, DialogState::DialogParenthheticalCurly), // PatternID 6
            
            // N→D without sentence boundary (continue sentence + enter dialog)
            (MatchType::DialogOpen, DialogState::DialogDoubleQuote),              // PatternID 7
            (MatchType::DialogOpen, DialogState::DialogSingleQuote),              // PatternID 8
            (MatchType::DialogOpen, DialogState::DialogSmartDoubleOpen),          // PatternID 9
            (MatchType::DialogOpen, DialogState::DialogSmartSingleOpen),          // PatternID 10
            (MatchType::DialogOpen, DialogState::DialogParenthheticalRound),      // PatternID 11
            (MatchType::DialogOpen, DialogState::DialogParenthheticalSquare),     // PatternID 12
            (MatchType::DialogOpen, DialogState::DialogParenthheticalCurly),      // PatternID 13
            
            // Independent dialog starts (enter dialog)
            (MatchType::DialogOpen, DialogState::DialogDoubleQuote),              // PatternID 14
            (MatchType::DialogOpen, DialogState::DialogSingleQuote),              // PatternID 15
            (MatchType::DialogOpen, DialogState::DialogSmartDoubleOpen),          // PatternID 16
            (MatchType::DialogOpen, DialogState::DialogSmartSingleOpen),          // PatternID 17
            (MatchType::DialogOpen, DialogState::DialogParenthheticalRound),      // PatternID 18
            (MatchType::DialogOpen, DialogState::DialogParenthheticalSquare),     // PatternID 19
            (MatchType::DialogOpen, DialogState::DialogParenthheticalCurly),      // PatternID 20
            
            // N→N narrative boundaries (stay in narrative)
            (MatchType::NarrativeGestureBoundary, DialogState::Narrative),        // PatternID 21
            (MatchType::NarrativeGestureBoundary, DialogState::Narrative),        // PatternID 22
            (MatchType::NarrativeGestureBoundary, DialogState::Narrative),        // PatternID 23
            
            // Hard separator patterns - target state determined by unified analysis
            (MatchType::HardSeparator, DialogState::Unknown),                     // PatternID 24 - hard_sep_dialog_start -> analyze to determine specific dialog state
            (MatchType::HardSeparator, DialogState::Narrative),                   // PatternID 25 - hard_sep_narrative_start  
            (MatchType::HardSeparator, DialogState::Narrative),                   // PatternID 26 - hard_sep_eof
        ];
        state_patterns.insert(DialogState::Narrative, Regex::new_many(&narrative_patterns)?);
        state_pattern_mappings.insert(DialogState::Narrative, narrative_mappings);
        
        // DIALOG DOUBLE QUOTE STATE - Multi-pattern with hard separator FIRST (highest priority)
        let dialog_double_patterns = vec![
            hard_sep_dialog_start.as_str(),               // PatternID 0 = hard sep + dialog opener
            hard_sep_narrative_start.as_str(),            // PatternID 1 = hard sep + narrative char
            hard_sep_eof.as_str(),                        // PatternID 2 = hard sep + EOF
            dialog_double_to_dialog_hard.as_str(),        // PatternID 3 = Dialog->Dialog WITH sentence boundary (e.g., " (The)
            dialog_double_to_dialog_soft.as_str(),        // PatternID 4 = Dialog->Dialog WITHOUT sentence boundary (e.g., " (note)
            dialog_hard_double_end.as_str(),              // PatternID 5 = punctuated sentence boundary
            dialog_soft_double_end.as_str(),              // PatternID 6 = punctuated continue sentence
            dialog_double_external_hard_end.as_str(),     // PatternID 7 = external definitive punct + capital → Split
            dialog_double_external_soft_end.as_str(),     // PatternID 8 = external definitive punct + lowercase → Continue
            dialog_double_continuation_before_end.as_str(), // PatternID 9 = continuation punct before close → Continue
            dialog_double_continuation_after_end.as_str(), // PatternID 10 = continuation punct after close → Continue
            dialog_double_unpunctuated_split_end.as_str(), // PatternID 11 = unpunctuated sentence boundary  
            dialog_double_unpunctuated_continue_end.as_str(), // PatternID 12 = unpunctuated continue sentence
        ];
        let dialog_double_mappings = vec![
            (MatchType::HardSeparator, DialogState::Unknown),        // PatternID 0 - hard sep + dialog opener
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 1 - hard sep + narrative char
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 2 - hard sep + EOF
            (MatchType::DialogEnd, DialogState::Unknown),            // PatternID 3 - Dialog->Dialog hard transition (determine target state from consumed char)
            (MatchType::DialogOpen, DialogState::Unknown),           // PatternID 4 - Dialog->Dialog soft transition (determine target state from consumed char)
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 5 - punctuated hard dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 6 - punctuated soft dialog end
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 7 - external definitive punct + capital → Split
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 8 - external definitive punct + lowercase → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 9 - continuation punct before close → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 10 - continuation punct after close → Continue
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 11 - unpunctuated sentence boundary  
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 12 - unpunctuated continue sentence
        ];
        state_patterns.insert(DialogState::DialogDoubleQuote, Regex::new_many(&dialog_double_patterns)?);
        state_pattern_mappings.insert(DialogState::DialogDoubleQuote, dialog_double_mappings);
        
        // DIALOG SINGLE QUOTE STATE - Similar structure
        let dialog_hard_single_end = format!("{sentence_end_punct}{single_quote_close}({soft_separator}){unified_sentence_start_chars}");  // .' The or .' (
        let dialog_soft_single_end = format!("{sentence_end_punct}{single_quote_close}({soft_separator}){not_all_sentence_start_chars}");  // .' the
        
        // EXTERNAL DEFINITIVE PUNCTUATION PATTERNS for single quotes: Handle definitive punctuation after close
        // WHY: Dialog close chars never appear in abbreviations, so external periods need no abbreviation check
        // Pattern: close + definitive_punct + space + capital → Split (external definitive + case signals split)
        let dialog_single_external_hard_end = format!("{single_quote_close}[.!?]({soft_separator}){non_dialog_sentence_start_chars}");  // '! The, '? The, '. The
        // Pattern: close + definitive_punct + space + lowercase → Continue (external definitive but lowercase overrides)
        let dialog_single_external_soft_end = format!("{single_quote_close}[.!?]({soft_separator}){not_all_sentence_start_chars}");  // '! the, '? the, '. the
        
        // CONTINUATION PUNCTUATION PATTERNS for single quotes
        let dialog_single_continuation_before_end = format!("{non_sentence_ending_punct}{single_quote_close}({soft_separator}){unified_sentence_start_chars}");  // ,' The
        let dialog_single_continuation_after_end = format!("{single_quote_close}{non_sentence_ending_punct}({soft_separator}){unified_sentence_start_chars}");  // ', The
        
        // UNPUNCTUATED PATTERNS for single quotes
        let dialog_single_unpunctuated_split_end = format!("{single_quote_close}({soft_separator}){unified_sentence_start_chars}");  // ' The or ' (
        let dialog_single_unpunctuated_continue_end = format!("{single_quote_close}({soft_separator}){not_all_sentence_start_chars}");
        
        let dialog_single_patterns = vec![
            hard_sep_dialog_start.as_str(),               // PatternID 0 = hard sep + dialog opener
            hard_sep_narrative_start.as_str(),            // PatternID 1 = hard sep + narrative char
            hard_sep_eof.as_str(),                        // PatternID 2 = hard sep + EOF
            dialog_hard_single_end.as_str(),              // PatternID 3 = punctuated sentence boundary
            dialog_soft_single_end.as_str(),              // PatternID 4 = punctuated continue sentence
            dialog_single_external_hard_end.as_str(),     // PatternID 5 = external definitive punct + capital → Split
            dialog_single_external_soft_end.as_str(),     // PatternID 6 = external definitive punct + lowercase → Continue
            dialog_single_continuation_before_end.as_str(), // PatternID 7 = continuation punct before close → Continue
            dialog_single_continuation_after_end.as_str(), // PatternID 8 = continuation punct after close → Continue
            dialog_single_unpunctuated_split_end.as_str(), // PatternID 9 = unpunctuated sentence boundary
            dialog_single_unpunctuated_continue_end.as_str(), // PatternID 10 = unpunctuated continue sentence
        ];
        let dialog_single_mappings = vec![
            (MatchType::HardSeparator, DialogState::Unknown),        // PatternID 0 - hard sep + dialog opener
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 1 - hard sep + narrative char
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 2 - hard sep + EOF
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 3 - punctuated hard dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 4 - punctuated soft dialog end
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 5 - external definitive punct + capital → Split
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 6 - external definitive punct + lowercase → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 7 - continuation punct before close → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 8 - continuation punct after close → Continue
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 9 - unpunctuated sentence boundary
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 10 - unpunctuated continue sentence
        ];
        state_patterns.insert(DialogState::DialogSingleQuote, Regex::new_many(&dialog_single_patterns)?);
        state_pattern_mappings.insert(DialogState::DialogSingleQuote, dialog_single_mappings);
        
        // DIALOG SMART DOUBLE QUOTE STATE  
        let dialog_hard_smart_double_end = format!("{sentence_end_punct}{smart_double_close}({soft_separator}){unified_sentence_start_chars}");  // ." The or ." (
        let dialog_soft_smart_double_end = format!("{sentence_end_punct}{smart_double_close}({soft_separator}){not_all_sentence_start_chars}");
        
        // CONTINUATION PUNCTUATION PATTERNS for smart double quotes
        let dialog_smart_double_continuation_before_end = format!("{non_sentence_ending_punct}{smart_double_close}({soft_separator}){unified_sentence_start_chars}");  // ," The
        let dialog_smart_double_continuation_after_end = format!("{smart_double_close}{non_sentence_ending_punct}({soft_separator}){unified_sentence_start_chars}");  // ", The
        
        // UNPUNCTUATED PATTERNS for smart double quotes
        let dialog_smart_double_unpunctuated_split_end = format!("{smart_double_close}({soft_separator}){unified_sentence_start_chars}");  // " The
        let dialog_smart_double_unpunctuated_continue_end = format!("{smart_double_close}({soft_separator}){not_all_sentence_start_chars}");
        
        let dialog_smart_double_patterns = vec![
            hard_sep_dialog_start.as_str(),               // PatternID 0 = hard sep + dialog opener
            hard_sep_narrative_start.as_str(),            // PatternID 1 = hard sep + narrative char
            hard_sep_eof.as_str(),                        // PatternID 2 = hard sep + EOF
            dialog_hard_smart_double_end.as_str(),        // PatternID 3 = punctuated sentence boundary
            dialog_soft_smart_double_end.as_str(),        // PatternID 4 = punctuated continue sentence
            dialog_smart_double_continuation_before_end.as_str(), // PatternID 5 = continuation punct before close → Continue
            dialog_smart_double_continuation_after_end.as_str(), // PatternID 6 = continuation punct after close → Continue
            dialog_smart_double_unpunctuated_split_end.as_str(), // PatternID 7 = unpunctuated sentence boundary
            dialog_smart_double_unpunctuated_continue_end.as_str(), // PatternID 8 = unpunctuated continue sentence
        ];
        let dialog_smart_double_mappings = vec![
            (MatchType::HardSeparator, DialogState::Unknown),        // PatternID 0 - hard sep + dialog opener
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 1 - hard sep + narrative char
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 2 - hard sep + EOF
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 3 - punctuated hard dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 4 - punctuated soft dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 5 - continuation punct before close → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 6 - continuation punct after close → Continue
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 7 - unpunctuated sentence boundary
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 8 - unpunctuated continue sentence
        ];
        state_patterns.insert(DialogState::DialogSmartDoubleOpen, Regex::new_many(&dialog_smart_double_patterns)?);
        state_pattern_mappings.insert(DialogState::DialogSmartDoubleOpen, dialog_smart_double_mappings);
        
        // DIALOG SMART SINGLE QUOTE STATE
        let dialog_hard_smart_single_end = format!("{sentence_end_punct}{smart_single_close}({soft_separator}){unified_sentence_start_chars}");  // .' The or .' (
        let dialog_soft_smart_single_end = format!("{sentence_end_punct}{smart_single_close}({soft_separator}){not_all_sentence_start_chars}");
        
        // CONTINUATION PUNCTUATION PATTERNS for smart single quotes
        let dialog_smart_single_continuation_before_end = format!("{non_sentence_ending_punct}{smart_single_close}({soft_separator}){unified_sentence_start_chars}");  // ,' The
        let dialog_smart_single_continuation_after_end = format!("{smart_single_close}{non_sentence_ending_punct}({soft_separator}){unified_sentence_start_chars}");  // ', The
        
        // UNPUNCTUATED PATTERNS for smart single quotes
        let dialog_smart_single_unpunctuated_split_end = format!("{smart_single_close}({soft_separator}){unified_sentence_start_chars}");  // ' The
        let dialog_smart_single_unpunctuated_continue_end = format!("{smart_single_close}({soft_separator}){not_all_sentence_start_chars}");
        
        let dialog_smart_single_patterns = vec![
            hard_sep_dialog_start.as_str(),               // PatternID 0 = hard sep + dialog opener
            hard_sep_narrative_start.as_str(),            // PatternID 1 = hard sep + narrative char
            hard_sep_eof.as_str(),                        // PatternID 2 = hard sep + EOF
            dialog_hard_smart_single_end.as_str(),        // PatternID 3 = punctuated sentence boundary
            dialog_soft_smart_single_end.as_str(),        // PatternID 4 = punctuated continue sentence
            dialog_smart_single_continuation_before_end.as_str(), // PatternID 5 = continuation punct before close → Continue
            dialog_smart_single_continuation_after_end.as_str(), // PatternID 6 = continuation punct after close → Continue
            dialog_smart_single_unpunctuated_split_end.as_str(), // PatternID 7 = unpunctuated sentence boundary
            dialog_smart_single_unpunctuated_continue_end.as_str(), // PatternID 8 = unpunctuated continue sentence
        ];
        let dialog_smart_single_mappings = vec![
            (MatchType::HardSeparator, DialogState::Unknown),        // PatternID 0 - hard sep + dialog opener
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 1 - hard sep + narrative char
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 2 - hard sep + EOF
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 3 - punctuated hard dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 4 - punctuated soft dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 5 - continuation punct before close → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 6 - continuation punct after close → Continue
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 7 - unpunctuated sentence boundary
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 8 - unpunctuated continue sentence
        ];
        state_patterns.insert(DialogState::DialogSmartSingleOpen, Regex::new_many(&dialog_smart_single_patterns)?);
        state_pattern_mappings.insert(DialogState::DialogSmartSingleOpen, dialog_smart_single_mappings);
        
        // DIALOG PARENTHETICAL ROUND STATE
        // WHY: Patterns match ) followed by punctuation outside the parentheses, then optional space and next sentence start
        let dialog_hard_paren_round_end = format!("{round_paren_close}{sentence_end_punct}({soft_separator}){unified_sentence_start_chars}");  // ). The or ). (
        let dialog_soft_paren_round_end = format!("{round_paren_close}{sentence_end_punct}({soft_separator}){not_all_sentence_start_chars}");
        
        // CONTINUATION PUNCTUATION PATTERNS for round parentheses
        // Pattern: continuation_punct + close + space + any → Continue (before close)
        let dialog_paren_round_continuation_before_end = format!("{non_sentence_ending_punct}{round_paren_close}({soft_separator}){unified_sentence_start_chars}");  // ,) The
        // WHY: Handle parenthetical closures with non-sentence punctuation (semicolon, comma, etc.) after close
        let dialog_paren_round_non_sentence_punct = format!("{round_paren_close}({non_sentence_ending_punct})");
        // Pattern: close + continuation_punct + space + any → Continue (after close)
        let dialog_paren_round_continuation_after_end = format!("{round_paren_close}{non_sentence_ending_punct}({soft_separator}){unified_sentence_start_chars}");  // ), The
        
        // UNPUNCTUATED PATTERNS for round parentheses
        // WHY: Handle simple parenthetical closures without any punctuation (continuation patterns)
        let dialog_paren_round_unpunctuated_split_end = format!("{round_paren_close}({soft_separator}){unified_sentence_start_chars}");  // ) The
        let dialog_paren_round_unpunctuated_continue_end = format!("{round_paren_close}({soft_separator}){not_all_sentence_start_chars}");
        
        let dialog_paren_round_patterns = vec![
            hard_sep_dialog_start.as_str(),               // PatternID 0 = hard sep + dialog opener
            hard_sep_narrative_start.as_str(),            // PatternID 1 = hard sep + narrative char
            hard_sep_eof.as_str(),                        // PatternID 2 = hard sep + EOF
            dialog_hard_paren_round_end.as_str(),         // PatternID 3 = punctuated sentence boundary
            dialog_soft_paren_round_end.as_str(),         // PatternID 4 = punctuated continue sentence
            dialog_paren_round_continuation_before_end.as_str(), // PatternID 5 = continuation punct before close → Continue
            dialog_paren_round_non_sentence_punct.as_str(), // PatternID 6 = non-sentence punctuation after close (legacy)
            dialog_paren_round_continuation_after_end.as_str(), // PatternID 7 = continuation punct after close → Continue
            dialog_paren_round_unpunctuated_split_end.as_str(), // PatternID 8 = unpunctuated sentence boundary
            dialog_paren_round_unpunctuated_continue_end.as_str(), // PatternID 9 = unpunctuated continue sentence
        ];
        let dialog_paren_round_mappings = vec![
            (MatchType::HardSeparator, DialogState::Unknown),        // PatternID 0 - hard sep + dialog opener
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 1 - hard sep + narrative char
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 2 - hard sep + EOF
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 3 - punctuated hard dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 4 - punctuated soft dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 5 - continuation punct before close → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 6 - non-sentence punctuation after close (legacy)
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 7 - continuation punct after close → Continue
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 8 - unpunctuated sentence boundary
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 9 - unpunctuated continue sentence
        ];
        state_patterns.insert(DialogState::DialogParenthheticalRound, Regex::new_many(&dialog_paren_round_patterns)?);
        state_pattern_mappings.insert(DialogState::DialogParenthheticalRound, dialog_paren_round_mappings);
        
        // DIALOG PARENTHETICAL SQUARE STATE
        let dialog_hard_paren_square_end = format!("{sentence_end_punct}{square_bracket_close}({soft_separator}){unified_sentence_start_chars}");  // .] The or .] (
        let dialog_soft_paren_square_end = format!("{sentence_end_punct}{square_bracket_close}({soft_separator}){not_all_sentence_start_chars}");
        
        // CONTINUATION PUNCTUATION PATTERNS for square brackets
        let dialog_paren_square_continuation_before_end = format!("{non_sentence_ending_punct}{square_bracket_close}({soft_separator}){unified_sentence_start_chars}");  // ,] The
        let dialog_paren_square_continuation_after_end = format!("{square_bracket_close}{non_sentence_ending_punct}({soft_separator}){unified_sentence_start_chars}");  // ], The
        
        // UNPUNCTUATED PATTERNS for square brackets
        let dialog_paren_square_unpunctuated_split_end = format!("{square_bracket_close}({soft_separator}){unified_sentence_start_chars}");  // ] The
        let dialog_paren_square_unpunctuated_continue_end = format!("{square_bracket_close}({soft_separator}){not_all_sentence_start_chars}");
        
        let dialog_paren_square_patterns = vec![
            hard_sep_dialog_start.as_str(),               // PatternID 0 = hard sep + dialog opener
            hard_sep_narrative_start.as_str(),            // PatternID 1 = hard sep + narrative char
            hard_sep_eof.as_str(),                        // PatternID 2 = hard sep + EOF
            dialog_hard_paren_square_end.as_str(),        // PatternID 3 = punctuated sentence boundary
            dialog_soft_paren_square_end.as_str(),        // PatternID 4 = punctuated continue sentence
            dialog_paren_square_continuation_before_end.as_str(), // PatternID 5 = continuation punct before close → Continue
            dialog_paren_square_continuation_after_end.as_str(), // PatternID 6 = continuation punct after close → Continue
            dialog_paren_square_unpunctuated_split_end.as_str(), // PatternID 7 = unpunctuated sentence boundary
            dialog_paren_square_unpunctuated_continue_end.as_str(), // PatternID 8 = unpunctuated continue sentence
        ];
        let dialog_paren_square_mappings = vec![
            (MatchType::HardSeparator, DialogState::Unknown),        // PatternID 0 - hard sep + dialog opener
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 1 - hard sep + narrative char
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 2 - hard sep + EOF
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 3 - punctuated hard dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 4 - punctuated soft dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 5 - continuation punct before close → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 6 - continuation punct after close → Continue
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 7 - unpunctuated sentence boundary
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 8 - unpunctuated continue sentence
        ];
        state_patterns.insert(DialogState::DialogParenthheticalSquare, Regex::new_many(&dialog_paren_square_patterns)?);
        state_pattern_mappings.insert(DialogState::DialogParenthheticalSquare, dialog_paren_square_mappings);
        
        // DIALOG PARENTHETICAL CURLY STATE
        let dialog_hard_paren_curly_end = format!("{sentence_end_punct}{curly_brace_close}({soft_separator}){unified_sentence_start_chars}");  // .} The or .} (
        let dialog_soft_paren_curly_end = format!("{sentence_end_punct}{curly_brace_close}({soft_separator}){not_all_sentence_start_chars}");
        
        // CONTINUATION PUNCTUATION PATTERNS for curly braces
        let dialog_paren_curly_continuation_before_end = format!("{non_sentence_ending_punct}{curly_brace_close}({soft_separator}){unified_sentence_start_chars}");  // ,} The
        let dialog_paren_curly_continuation_after_end = format!("{curly_brace_close}{non_sentence_ending_punct}({soft_separator}){unified_sentence_start_chars}");  // }, The
        
        // UNPUNCTUATED PATTERNS for curly braces
        let dialog_paren_curly_unpunctuated_split_end = format!("{curly_brace_close}({soft_separator}){unified_sentence_start_chars}");  // } The
        let dialog_paren_curly_unpunctuated_continue_end = format!("{curly_brace_close}({soft_separator}){not_all_sentence_start_chars}");
        
        let dialog_paren_curly_patterns = vec![
            hard_sep_dialog_start.as_str(),               // PatternID 0 = hard sep + dialog opener
            hard_sep_narrative_start.as_str(),            // PatternID 1 = hard sep + narrative char
            hard_sep_eof.as_str(),                        // PatternID 2 = hard sep + EOF
            dialog_hard_paren_curly_end.as_str(),         // PatternID 3 = punctuated sentence boundary
            dialog_soft_paren_curly_end.as_str(),         // PatternID 4 = punctuated continue sentence
            dialog_paren_curly_continuation_before_end.as_str(), // PatternID 5 = continuation punct before close → Continue
            dialog_paren_curly_continuation_after_end.as_str(), // PatternID 6 = continuation punct after close → Continue
            dialog_paren_curly_unpunctuated_split_end.as_str(), // PatternID 7 = unpunctuated sentence boundary
            dialog_paren_curly_unpunctuated_continue_end.as_str(), // PatternID 8 = unpunctuated continue sentence
        ];
        let dialog_paren_curly_mappings = vec![
            (MatchType::HardSeparator, DialogState::Unknown),        // PatternID 0 - hard sep + dialog opener
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 1 - hard sep + narrative char
            (MatchType::HardSeparator, DialogState::Narrative),      // PatternID 2 - hard sep + EOF
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 3 - punctuated hard dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 4 - punctuated soft dialog end
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 5 - continuation punct before close → Continue
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 6 - continuation punct after close → Continue
            (MatchType::DialogEnd, DialogState::Narrative),          // PatternID 7 - unpunctuated sentence boundary
            (MatchType::DialogSoftEnd, DialogState::Narrative),      // PatternID 8 - unpunctuated continue sentence
        ];
        state_patterns.insert(DialogState::DialogParenthheticalCurly, Regex::new_many(&dialog_paren_curly_patterns)?);
        state_pattern_mappings.insert(DialogState::DialogParenthheticalCurly, dialog_paren_curly_mappings);
        
        Ok(DialogStateMachine {
            state_patterns,
            state_pattern_mappings,
            abbreviation_checker: AbbreviationChecker::new(),
        })
    }
    
    
    pub fn detect_sentences(&self, text: &str) -> Result<Vec<DialogDetectedSentence>> {
        self.detect_sentences_internal(text, None).map(|(sentences, _)| sentences)
    }

    pub fn detect_sentences_with_debug(&self, text: &str) -> Result<(Vec<DialogDetectedSentence>, Vec<DebugTransitionInfo>)> {
        let mut debug_info = Vec::new();
        self.detect_sentences_internal(text, Some(&mut debug_info))
    }

    fn detect_sentences_internal(&self, text: &str, mut debug_collector: Option<&mut Vec<DebugTransitionInfo>>) -> Result<(Vec<DialogDetectedSentence>, Vec<DebugTransitionInfo>)> {
        debug!("Starting dialog state machine detection on {} characters", text.len());
        
        let mut sentences = Vec::new();
        let mut current_state = DialogState::Narrative;
        let mut sentence_start_byte = BytePos::new(0);
        let mut position_byte = BytePos::new(0);
        let mut remaining_text_handled = false;
        
        // PHASE 1: Use incremental position tracker instead of O(N) position conversions
        let mut position_tracker = PositionTracker::new(text);        
        
        while position_byte.0 < text.len() {
            let pattern = self.state_patterns.get(&current_state)
                .unwrap_or_else(|| panic!("No patterns defined for state {current_state:?}"));
            
            let pattern_mappings = self.state_pattern_mappings.get(&current_state)
                .unwrap_or_else(|| panic!("No pattern mappings defined for state {current_state:?}"));
            
            let input = Input::new(&text[position_byte.0..]);
            
            // Use state-specific multi-pattern regex
            if let Some(mat) = pattern.find(input) {
                let match_start_byte = position_byte.advance(mat.start());
                let match_end_byte = position_byte.advance(mat.end());
                let matched_text = &text[match_start_byte.0..match_end_byte.0];
                
                // Get PatternID and map directly to (MatchType, DialogState) using state-specific mappings
                let pattern_id = mat.pattern().as_usize();
                let (match_type, next_state) = if pattern_id < pattern_mappings.len() {
                    pattern_mappings[pattern_id].clone()
                } else {
                    // Fallback for invalid pattern ID
                    (MatchType::NarrativeGestureBoundary, DialogState::Narrative)
                };
                
                // Capture debug information if collector is provided
                let pattern_name = self.get_pattern_name(&current_state, pattern_id);
                let seam_context_start = match_start_byte.0.saturating_sub(10);
                let seam_context_end = (match_end_byte.0 + 10).min(text.len());
                
                // WHY: Ensure we slice at character boundaries to avoid UTF-8 panics
                let safe_start = (0..=seam_context_start).rev()
                    .find(|&i| text.is_char_boundary(i))
                    .unwrap_or(0);
                let safe_end = (seam_context_end..text.len())
                    .find(|&i| text.is_char_boundary(i))
                    .unwrap_or(text.len());
                let seam_text = text[safe_start..safe_end].to_string();
                
                
                // Handle special cases for hard separator and Dialog->Dialog transitions
                let (match_type, next_state) = if matches!(match_type, MatchType::HardSeparator) {
                    let (should_reject, target_state) = self.analyze_hard_separator_context(text, match_start_byte.0, matched_text);
                    if should_reject {
                        // Reject separator - treat as soft end (Continue)
                        (MatchType::DialogSoftEnd, current_state.clone())
                    } else {
                        // Accept separator - use determined target state
                        (match_type, target_state)
                    }
                } else if matches!(next_state, DialogState::Unknown) {
                    // Dialog->Dialog transition - determine target state from the opening delimiter (second-to-last char)
                    let chars: Vec<char> = matched_text.chars().collect();
                    let target_state = if chars.len() >= 2 {
                        match chars[chars.len() - 2] {  // Second-to-last character is the opening delimiter
                            '"' => DialogState::DialogDoubleQuote,
                            '\'' => DialogState::DialogSingleQuote,
                            '(' => DialogState::DialogParenthheticalRound,
                            '[' => DialogState::DialogParenthheticalSquare,
                            '{' => DialogState::DialogParenthheticalCurly,
                            '\u{201C}' => DialogState::DialogSmartDoubleOpen,
                            '\u{2018}' => DialogState::DialogSmartSingleOpen,
                            _ => DialogState::Narrative,
                        }
                    } else {
                        DialogState::Narrative
                    };
                    (match_type, target_state)
                } else {
                    (match_type, next_state)
                };
                
                match match_type {
                    MatchType::NarrativeToDialog => {
                        // N→D transition: creates sentence boundary AND transitions to dialog state
                        let sentence_end_byte = self.find_sent_sep_start(matched_text)
                            .map(|sep_offset| match_start_byte.advance(sep_offset))
                            .unwrap_or(match_start_byte);
                        
                        if sentence_end_byte.0 > sentence_start_byte.0 {
                            let content = text[sentence_start_byte.0..sentence_end_byte.0].trim().to_string();
                            if !content.is_empty() {
                                // WHY: Check for abbreviation false positives before creating sentence boundary
                                if self.abbreviation_checker.ends_with_title_abbreviation(&content) {
                                    // This is a false positive - don't create sentence boundary
                                    // Continue processing from current position without advancing sentence_start_byte
                                } else {
                                    // Use incremental position tracker instead of O(N) conversions
                                    let (__start_char, start_line, start_col) = position_tracker.advance_to_byte(sentence_start_byte)
                                        .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                                    let (__end_char, end_line, end_col) = position_tracker.advance_to_byte(sentence_end_byte)
                                        .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                                    
                                    sentences.push(DialogDetectedSentence {
                                        start_byte: sentence_start_byte,
                                        end_byte: sentence_end_byte,
                                        start_line,
                                        start_col,
                                        end_line,
                                        end_col,
                                    });
                                    
                                    // Next sentence starts after the separator
                                    let next_sentence_start_byte = self.find_sent_sep_end(matched_text)
                                        .map(|sep_end_offset| match_start_byte.advance(sep_end_offset))
                                        .unwrap_or(match_end_byte);
                                    
                                    sentence_start_byte = next_sentence_start_byte;
                                }
                            }
                        }
                    }
                    MatchType::NarrativeGestureBoundary => {
                        // This creates a sentence boundary - record the sentence
                        let sentence_end_byte = self.find_sent_sep_start(matched_text)
                            .map(|sep_offset| match_start_byte.advance(sep_offset))
                            .unwrap_or(match_start_byte);
                        
                        if sentence_end_byte.0 > sentence_start_byte.0 {
                            let content = text[sentence_start_byte.0..sentence_end_byte.0].trim().to_string();
                            if !content.is_empty() {
                                // WHY: Check for abbreviation false positives before creating sentence boundary
                                if self.abbreviation_checker.ends_with_title_abbreviation(&content) {
                                    // This is a false positive - don't create sentence boundary
                                    // Continue processing from current position without advancing sentence_start_byte
                                } else {
                                    // Use incremental position tracker instead of O(N) conversions
                                    let (__start_char, start_line, start_col) = position_tracker.advance_to_byte(sentence_start_byte)
                                        .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                                    let (__end_char, end_line, end_col) = position_tracker.advance_to_byte(sentence_end_byte)
                                        .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                                    
                                    sentences.push(DialogDetectedSentence {
                                        start_byte: sentence_start_byte,
                                        end_byte: sentence_end_byte,
                                        start_line,
                                        start_col,
                                        end_line,
                                        end_col,
                                    });
                                    
                                    // Next sentence starts after the separator
                                    let next_sentence_start_byte = self.find_sent_sep_end(matched_text)
                                        .map(|sep_end_offset| match_start_byte.advance(sep_end_offset))
                                        .unwrap_or(match_end_byte);
                                    
                                    sentence_start_byte = next_sentence_start_byte;
                                }
                            }
                        }
                    }
                    MatchType::DialogOpen => {
                        // State transition only - no sentence boundary created
                        // Continue from match start (include the opening punctuation in dialog)
                        // No sentence recorded, just state change
                    }
                    MatchType::DialogEnd => {
                        // Dialog end creates a sentence boundary
                        // Use separator logic to find where current sentence ends
                        let sentence_end_byte = self.find_sent_sep_start(matched_text)
                            .map(|sep_offset| match_start_byte.advance(sep_offset))
                            .unwrap_or(match_start_byte);
                        
                        if sentence_end_byte.0 > sentence_start_byte.0 {
                            let content = text[sentence_start_byte.0..sentence_end_byte.0].trim().to_string();
                            if !content.is_empty() {
                                // PHASE 1: Use incremental position tracker instead of O(N) conversions
                                let (_start_char, start_line, start_col) = position_tracker.advance_to_byte(sentence_start_byte)
                                    .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                                let (_end_char, end_line, end_col) = position_tracker.advance_to_byte(sentence_end_byte)
                                    .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                                
                                sentences.push(DialogDetectedSentence {
                                    start_byte: sentence_start_byte,
                                    end_byte: sentence_end_byte,
                                    start_line,
                                    start_col,
                                    end_line,
                                    end_col,
                                });
                            }
                        }
                        
                        let next_sentence_start_byte = self.find_sent_sep_end(matched_text)
                            .map(|sep_end_offset| match_start_byte.advance(sep_end_offset))
                            .unwrap_or(match_end_byte);
                        
                        sentence_start_byte = next_sentence_start_byte;
                    }
                    MatchType::DialogSoftEnd => {
                        // Soft dialog end - state transition only, no sentence boundary
                        // Continue the current sentence through the dialog close
                        // No sentence recorded, just state change
                    }
                    MatchType::HardSeparator => {
                        // Hard separator - record sentence and transition to Unknown
                        if sentence_start_byte.0 < match_start_byte.0 {
                            // For hard separators, preserve punctuation but trim trailing whitespace carefully
                            let raw_content = &text[sentence_start_byte.0..match_start_byte.0];
                            let content = raw_content.trim_start().trim_end_matches(char::is_whitespace).to_string();
                            if !content.is_empty() {
                                // PHASE 1: Use incremental position tracker instead of O(N) conversions
                                let (_start_char, start_line, start_col) = position_tracker.advance_to_byte(sentence_start_byte)
                                    .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                                let (_end_char, end_line, end_col) = position_tracker.advance_to_byte(match_start_byte)
                                    .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                                
                                sentences.push(DialogDetectedSentence {
                                    start_byte: sentence_start_byte,
                                    end_byte: match_start_byte,
                                    start_line,
                                    start_col,
                                    end_line,
                                    end_col,
                                });
                            }
                        }
                        
                        // Use find_sent_sep_end to handle patterns that consume extra characters
                        let next_sentence_start_byte = self.find_sent_sep_end(matched_text)
                            .map(|sep_end_offset| match_start_byte.advance(sep_end_offset))
                            .unwrap_or(match_end_byte);
                        
                        sentence_start_byte = next_sentence_start_byte;
                    }
                }
                
                // Determine actual transition type based on match type
                let transition_type = match match_type {
                    MatchType::NarrativeToDialog | MatchType::NarrativeGestureBoundary | MatchType::DialogEnd | MatchType::HardSeparator => TransitionType::Split,
                    MatchType::DialogOpen | MatchType::DialogSoftEnd => TransitionType::Continue,
                };
                
                // Collect debug information if requested
                if let Some(ref mut collector) = debug_collector {
                    collector.push(DebugTransitionInfo {
                        sentence_index: sentences.len(),
                        state_before: current_state.clone(),
                        state_after: next_state.clone(),
                        transition_type,
                        matched_pattern: matched_text.to_string(),
                        pattern_name,
                        seam_text,
                    });
                }
                
                // Update position and state
                position_byte = match_end_byte;
                current_state = next_state;
            } else {
                // No more boundaries found, handle remaining text
                if sentence_start_byte.0 < text.len() {
                    let content = text[sentence_start_byte.0..].trim().to_string();
                    if !content.is_empty() {
                        // PHASE 1: Use incremental position tracker instead of O(N) conversions
                        let (_start_char, start_line, start_col) = position_tracker.advance_to_byte(sentence_start_byte)
                            .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                        let (_end_char, end_line, end_col) = position_tracker.advance_to_byte(BytePos::new(text.len()))
                            .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                        
                        sentences.push(DialogDetectedSentence {
                            start_byte: sentence_start_byte,
                            end_byte: BytePos::new(text.len()),
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                        });
                    }
                }
                remaining_text_handled = true;
                break;
            }
        }
        
        // Handle any remaining text after the loop exits naturally (not via else clause)
        if !remaining_text_handled && sentence_start_byte.0 < text.len() {
            let content = text[sentence_start_byte.0..].trim().to_string();
            if !content.is_empty() {
                // WHY: Create new position tracker for final sentence since main tracker is at end of text
                let mut final_position_tracker = PositionTracker::new(text);
                let (_start_char, start_line, start_col) = final_position_tracker.advance_to_byte(sentence_start_byte)
                    .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                let (_end_char, end_line, end_col) = final_position_tracker.advance_to_byte(BytePos::new(text.len()))
                    .map_err(|e| anyhow::anyhow!("Position tracking error: {}", e))?;
                
                sentences.push(DialogDetectedSentence {
                    start_byte: sentence_start_byte,
                    end_byte: BytePos::new(text.len()),
                    start_line,
                    start_col,
                    end_line,
                    end_col,
                });
            }
        }
        
        let final_debug_info = debug_collector.map(|c| c.clone()).unwrap_or_default();
        info!("Dialog state machine detected {} sentences with {} debug transitions", sentences.len(), final_debug_info.len());
        Ok((sentences, final_debug_info))
    }
    
    /// Find hard separator position (handles both Unix \n\n and Windows \r\n\r\n)
    fn find_hard_separator(&self, text: &str) -> Option<(usize, usize)> {
        if let Some(pos) = text.find("\r\n\r\n") {
            return Some((pos, 4)); // position and length
        }
        if let Some(pos) = text.find("\n\n") {
            return Some((pos, 2)); // position and length
        }
        None
    }
    
    fn find_sent_sep_start(&self, matched_boundary: &str) -> Option<usize> {
        // Find where SENT_SEP starts within a SENT_END + SENT_SEP + SENT_START match
        // Look for the first whitespace character or hard separator
        if let Some((hard_sep_pos, _)) = self.find_hard_separator(matched_boundary) {
            return Some(hard_sep_pos);
        }
        
        // Find first whitespace after punctuation
        let mut found_punct = false;
        for (i, ch) in matched_boundary.char_indices() {
            if ".!?".contains(ch) || "\"'".contains(ch) || ")]}>".contains(ch) {
                found_punct = true;
            } else if found_punct && ch.is_whitespace() {
                return Some(i);
            }
        }
        
        None
    }
    
    
    fn find_sent_sep_end(&self, matched_boundary: &str) -> Option<usize> {
        // Find where SENT_SEP ends within a SENT_END + SENT_SEP + SENT_START match
        // This is where the next sentence should start
        if let Some((hard_sep_pos, sep_len)) = self.find_hard_separator(matched_boundary) {
            // Check if this is a hard separator pattern that consumed an extra character
            // (hard_sep_dialog_start or hard_sep_narrative_start)
            let after_sep = hard_sep_pos + sep_len;
            if after_sep < matched_boundary.len() {
                // There's a consumed character - next sentence should start at the consumed character
                return Some(after_sep);
            } else {
                // No consumed character (hard_sep_eof) - next sentence starts after separator
                return Some(after_sep);
            }
        }
        
        // Find the end of whitespace sequence - where SENT_START begins
        let mut in_whitespace = false;
        
        for (i, ch) in matched_boundary.char_indices() {
            if ch.is_whitespace() {
                if !in_whitespace {
                    in_whitespace = true;
                }
            } else if in_whitespace {
                // Found non-whitespace after whitespace - this is start of SENT_START
                return Some(i);
            }
        }
        
        // If we end in whitespace, return end of string
        if in_whitespace {
            Some(matched_boundary.len())
        } else {
            None
        }
    }
}

/// Main dialog detector with dual API support
pub struct SentenceDetectorDialog {
    machine: DialogStateMachine,
}

impl SentenceDetectorDialog {
    /// Create new dialog detector 
    pub fn new() -> Result<Self> {
        let machine = DialogStateMachine::new()?;
        Ok(Self { machine })
    }

    /// Detect sentences with borrowed API (zero allocations, mmap-friendly)
    pub fn detect_sentences_borrowed<'a>(&self, text: &'a str) -> Result<Vec<DetectedSentenceBorrowed<'a>>> {
        let dialog_sentences = self.machine.detect_sentences(text)?;
        
        let borrowed_sentences = dialog_sentences
            .into_iter()
            .enumerate()
            .map(|(index, dialog_sentence)| {
                // Use precomputed byte positions for O(1) slice creation
                let start_byte = dialog_sentence.start_byte.0;
                let end_byte = dialog_sentence.end_byte.0;
                let raw_content = &text[start_byte..end_byte];
                
                DetectedSentenceBorrowed {
                    index,
                    raw_content,
                    span: Span {
                        start_line: dialog_sentence.start_line.into(),
                        start_col: dialog_sentence.start_col.into(),
                        end_line: dialog_sentence.end_line.into(),
                        end_col: dialog_sentence.end_col.into(),
                    },
                }
            })
            .collect();
            
        Ok(borrowed_sentences)
    }

    /// Detect sentences with debug info using borrowed API (zero allocations, mmap-friendly)
    /// WHY: Conditional debug tracking without performance impact when disabled
    pub fn detect_sentences_borrowed_with_debug<'a>(&self, text: &'a str) -> Result<(Vec<DetectedSentenceBorrowed<'a>>, Vec<DebugTransitionInfo>)> {
        let (dialog_sentences, debug_info) = self.machine.detect_sentences_with_debug(text)?;
        
        let borrowed_sentences = dialog_sentences
            .into_iter()
            .enumerate()
            .map(|(index, dialog_sentence)| {
                // Use precomputed byte positions for O(1) slice creation
                let start_byte = dialog_sentence.start_byte.0;
                let end_byte = dialog_sentence.end_byte.0;
                let raw_content = &text[start_byte..end_byte];
                
                DetectedSentenceBorrowed {
                    index,
                    raw_content,
                    span: Span {
                        start_line: dialog_sentence.start_line.into(),
                        start_col: dialog_sentence.start_col.into(),
                        end_line: dialog_sentence.end_line.into(),
                        end_col: dialog_sentence.end_col.into(),
                    },
                }
            })
            .collect();
            
        Ok((borrowed_sentences, debug_info))
    }


}

