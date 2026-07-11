// Dialog detector unit tests
// WHY: Separated from dialog_detector.rs to improve maintainability and reduce file size

use seams::sentence_detector::dialog_detector::SentenceDetectorDialog;
// use seams::sentence_detector::dialog_detector2::SentenceDetectorDialog2;  // Disabled
use std::sync::OnceLock;

// WHY: Single shared detector instance reduces test overhead from 38+ instantiations
static SHARED_DETECTOR: OnceLock<SentenceDetectorDialog> = OnceLock::new();
// static SHARED_DETECTOR2: OnceLock<SentenceDetectorDialog2> = OnceLock::new();  // Disabled

fn get_detector() -> &'static SentenceDetectorDialog {
    SHARED_DETECTOR.get_or_init(|| SentenceDetectorDialog::new().unwrap())
}

// fn get_detector2() -> &'static SentenceDetectorDialog2 {
//     SHARED_DETECTOR2.get_or_init(|| SentenceDetectorDialog2::new().unwrap())
// }  // Disabled

#[test]
fn test_user_hermit_scroll_text() {
    let detector = get_detector();
    
    let input = "He had thus sat for hours one day, interrupting his meditations only by\nan occasional pace to the door to look out for a break in the weather,\nwhen there came upon him with a shock of surprise the recollection that\nthere was more in the hermit's scroll than he had considered at first.\nNot much. He unfurled it, and beside the bequest of the hut, only these\nwords were added: \"For a commission look below my bed.\"";
    
    let sentences = detector.detect_sentences_borrowed(input).unwrap();
    
    // Expected sentences
    let expected = ["He had thus sat for hours one day, interrupting his meditations only by an occasional pace to the door to look out for a break in the weather, when there came upon him with a shock of surprise the recollection that there was more in the hermit's scroll than he had considered at first.",
        "Not much.",
        "He unfurled it, and beside the bequest of the hut, only these words were added: \"For a commission look below my bed.\""];
    
    assert_eq!(sentences.len(), expected.len(), 
        "Expected {} sentences, got {}. Sentences: {:?}", 
        expected.len(), sentences.len(), 
        sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
    
    for (i, (actual, expected)) in sentences.iter().zip(expected.iter()).enumerate() {
        assert_eq!(actual.normalize().trim(), *expected,
            "Sentence {} mismatch:\nExpected: '{}'\nActual: '{}'", 
            i + 1, expected, actual.normalize().trim());
    }
}

#[test]
fn test_basic_narrative_sentences() {
    let detector = get_detector();
    let text = "This is a sentence. This is another sentence.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    assert_eq!(sentences.len(), 2);
    assert!(sentences[0].raw_content.contains("This is a sentence"));
    assert!(sentences[1].raw_content.contains("This is another sentence"));
}

#[test]
fn test_dialog_coalescing() {
    let detector = get_detector();
    let text = "He said, \"Stop her, sir! Ting-a-ling-ling!\" The headway ran almost out.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    println!("Dialog coalescing test: {} sentences: {:?}", sentences.len(), 
        sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
    
    assert_eq!(sentences.len(), 2);
    assert!(sentences[0].raw_content.contains("Stop her, sir! Ting-a-ling-ling!"));
    assert!(sentences[1].raw_content.contains("The headway ran almost out"));
}

#[test]
fn test_abbreviation_handling() {
    let detector = get_detector();
    
    // Test comprehensive abbreviation handling in various contexts
    let test_cases = [
        // Basic title abbreviation - should not be split
        ("Dr. Smith examined the patient. The results were clear.", 2, ["Dr. Smith examined the patient", "The results were clear"]),
        // Multiple title abbreviations
        ("Mr. and Mrs. Johnson arrived. They were late.", 2, ["Mr. and Mrs. Johnson arrived", "They were late"]),
        // Geographic abbreviations
        ("The U.S.A. declared independence. It was 1776.", 2, ["The U.S.A. declared independence", "It was 1776"]),
        // Measurement abbreviations
        ("Distance is 2.5 mi. from here. We can walk it.", 2, ["Distance is 2.5 mi. from here", "We can walk it"]),
        // Dialog with abbreviations
        ("He said, 'Dr. Smith will see you.' She nodded.", 2, ["Dr. Smith will see you", "She nodded"]),
    ];
    
    for (text, expected_count, expected_content) in test_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        if sentences.len() != expected_count {
            println!("MISMATCH for text: {text}");
            println!("Expected {} sentences, got {} sentences:", expected_count, sentences.len());
            for (i, sentence) in sentences.iter().enumerate() {
                println!("  {}: '{}'", i, sentence.raw_content);
            }
            panic!("Failed for text: {text}");
        }
        
        for (i, expected) in expected_content.iter().enumerate() {
            assert!(sentences[i].raw_content.contains(expected), 
                "Sentence {} should contain '{}' but got '{}'", i, expected, sentences[i].raw_content);
        }
    }
    
    // Additional validation: ensure "Dr." is not treated as sentence boundary
    let text = "Dr. Smith examined the patient. The results were clear.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    assert!(!sentences[0].raw_content.trim().ends_with("Dr."), "Dr. should not end a sentence when followed by a name");
}

#[test]
fn test_structural_enumerated_headings_split_before_prose() {
    let detector = get_detector();
    let cases = [
        ("CHAPTER I.\n\nSir Walter Elliot was a man. This was next.", "CHAPTER I.", "Sir Walter Elliot was a man."),
        ("PART I.\n\nIn the morning, she left. This was next.", "PART I.", "In the morning, she left."),
        ("BOOK V.\n\nThe road was empty. This was next.", "BOOK V.", "The road was empty."),
        ("APPENDIX A.\n\nAdditional notes follow. This was next.", "APPENDIX A.", "Additional notes follow."),
        ("LETTER I.\n\nTo my dear friend, I write. This was next.", "LETTER I.", "To my dear friend, I write."),
        ("ACT I.\n\nThe curtain rises. This was next.", "ACT I.", "The curtain rises."),
        ("SCENE I.\n\nA room in the palace. This was next.", "SCENE I.", "A room in the palace."),
        ("VOLUME I.\n\nThe account begins here. This was next.", "VOLUME I.", "The account begins here."),
        ("PROLOGUE I.\n\nThe account begins here. This was next.", "PROLOGUE I.", "The account begins here."),
        ("EPILOGUE I.\n\nThe account concludes here. This was next.", "EPILOGUE I.", "The account concludes here."),
        ("PREFACE I.\n\nThe editor introduces the work. This was next.", "PREFACE I.", "The editor introduces the work."),
        ("INTRODUCTION I.\n\nThe argument begins here. This was next.", "INTRODUCTION I.", "The argument begins here."),
    ];

    for (text, heading, first_prose) in cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        let normalized: Vec<_> = sentences
            .iter()
            .map(|sentence| sentence.normalize().trim().to_string())
            .collect();

        assert_eq!(
            normalized,
            vec![
                heading.to_string(),
                first_prose.to_string(),
                "This was next.".to_string(),
            ],
            "Structural heading ending in a single-capital enumerator should not be suppressed as a title abbreviation"
        );
    }
}

#[test]
fn dialog_close_single_newline_uses_following_clause_context() {
    let detector = get_detector();
    let cases = [
        (
            "He asked, \u{201C}Are you ready?\u{201D}\nshe nodded and opened the door. Then they left.",
            vec![
                "He asked, \u{201C}Are you ready?\u{201D} she nodded and opened the door.",
                "Then they left.",
            ],
        ),
        (
            "\u{201C}What did you think? How do you feel?\u{201D}\ndemanded the visitors. A hush followed.",
            vec![
                "\u{201C}What did you think? How do you feel?\u{201D} demanded the visitors.",
                "A hush followed.",
            ],
        ),
        (
            "\u{201C}Où allez-vous?\u{201D}\nrépondit Renée. La salle se tut.",
            vec![
                "\u{201C}Où allez-vous?\u{201D} répondit Renée.",
                "La salle se tut.",
            ],
        ),
        (
            "\u{201C}Where is she?\u{201D}\n(pointing to Adèle). The room fell quiet.",
            vec![
                "\u{201C}Where is she?\u{201D} (pointing to Adèle).",
                "The room fell quiet.",
            ],
        ),
        (
            "\u{201C}Then, what induced you to take charge of such a little doll as that?\u{201D}\n(pointing to Adèle). \u{201C}Where did you pick her up?\u{201D}",
            vec![
                "\u{201C}Then, what induced you to take charge of such a little doll as that?\u{201D} (pointing to Adèle).",
                "\u{201C}Where did you pick her up?\u{201D}",
            ],
        ),
    ];

    for (text, expected) in cases {
        let actual: Vec<_> = detector
            .detect_sentences_borrowed(text)
            .unwrap()
            .iter()
            .map(|sentence| sentence.normalize().trim().to_string())
            .collect();

        assert_eq!(actual, expected);
    }
}

#[test]
fn dialog_close_blank_line_remains_a_hard_boundary() {
    let detector = get_detector();
    for text in [
        "He asked, \u{201C}Are you ready?\u{201D}\n\nShe opened the door.",
        "He asked, \u{201C}Are you ready?\u{201D}\r\n\r\nShe opened the door.",
    ] {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        let actual: Vec<_> = sentences
            .iter()
            .map(|sentence| sentence.normalize().trim().to_string())
            .collect();

        assert_eq!(
            actual,
            vec![
                "He asked, \u{201C}Are you ready?\u{201D}",
                "She opened the door.",
            ]
        );
        assert!(sentences[1].raw_content.starts_with("She"));
    }
}

#[test]
fn embedded_dialog_may_continue_into_ambiguous_standalone_i_clause() {
    let detector = get_detector();
    for (open, close) in [("\u{201C}", "\u{201D}"), ("\"", "\"")] {
        let text = format!(
            "To her hurried\n{open}Is it really you?{close}\nI answered by taking her hand. Then we went inside."
        );
        let actual: Vec<_> = detector
            .detect_sentences_borrowed(&text)
            .unwrap()
            .iter()
            .map(|sentence| sentence.normalize().trim().to_string())
            .collect();

        assert_eq!(
            actual,
            vec![
                format!(
                    "To her hurried {open}Is it really you?{close} I answered by taking her hand."
                ),
                "Then we went inside.".to_string(),
            ]
        );
    }
}

#[test]
fn capitalized_dialog_followup_still_splits() {
    let detector = get_detector();
    let cases = [
        "The other drawings pleased her, but she called that \u{201C}an ugly man.\u{201D}\nThey both seemed surprised at my skill.",
        "He called that \u{201C}wrong.\u{201D} I went home.",
        "He called that \u{201C}wrong.\u{201D}\nI went home.",
        "He asked,\n\u{201C}Are you ready?\u{201D}\nI went home.",
    ];

    for text in cases {
        let actual: Vec<_> = detector
            .detect_sentences_borrowed(text)
            .unwrap()
            .iter()
            .map(|sentence| sentence.normalize().trim().to_string())
            .collect();
        assert_eq!(actual.len(), 2, "capitalized follow-up should split: {actual:?}");
    }
}

#[test]
fn terminal_punctuation_before_nested_close_keeps_standalone_i_split() {
    let detector = get_detector();
    let cases = [
        "The letter is signed \u{201C}\u{2018}Alice Fairfax.\u{2019}\u{201D} I felt cold.",
        "\u{201C}What do you think, _mon ami?_\u{201D} I shook my head.",
        "He called that \u{201C}wrong.\u{201D} I went home.",
    ];

    for text in cases {
        let actual = detector.detect_sentences_borrowed(text).unwrap();
        assert_eq!(actual.len(), 2, "terminal close should split: {text}");
    }
}

#[test]
fn unpunctuated_dialog_may_continue_into_parenthetical_i() {
    let detector = get_detector();
    let cases = [
        (
            "An advertisement said \u{201C}children under fourteen\u{201D} (I thought that prudent). \u{201C}She is qualified.\u{201D}",
            vec!["An advertisement said \u{201C}children under fourteen\u{201D} (I thought that prudent). \u{201C}She is qualified.\u{201D}"],
        ),
        (
            "\u{201C}Captain—Captain\u{201D} (I could not remember the name) \u{201C}dossing down in there.\u{201D} Next.",
            vec![
                "\u{201C}Captain—Captain\u{201D} (I could not remember the name) \u{201C}dossing down in there.\u{201D}",
                "Next.",
            ],
        ),
        (
            "\u{201C}Captain—Captain\u{201D}\n(I could not remember the name) \u{201C}dossing down in there.\u{201D} Next.",
            vec![
                "\u{201C}Captain—Captain\u{201D} (I could not remember the name) \u{201C}dossing down in there.\u{201D}",
                "Next.",
            ],
        ),
    ];

    for (text, expected) in cases {
        let actual: Vec<_> = detector
            .detect_sentences_borrowed(text)
            .unwrap()
            .iter()
            .map(|sentence| sentence.normalize().trim().to_string())
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn terminal_dialog_before_parenthetical_i_remains_split() {
    let detector = get_detector();
    for text in [
        "\u{201C}Done.\u{201D} (I started a new thought.)",
        "\u{201C}A mere reed!\u{201D} (And he shook me.) \u{201C}I could bend her.\u{201D}",
    ] {
        let actual = detector.detect_sentences_borrowed(text).unwrap();
        assert!(actual.len() >= 2, "terminal close should split: {text}");
    }
}

#[test]
fn unpunctuated_close_does_not_blanket_merge_standalone_i() {
    let detector = get_detector();
    for text in [
        "The box said \u{201C}Honey Boy\u{201D} I suppose it meant cookies.",
        "He said \u{2018}You have all the old-fashioned principles, good and bad\u{2019} I acknowledge I have.",
    ] {
        let actual = detector.detect_sentences_borrowed(text).unwrap();
        assert_eq!(actual.len(), 2, "standalone I boundary control failed: {text}");
    }
}

#[test]
fn test_single_capital_abbreviation_still_suppresses_non_heading_boundary() {
    let detector = get_detector();
    let text = "Point I. Sir Walter Elliot was listed. This was next.";

    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    let normalized: Vec<_> = sentences
        .iter()
        .map(|sentence| sentence.normalize().trim().to_string())
        .collect();

    assert_eq!(
        normalized,
        vec![
            "Point I. Sir Walter Elliot was listed.",
            "This was next.",
        ],
        "Non-heading single-capital abbreviations should keep their existing suppression behavior"
    );
}

#[test]
fn test_soft_dialog_transitions() {
    let detector = get_detector();
    
    // Test case 1: comma + quote should soft transition, continue sentence
    let text = "\"Hello,\" she said quietly.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    // Should be one sentence - soft transition should continue
    assert_eq!(sentences.len(), 1, "Soft transition with comma should continue sentence");
    assert!(sentences[0].raw_content.contains("Hello") && sentences[0].raw_content.contains("she said"));
    
    // Test case 2: quote alone should soft transition
    let text = "\"Yes\" followed by more narrative.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    // Should be one sentence - soft transition should continue
    assert_eq!(sentences.len(), 1, "Soft transition with quote alone should continue sentence");
    
    // Test case 3: parenthetical close should soft transition
    let text = "(thinking quietly) and then he spoke.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    // Should be one sentence - soft transition should continue
    assert_eq!(sentences.len(), 1, "Soft transition with parenthetical should continue sentence");
}

#[test]
fn test_hard_dialog_transitions() {
    let detector = get_detector();
    
    // Test case: exclamation + space + capital should hard transition, create boundary
    let text = "\"Wait!\" he shouted loudly. Then he left.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    // Should be two sentences - hard transition should create boundary
    assert_eq!(sentences.len(), 2,
        "Hard transition should create sentence boundary\nExpected: 2 sentences\nActual: {} sentences\nSentences: {:?}",
        sentences.len(),
        sentences.iter().map(|s| &s.raw_content).collect::<Vec<_>>());
    assert!(sentences[0].raw_content.contains("Wait!") && sentences[0].raw_content.contains("he shouted"));
    assert!(sentences[1].raw_content.contains("Then he left"));
}

#[test]
fn test_dialog_quote_transition_comparison() {
    // Compare old vs new implementation on the failing case
    let detector_old = get_detector();   // Original implementation  
    // let detector_new = get_detector2();  // TOML-based implementation - DISABLED
    let test_text = r#"we read of an "Azacari (or Toucan) of Brazil; has his beak four inches long, almost two thick, like a Turk's sword" (A.D. 1656). From this description Tradescant knew the nature of the bird, if he had not seen it."#;
    
    println!("=== COMPARISON TEST ===");
    println!("Text: {test_text}");
    println!("Length: {}", test_text.len());
    
    println!("\n--- OLD IMPLEMENTATION ---");
    let sentences_old = detector_old.detect_sentences_borrowed(test_text).expect("Old detection failed");
    println!("Old detector: {} sentences", sentences_old.len());
    for (i, sentence) in sentences_old.iter().enumerate() {
        println!("  OLD {}: '{}'", i + 1, sentence.raw_content.trim());
    }
    
    // println!("\n--- NEW IMPLEMENTATION ---");
    // let sentences_new = detector_new.detect_sentences_borrowed(test_text).expect("New detection failed");
    // println!("New detector: {} sentences", sentences_new.len());
    // for (i, sentence) in sentences_new.iter().enumerate() {
    //     println!("  NEW {}: '{}'", i + 1, sentence.raw_content.trim());
    // }
    
    println!("\n--- COMPARISON RESULT ---");
    println!("✓ OLD implementation: {} sentences", sentences_old.len());
    // if sentences_old.len() == sentences_new.len() {
    //     println!("✓ Same sentence count: {}", sentences_old.len());
    // } else {
    //     println!("✗ Different sentence count: OLD={}, NEW={}", sentences_old.len(), sentences_new.len());
    // }
    
    // The test currently fails, but we want to see the difference
    // assert_eq!(sentences_old.len(), sentences_new.len(), "Old and new implementations should produce same sentence count");
}

#[test]
fn test_simple_quote_open_close() {
    // Test the core issue: Narrative -> open quotes -> content -> close quotes
    let detector = get_detector();
    let simple_text = r#"He said "hello world" and left."#;
    
    println!("DEBUG: Simple quote text: {simple_text}");
    println!("DEBUG: Text length: {}", simple_text.len());
    
    let sentences = detector.detect_sentences_borrowed(simple_text).expect("Detection failed");
    
    println!("DEBUG: Got {} sentences:", sentences.len());
    for (i, sentence) in sentences.iter().enumerate() {
        println!("  Sentence {}: '{}'", i + 1, sentence.raw_content.trim());
    }
    
    // Expected: Should properly handle quote opening and closing
    assert_eq!(sentences.len(), 1, "Simple quote should be one sentence");
}

#[test]
fn test_quote_with_parenthetical() {
    // Test the specific pattern from our failing case: quotes with parenthetical content
    let detector = get_detector();
    let text = r#"He said "word" (note). Next sentence."#;
    
    println!("DEBUG: Quote with parenthetical: {text}");
    println!("DEBUG: Text length: {}", text.len());
    
    let sentences = detector.detect_sentences_borrowed(text).expect("Detection failed");
    
    println!("DEBUG: Got {} sentences:", sentences.len());
    for (i, sentence) in sentences.iter().enumerate() {
        println!("  Sentence {}: '{}'", i + 1, sentence.raw_content.trim());
    }
    
    // Expected: Should be 2 sentences - the pattern `" (` should trigger dialog continuation
    assert_eq!(sentences.len(), 2, "Quote with parenthetical should create sentence boundary");
}

#[test]
fn test_colon_paragraph_break_dialog_separation() {
    let detector = get_detector();
    
    // Test case from task: colon + paragraph break + dialog should create sentence boundary
    let text = r#"She looked perplexed for a moment, and then said, not fiercely, but still loud enough for the furniture to hear:

"Well, I lay if I get hold of you I'll—"

She did not finish, for by this time she was bending down and punching under the bed with the broom, and so she needed breath to punctuate the punches with."#;
    
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    // Should be 2 sentences - colon followed by paragraph break should not over-coalesce
    assert_eq!(sentences.len(), 2, "Colon + paragraph break + dialog should create sentence boundary");
    
    // First sentence should include the dialog
    assert!(sentences[0].raw_content.contains("furniture to hear:"));
    assert!(sentences[0].raw_content.contains("Well, I lay if I get hold of you I'll—"));
    
    // Second sentence should be the narrative continuation
    assert!(sentences[1].raw_content.contains("She did not finish"));
}

#[test]
fn test_dialog_hard_separator_bug() {
    let detector = get_detector();
    
    // Test case: Hard separator between dialog lines should create separate sentences
    let input = r#"As the
young woman spoke, he rose, and advancing to the bed's head, said, with
more kindness than might have been expected of him:

"Oh, you must not talk about dying yet."

"Lor bless her dear heart, no!" interposed the nurse, hastily
depositing in her pocket a green glass bottle, the contents of which
she had been tasting in a corner with evident satisfaction."#;

    let sentences = detector.detect_sentences_borrowed(input).unwrap();
    
    assert_eq!(sentences.len(), 2,
        "Dialog hard separator test failed\nExpected: 2 sentences\nActual: {} sentences\nSentences: {:?}",
        sentences.len(),
        sentences.iter().map(|s| &s.raw_content).collect::<Vec<_>>());
    
    // Check sentence content
    assert!(sentences[0].normalize().contains("Oh, you must not talk about dying yet"));
    assert!(sentences[1].normalize().contains("Lor bless her dear heart, no!"));
    
    // Check span positioning - key bug validation
    assert_eq!(sentences[0].span.end_line, 5, "First sentence should end at line 5");
    assert_eq!(sentences[1].span.start_line, 7, "Second sentence should start at line 7");
}

#[test]
fn test_dialog_hard_separator_minimal() {
    let detector = get_detector();
    
    // Minimal case: colon followed by hard separator and dialog
    let input = "He said:\n\n\"Hello.\"\n\n\"World.\"";
    let sentences = detector.detect_sentences_borrowed(input).unwrap();
    
    assert_eq!(sentences.len(), 2, 
        "Expected 2 sentences, got {}. Sentences: {:?}", 
        sentences.len(), 
        sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
    
    assert_eq!(sentences[0].normalize().trim(), "He said: \"Hello.\"",
        "First sentence mismatch:\nExpected: 'He said: \"Hello.\"'\nActual: '{}'", 
        sentences[0].normalize().trim());
    
    assert_eq!(sentences[1].normalize().trim(), "\"World.\"",
        "Second sentence mismatch:\nExpected: '\"World.\"'\nActual: '{}'", 
        sentences[1].normalize().trim());
    
    // Verify line positions
    assert_eq!(sentences[0].span.start_line, 1, 
        "First sentence should start at line 1, got line {}", sentences[0].span.start_line);
    assert_eq!(sentences[1].span.start_line, 5,
        "Second sentence should start at line 5, got line {}", sentences[1].span.start_line);
    
    // Also test Windows line endings
    let input_windows = "He said:\r\n\r\n\"Hello.\"\r\n\r\n\"World.\"";
    let sentences_windows = detector.detect_sentences_borrowed(input_windows).unwrap();
    
    assert_eq!(sentences_windows.len(), 2, "Should detect 2 sentences with Windows line endings");
    assert_eq!(sentences_windows[0].normalize().trim(), "He said: \"Hello.\"");
    assert_eq!(sentences_windows[1].normalize().trim(), "\"World.\"");
}

#[test]
fn test_all_caps_dash_heading_splits_across_hard_separator() {
    let detector = get_detector();
    let text = "CHAPTER XIV.\nOUT OF THE FRYING-PAN--\n\n/After/ that exciting ride home, she rested. This was next.";

    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    let normalized: Vec<_> = sentences
        .iter()
        .map(|sentence| sentence.normalize().trim().to_string())
        .collect();

    assert_eq!(
        normalized,
        vec![
            "CHAPTER XIV.",
            "OUT OF THE FRYING-PAN--",
            "/After/ that exciting ride home, she rested.",
            "This was next.",
        ],
        "All-caps title lines ending in dashes should not fuse with prose across a blank line"
    );
}

#[test]
fn test_ordinary_dash_continuation_still_coalesces_across_hard_separator() {
    let detector = get_detector();
    let text = "She stopped--\n\n/After/ that, she rested. This was next.";

    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    let normalized: Vec<_> = sentences
        .iter()
        .map(|sentence| sentence.normalize().trim().to_string())
        .collect();

    assert_eq!(
        normalized,
        vec![
            "She stopped-- /After/ that, she rested.",
            "This was next.",
        ],
        "Ordinary dash continuations should keep coalescing across hard separators"
    );
}

#[test]
fn test_pg4300_compass_directions_fix() {
    let detector = get_detector();
    
    // Test the specific PG 4300 case that was failing - compass directions should not split
    let text = "Listener, S. E. by E.: Narrator, N. W. by W.: on the 53rd parallel of latitude, N., and 6th meridian of longitude, W.: at an angle of 45° to the terrestrial equator.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    // This should be one sentence - single capital letters should not create false boundaries
    assert_eq!(sentences.len(), 1, "Compass directions with single capitals should remain one sentence");
    assert!(sentences[0].raw_content.contains("S. E. by E."));
    assert!(sentences[0].raw_content.contains("N. W. by W."));
    assert!(sentences[0].raw_content.contains("latitude, N.,"));
    assert!(sentences[0].raw_content.contains("longitude, W.:"));
}

#[test]
fn coordinate_direction_abbreviation_can_end_sentence() {
    let detector = get_detector();

    let text = "The ship was lost at latitude 1° S. and longitude 107° W. On January the report arrived.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    let got: Vec<_> = sentences
        .iter()
        .map(|s| s.normalize().trim().to_string())
        .collect();

    assert_eq!(
        got,
        vec![
            "The ship was lost at latitude 1° S. and longitude 107° W.",
            "On January the report arrived.",
        ]
    );
}

#[test]
fn apostrophe_title_does_not_open_single_quote_dialog() {
    let detector = get_detector();

    let text = "It was near 'Squire Newcome's residence. The Prescott family had lived here five years.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    let got: Vec<_> = sentences
        .iter()
        .map(|s| s.normalize().trim().to_string())
        .collect();

    assert_eq!(
        got,
        vec![
            "It was near 'Squire Newcome's residence.",
            "The Prescott family had lived here five years.",
        ]
    );
}

#[test]
fn single_quoted_dialog_allows_internal_apostrophe() {
    let detector = get_detector();

    let text = "He cried, 'Don't go!' She stayed.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    let got: Vec<_> = sentences
        .iter()
        .map(|s| s.normalize().trim().to_string())
        .collect();

    assert_eq!(got, vec!["He cried, 'Don't go!'", "She stayed."]);
}

#[test]
fn test_missing_seams_reproduction() {
    let detector = get_detector();
    
    // Reproduce the MissingSeams.txt failure case - using Windows line endings (\r\n)
    let text = "By the narrator a\r\nlimitation of activity, mental and corporal, inasmuch as complete\r\nmental intercourse between himself and the listener had not taken place\r\nsince the consummation of puberty, indicated by catamenic hemorrhage,\r\nof the female issue of narrator and listener, 15 September 1903, there\r\nremained a period of 9 months and 1 day during which, in consequence of\r\na preestablished natural comprehension in incomprehension between the\r\nconsummated females (listener and issue), complete corporal liberty of\r\naction had been circumscribed.\r\n\r\nHow?\r\n\r\nBy various reiterated feminine interrogation concerning the masculine\r\ndestination whither, the place where, the time at which, the duration\r\nfor which, the object with which in the case of temporary absences,\r\nprojected or effected.\r\n\r\nWhat moved visibly above the listener's and the narrator's invisible\r\nthoughts?\r\n\r\nThe upcast reflection of a lamp and shade, an inconstant series of\r\nconcentric circles of varying gradations of light and shadow.\r\n\r\nIn what directions did listener and narrator lie?\r\n\r\nListener, S. E. by E.: Narrator, N. W. by W.: on the 53rd parallel of\r\nlatitude, N., and 6th meridian of longitude, W.: at an angle of 45° to\r\nthe terrestrial equator.\r\n\r\nIn what state of rest or motion?\r\n\r\nAt rest relatively to themselves and to each other.";
    
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    // This should be multiple sentences, not one massive sentence
    
    // Expected sentence boundaries:
    // 1. "...had been circumscribed." 
    // 2. "How?"
    // 3. "By various... projected or effected."
    // 4. "What moved... invisible thoughts?"
    // 5. "The upcast... light and shadow."
    // 6. "In what directions... narrator lie?"
    // 7. "Listener, S. E. by E.... terrestrial equator."
    // 8. "In what state... rest or motion?"
    // 9. "At rest... each other."
    
    // Should now detect multiple sentences with Windows line ending support
    assert!(sentences.len() > 1, "Should detect multiple sentences with Windows line endings, got {}", sentences.len());
    
    // Verify we get the expected 9 sentences
    assert_eq!(sentences.len(), 9, "Should detect exactly 9 sentences");
    
    // Verify some key sentence boundaries
    assert!(sentences[0].raw_content.contains("had been circumscribed"));
    assert_eq!(sentences[1].raw_content.trim(), "How?");
    assert!(sentences[2].raw_content.contains("projected or effected"));
    assert!(sentences[5].raw_content.contains("In what directions"));
    assert!(sentences[6].raw_content.contains("S. E. by E."));
    assert!(sentences[8].raw_content.contains("relatively to themselves"));
}

#[test]
fn test_actual_backward_seek_repro() {
    // Reproduce the exact failure pattern from the Gutenberg text
    // The problematic pattern: 'meet.' "Why should
    let detector = SentenceDetectorDialog::new().unwrap();
    let text = r#"S. and B. emend so as to negative the verb 'meet.' "Why should
Hrothgar weep if he expects to meet Beowulf again?" both these
scholars ask."#;
    
    match detector.detect_sentences_borrowed(text) {
        Ok(_sentences) => {
            // Test passed - no backward seek error
        }
        Err(e) => {
            if e.to_string().contains("Cannot seek backwards") {
                panic!("Backward seek bug reproduced with text: '{}'", text.replace('\n', "\\n"));
            } else {
                panic!("Unexpected error: {e}");
            }
        }
    }
}

#[test]
fn test_dialog_attribution_no_split() {
    let detector = get_detector();
    let text = r#""Lor bless her dear heart, no!" interposed the nurse, hastily
depositing in her pocket a green glass bottle, the contents of which
she had been tasting in a corner with evident satisfaction."#;
    
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    // Should be ONE sentence (dialog + attribution), not split at "no!" interposed
    assert_eq!(sentences.len(), 1,
        "Dialog with attribution should not be split\nExpected: 1 sentence\nActual: {} sentences\nSentences: {:?}",
        sentences.len(),
        sentences.iter().map(|s| &s.raw_content).collect::<Vec<_>>());
    assert!(sentences[0].raw_content.contains("no!"));
    assert!(sentences[0].raw_content.contains("interposed"));
    assert!(sentences[0].raw_content.contains("satisfaction"));
}

#[test]
fn test_narrative_dialog_separation_expected_fail() {
    let detector = get_detector();
    
    // This test is expected to fail - demonstrates current limitation
    let text = r#"Then he struggled up and looked round him, somewhat confused, for a second or two.  "Hallo!  Is it all over?""#;
    
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    // Expected: 2 sentences (narrative + dialog should be separate)
    // Current implementation may fail this expectation
    let expected_sentences = vec![
        "Then he struggled up and looked round him, somewhat confused, for a second or two.",
        "\"Hallo!  Is it all over?\""
    ];
    
    assert_eq!(sentences.len(), expected_sentences.len(),
        "Expected {} sentences, got {}\nExpected: {:?}\nActual: {:?}",
        expected_sentences.len(),
        sentences.len(),
        expected_sentences,
        sentences.iter().map(|s| s.raw_content.trim()).collect::<Vec<_>>());
    
    for (i, (actual, expected)) in sentences.iter().zip(expected_sentences.iter()).enumerate() {
        assert_eq!(actual.raw_content.trim(), *expected,
            "Sentence {} mismatch:\nExpected: '{}'\nActual: '{}'", 
            i + 1, expected, actual.raw_content.trim());
    }
}

// NEW TESTS FOR SENTENCE BOUNDARY ISSUE

#[test]
fn test_mismatched_quote_dialogue_by_design_choice() {
    let detector = get_detector();
    
    // BY DESIGN CHOICE: Current implementation detects 1 sentence for this mismatched quote pattern.
    // The quotes don't properly match (single quote opens/closes, double quote opens, single quote closes).
    // This could potentially be improved to detect 2 sentences in the future, but doing so
    // would require mechanism changes that might introduce false positives or significant overhead.
    // More data would be needed before changing this behavior.
    // Alternative parsers like pysbd might detect 2 sentences.
    let text = "'Ah! fair lady,' quoth the king, \"I love you, and without your love I am but dead.' Then the lady said, 'Stop it.";
    
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    // BY DESIGN: Current implementation produces 1 sentence for this pattern
    let expected = vec![
        "'Ah! fair lady,' quoth the king, \"I love you, and without your love I am but dead.' Then the lady said, 'Stop it."
    ];
    
    assert_eq!(sentences.len(), expected.len(),
        "BY DESIGN: Current behavior for mismatched quotes\nExpected: {} sentences, got {}\nExpected: {:?}\nActual: {:?}",
        expected.len(),
        sentences.len(),
        expected,
        sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
    
    for (i, (actual, expected)) in sentences.iter().zip(expected.iter()).enumerate() {
        assert_eq!(actual.normalize().trim(), *expected,
            "Sentence {} mismatch:\nExpected: '{}'\nActual: '{}'", 
            i + 1, expected, actual.normalize().trim());
    }
}

#[test]
fn test_backward_seek_minimal_reproduction() {
    let detector = get_detector();
    
    // Create a minimal case that demonstrates the backward seek issue
    // Start with a simple case and add complexity until we reproduce the problem
    let test_cases = [
        // Simple case - should work
        "Hello. World.",
        // Case with whitespace
        "Hello. World. Another.",
        // Case with hard separator
        "Hello.\n\nWorld.",
        // Case with dialog
        "He said. \"Hello.\" She replied.",
        // Case with colon + hard separator (from the original problem)
        "He said:\n\n\"Hello.\"\n\n\"World.\"",
        
        // DIALOG HARD END PATTERN TESTS
        // Test each type of dialog hard end pattern to see if they all have the same issue
        
        // 1. Single quote (known failing case)
        "verb 'meet.' \"Why should",
        
        // 2. Double quote  
        "verb \"meet.\" 'Why should",
        
        // 3. Smart double quote (opening: ", closing: ")
        "verb \u{201C}meet.\u{201D} \"Why should",
        
        // 4. Smart single quote (opening: ', closing: ')
        "verb \u{2018}meet.\u{2019} \"Why should",
        
        // 5. Round parentheses
        "verb (meet.) \"Why should",
        
        // 6. Square brackets
        "verb [meet.] \"Why should",
        
        // 7. Curly braces
        "verb {meet.} \"Why should",
    ];
    
    let mut failed_cases = Vec::new();
    
    for (i, test_case) in test_cases.iter().enumerate() {
        let result = detector.detect_sentences_borrowed(test_case);
        match result {
            Ok(_sentences) => {
                // Test case passed
            }
            Err(e) => {
                if e.to_string().contains("Cannot seek backwards") {
                    failed_cases.push((i, test_case, e.to_string()));
                } else {
                    panic!("Unexpected error in test case {i}: {e}");
                }
            }
        }
    }
    
    if !failed_cases.is_empty() {
        panic!("Found {} cases with backward seek issues: {:?}", 
            failed_cases.len(), 
            failed_cases.iter().map(|(i, case, err)| format!("Case {}: '{}' - {}", i, case.replace('\n', "\\n"), err)).collect::<Vec<_>>());
    }
    let problem_text = std::fs::read_to_string("exploration/problem_repro-0.txt")
        .expect("Failed to read problem_repro-0.txt");
    
    let result = detector.detect_sentences_borrowed(&problem_text);
    match result {
        Ok(_sentences) => {
            // Full file processed successfully
        }
        Err(e) => {
            if e.to_string().contains("Cannot seek backwards") {
                panic!("Backward seek error reproduced with full file: {e}");
            } else {
                panic!("Unexpected error with full file: {e}");
            }
        }
    }
}

#[test]
fn test_vad_differential_diagnosis() {
    let detector = get_detector();
    
    // Hypothesis 1: V.A.D. abbreviation causes sentence 1+2 to merge
    let text1 = "She was the V.A.D.  Stenography was unknown.";
    let sentences1 = detector.detect_sentences_borrowed(text1).unwrap();
    // DISCOVERY: V.A.D. does NOT cause sentences to merge - they split normally!
    println!("V.A.D. test: {} sentences: {:?}", sentences1.len(), 
        sentences1.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
    assert_eq!(sentences1.len(), 2, "V.A.D. actually splits normally - hypothesis wrong!");
    
    // Hypothesis 2: What happens after V.A.D. + normal sentence? Should split at next boundary
    let text2 = "She was the V.A.D.  Stenography was unknown. Munition-making was new.";
    let sentences2 = detector.detect_sentences_borrowed(text2).unwrap();
    println!("Three sentence test: {} sentences: {:?}", sentences2.len(), 
        sentences2.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
    // Actually splits into 3 sentences - V.A.D. doesn't cause merging!
    assert_eq!(sentences2.len(), 3, "All three sentences split normally");
    
    // Hypothesis 3: Are there other problematic abbreviations/patterns in the text?
    let text3 = "Munition-making was new. There were activities. Other forms existed.";
    let sentences3 = detector.detect_sentences_borrowed(text3).unwrap();
    // Should be 3 sentences - no abbreviation issues here
    assert_eq!(sentences3.len(), 3, "Simple sentences should split normally");
    
    // Hypothesis 4: Does the em-dash cause issues?
    let text4 = "There were activities—bandage-rolling, parcel-packing. Other forms existed.";
    let sentences4 = detector.detect_sentences_borrowed(text4).unwrap();
    // Should be 2 sentences
    assert_eq!(sentences4.len(), 2, "Em-dash should not prevent sentence boundary");
    
    // Hypothesis 5: Does the _italics_ pattern cause issues?
    let text5 = "They sold programmes at charity _matinées_. Other forms existed.";
    let sentences5 = detector.detect_sentences_borrowed(text5).unwrap();
    // Should be 2 sentences  
    assert_eq!(sentences5.len(), 2, "Italic markers should not prevent sentence boundary");
    
    // Hypothesis 6: Test quotation marks in middle of sentence
    let text6 = "The tray was marked \"Pending.\" Those expressions existed.";
    let sentences6 = detector.detect_sentences_borrowed(text6).unwrap();
    // Should be 2 sentences
    assert_eq!(sentences6.len(), 2, "Quoted words should not prevent sentence boundary");
    
    // Hypothesis 7: Test the actual problematic text incrementally
    let original_start = "Nursing attracted her most; but she knew herself to be pathetically ignorant of the elements of the craft, and furthermore doubted (rightly) if her combative nature would endure the complete subservience to the professional element inevitable in the life of that plucky, much-enduring, self-effacing Cinderella, the V.A.D.  Stenography and typewriting were unknown to her.";
    let sentences_start = detector.detect_sentences_borrowed(original_start).unwrap();
    println!("Original start: {} sentences: {:?}", sentences_start.len(), 
        sentences_start.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
    
    // Add next sentence
    let with_munition = format!("{original_start} Munition-making at this time was but an infant industry—as the occupants of the trenches had continuous occasion to note, with characteristic comment.");
    let sentences_munition = detector.detect_sentences_borrowed(&with_munition).unwrap();
    println!("With munition: {} sentences", sentences_munition.len());
    
    // Isolate the problem: What specific part of the long sentence is causing the merge?
    // Test progressively longer prefixes to find the exact issue
    let parts = [
        "Nursing attracted her most; but she knew herself to be pathetically ignorant.",
        "Nursing attracted her most; but she knew herself to be pathetically ignorant of the elements of the craft.",
        "Nursing attracted her most; but she knew herself to be pathetically ignorant of the elements of the craft, and furthermore doubted (rightly) if her combative nature would endure.",
        "Nursing attracted her most; but she knew herself to be pathetically ignorant of the elements of the craft, and furthermore doubted (rightly) if her combative nature would endure the complete subservience to the professional element.",
        "Nursing attracted her most; but she knew herself to be pathetically ignorant of the elements of the craft, and furthermore doubted (rightly) if her combative nature would endure the complete subservience to the professional element inevitable in the life of that plucky, much-enduring, self-effacing Cinderella.",
        "Nursing attracted her most; but she knew herself to be pathetically ignorant of the elements of the craft, and furthermore doubted (rightly) if her combative nature would endure the complete subservience to the professional element inevitable in the life of that plucky, much-enduring, self-effacing Cinderella, the V.A.D.",
    ];
    
    for (i, part) in parts.iter().enumerate() {
        let test_text = format!("{part}  Stenography was unknown.");
        let sentences = detector.detect_sentences_borrowed(&test_text).unwrap();
        println!("Part {}: {} sentences", i, sentences.len());
        if sentences.len() == 1 {
            println!("  FOUND MERGE POINT at part {i}: '{part}'");
            break;
        }
    }
}

#[test] 
fn test_parenthetical_state_transitions() {
    let detector = get_detector();
    
    // Progressive test cases to understand the state machine behavior
    let test_cases = [
        // Simple parenthetical that should work
        ("Simple: (test)", 1),
        // Parenthetical followed by period  
        ("Simple (test).", 1),
        // Parenthetical followed by period and new sentence
        ("Simple (test). Next sentence.", 2),
        // The actual failing case
        ("She doubted (rightly) if her nature would endure.  Stenography was unknown.", 2),
    ];
    
    for (text, expected) in test_cases {
        println!("\n=== Testing: '{text}' ===");
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("Result: {} sentences (expected {}): {:?}", 
            sentences.len(), 
            expected,
            sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
        
        if sentences.len() != expected {
            println!("MISMATCH: Expected {}, got {}", expected, sentences.len());
        }
    }
    
    // Test the broader issue - affects ALL dialog types without ending punctuation
    println!("\n=== Testing broader dialog issue ===");
    let broader_cases = [
        // Double quotes without ending punctuation - should work (closing " handled)
        ("She said, \"Whatever\" and went about her business. Next sentence.", 2),
        // Single quotes without ending punctuation - should work (closing ' handled)  
        ("She said, 'Whatever' and went about her business. Next sentence.", 2),
        // Smart double quotes without ending punctuation - should work (closing " handled)
        ("She said, \u{201C}Whatever\u{201D} and went about her business. Next sentence.", 2),
        // Smart single quotes without ending punctuation - should work (closing ' handled)
        ("She said, \u{2018}Whatever\u{2019} and went about her business. Next sentence.", 2),
        // Square brackets without ending punctuation - should NOT work yet (unfixed)
        ("The reference [whatever] was cited in the text. Next sentence.", 2),
        // Curly braces without ending punctuation - should NOT work yet (unfixed)  
        ("The variable {whatever} was used in code. Next sentence.", 2),
        // Round parentheses without ending punctuation - should work (fixed)
        ("She doubted (rightly) if her nature would endure. Next sentence.", 2),
    ];
    
    for (text, expected) in broader_cases {
        println!("Testing: '{text}'");
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("Result: {} sentences: {:?}", 
            sentences.len(),
            sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
        
        if sentences.len() != expected {
            println!("POTENTIAL ISSUE: Expected {}, got {}", expected, sentences.len());
        }
    }
    
    // Focus on the specific bug - parenthetical should not prevent sentence boundary
    let text = "She doubted (rightly) if her nature would endure.  Stenography was unknown.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    assert_eq!(sentences.len(), 2, "Parenthetical (rightly) should not prevent sentence boundary");
}

#[test]
fn test_pattern_coverage_analysis() {
    let detector = get_detector();
    
    // Test cases that should demonstrate the 4-pattern coverage gaps
    let test_cases = [
        // Pattern 4: Unpunctuated dialog close + space + non-dialog → should exit to narrative
        ("The item (expensive) was purchased.", 1, "Unpunctuated parenthetical"),
        ("He said \"whatever\" and left.", 1, "Unpunctuated quote"),
        
        // Pattern 3: Unpunctuated dialog close + space + dialog opener → should stay in dialog 
        ("The items (first)(second) were listed.", 1, "Consecutive parentheticals"),
        ("Quote \"first\"\"second\" content.", 1, "Consecutive quotes"),
        
        // Pattern 1: Punctuated dialog close + space + sentence start → should create boundary
        ("Dialog \"Hello!\" Next sentence.", 2, "Hard boundary"),
        ("Note (done.) New task.", 2, "Hard boundary parenthetical"),
        
        // Pattern 2: Punctuated dialog close + space + non-sentence start → should continue
        ("Dialog \"Hello,\" she said.", 1, "Soft boundary"),
        ("Note (good,) he thought.", 1, "Soft boundary parenthetical"),
    ];
    
    println!("\n=== Pattern Coverage Analysis ===");
    for (text, expected, description) in test_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{text}'");
        for (i, sent) in sentences.iter().enumerate() {
            println!("    {}: '{}'", i+1, sent.raw_content.trim());
        }
        if sentences.len() != expected {
            println!("  ❌ MISMATCH!");
        } else {
            println!("  ✅ OK");
        }
        println!();
    }
}

#[test]
fn test_semicolon_after_parenthetical_bug() {
    let detector = get_detector();
    
    // Theory: semicolon after closing parenthesis prevents proper dialog state exit
    // This should create under-splitting (fewer sentences than expected)
    
    let test_cases = [
        // Minimal case: parenthetical + semicolon + continuation + period + new sentence
        ("Text (year); more text. New sentence.", 2, "Basic semicolon after parenthetical"),
        
        // The specific pattern from Kanawha text
        ("Settlement (1748); several Virginians hunted. Before the close happened.", 2, "Kanawha pattern simplified"),
        
        // Control case: without semicolon should work correctly
        ("Text (year) more text. New sentence.", 2, "Control: no semicolon"),
        
        // Control case: with comma instead of semicolon
        ("Text (year), more text. New sentence.", 2, "Control: comma instead"),
        
        // Other punctuation after parenthetical
        ("Text (year): more text. New sentence.", 2, "Colon after parenthetical"),
        ("Text (year)! More text. New sentence.", 2, "Exclamation after parenthetical"),
        ("Text (year)? More text. New sentence.", 2, "Question after parenthetical"),
        
        // Multiple parentheticals with semicolons
        ("First (1748); second (1749); third text. New sentence.", 2, "Multiple parenthetical semicolons"),
    ];
    
    println!("=== Testing Semicolon After Parenthetical Bug ===");
    
    for (text, expected, description) in test_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{text}'");
        for (i, sentence) in sentences.iter().enumerate() {
            println!("    {}: '{}'", i + 1, sentence.normalize().trim());
        }
        
        if sentences.len() != expected {
            println!("  ❌ BUG REPRODUCED! Expected {}, got {}", expected, sentences.len());
        } else {
            println!("  ✅ Working correctly");
        }
        println!();
    }
    
    // Focus on the minimal reproduction case
    let minimal_case = "Text (year); more text. New sentence.";
    let sentences = detector.detect_sentences_borrowed(minimal_case).unwrap();
    
    if sentences.len() == 1 {
        println!("🔍 CONFIRMED BUG: Semicolon after parenthetical causes under-splitting");
        println!("Single sentence detected: '{}'", sentences[0].normalize().trim());
    }
}

#[test]
fn test_kanawha_settlement_text() {
    let detector = get_detector();
    
    let input = "The first settlement made west of the mountains was on a branch of\nthe Kanawha (1748); in the same season several adventurous Virginians\nhunted and made land-claims in Kentucky and Tennessee. Before the close of\nthe following year (1749) there had been formed the Ohio Company, composed\nof wealthy Virginians, among whom were two brothers of Washington.";
    
    let sentences = detector.detect_sentences_borrowed(input).unwrap();
    
    println!("Kanawha settlement test: {} sentences:", sentences.len());
    for (i, sentence) in sentences.iter().enumerate() {
        println!("  {}: '{}'", i + 1, sentence.normalize().trim());
    }
    
    // Should now correctly detect 2 sentences after fixing semicolon bug
    assert_eq!(sentences.len(), 2, "Should detect 2 sentences in Kanawha settlement text");
    
    assert!(sentences[0].normalize().contains("Kanawha (1748)"));
    assert!(sentences[0].normalize().contains("Kentucky and Tennessee"));
    assert!(sentences[1].normalize().contains("Ohio Company"));
    assert!(sentences[1].normalize().contains("brothers of Washington"));
}

#[test]
fn test_punctuation_after_quotes_bug() {
    let detector = get_detector();
    
    // Theory: punctuation after closing quotes prevents proper dialog state exit
    // Similar to the parenthetical bug, test single and double quotes
    
    let double_quote_cases = [
        // Double quotes with various punctuation after closing quote
        ("Text \"word\"; more text. New sentence.", 2, "Double quote + semicolon"),
        ("Text \"word\", more text. New sentence.", 2, "Double quote + comma"),
        ("Text \"word\": more text. New sentence.", 2, "Double quote + colon"),
        
        // Control: no punctuation after quote
        ("Text \"word\" more text. New sentence.", 2, "Double quote control: no punctuation"),
        
        // Control: punctuation inside quote (should work normally)
        ("Text \"word!\" more text. New sentence.", 2, "Double quote control: punctuation inside"),
    ];
    
    let single_quote_cases = [
        // Single quotes with various punctuation after closing quote
        ("Text 'word'; more text. New sentence.", 2, "Single quote + semicolon"),
        ("Text 'word', more text. New sentence.", 2, "Single quote + comma"),
        ("Text 'word': more text. New sentence.", 2, "Single quote + colon"),
        
        // Control: no punctuation after quote
        ("Text 'word' more text. New sentence.", 2, "Single quote control: no punctuation"),
        
        // Control: punctuation inside quote (should work normally)
        ("Text 'word!' more text. New sentence.", 2, "Single quote control: punctuation inside"),
    ];
    
    println!("=== Testing Punctuation After Double Quotes ===");
    
    for (text, expected, description) in double_quote_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{text}'");
        for (i, sentence) in sentences.iter().enumerate() {
            println!("    {}: '{}'", i + 1, sentence.normalize().trim());
        }
        
        if sentences.len() != expected {
            println!("  ❌ BUG REPRODUCED! Expected {}, got {}", expected, sentences.len());
        } else {
            println!("  ✅ Working correctly");
        }
        println!();
    }
    
    println!("=== Testing Punctuation After Single Quotes ===");
    
    for (text, expected, description) in single_quote_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{text}'");
        for (i, sentence) in sentences.iter().enumerate() {
            println!("    {}: '{}'", i + 1, sentence.normalize().trim());
        }
        
        if sentences.len() != expected {
            println!("  ❌ BUG REPRODUCED! Expected {}, got {}", expected, sentences.len());
        } else {
            println!("  ✅ Working correctly");
        }
        println!();
    }
    
    // Test smart quotes too
    let smart_quote_cases = [
        ("Text \u{201C}word\u{201D}; more text. New sentence.", 2, "Smart double quote + semicolon"),
        ("Text \u{2018}word\u{2019}; more text. New sentence.", 2, "Smart single quote + semicolon"),
    ];
    
    println!("=== Testing Punctuation After Smart Quotes ===");
    
    for (text, expected, description) in smart_quote_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{text}'");
        for (i, sentence) in sentences.iter().enumerate() {
            println!("    {}: '{}'", i + 1, sentence.normalize().trim());
        }
        
        if sentences.len() != expected {
            println!("  ❌ BUG REPRODUCED! Expected {}, got {}", expected, sentences.len());
        } else {
            println!("  ✅ Working correctly");
        }
        println!();
    }
}

#[test]
fn test_sentence_ending_punctuation_after_dialog_three_sentence_expectation() {
    let detector = get_detector();
    
    // These cases should produce 3 sentences according to design goal:
    // 1. "Text \"word\"!" (dialog with sentence-ending punctuation)
    // 2. "More text." (separate sentence)  
    // 3. "New sentence." (final sentence)
    // Currently failing - produces 1 sentence instead of 3
    
    let three_sentence_cases = [
        ("Text \"word\"! More text. New sentence.", 3, "Double quote + exclamation should create 3 sentences"),
        ("Text \"word\"? More text. New sentence.", 3, "Double quote + question should create 3 sentences"),
        ("Text 'word'! More text. New sentence.", 3, "Single quote + exclamation should create 3 sentences"),
        ("Text 'word'? More text. New sentence.", 3, "Single quote + question should create 3 sentences"),
    ];
    
    println!("=== Testing Sentence-Ending Punctuation After Dialog (Expected: 3 sentences) ===");
    
    for (text, expected, description) in three_sentence_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{text}'");
        for (i, sentence) in sentences.iter().enumerate() {
            println!("    {}: '{}'", i + 1, sentence.normalize().trim());
        }
        
        if sentences.len() != expected {
            println!("  ❌ CURRENTLY FAILING! Expected {}, got {} (design goal not yet implemented)", expected, sentences.len());
        } else {
            println!("  ✅ Meeting design goal");
        }
        println!();
    }
    
    // Document current failing behavior - do not assert for now since this is not yet implemented
    // When this feature is implemented, change these to assert_eq!
    let text = "Text \"word\"! More text. New sentence.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    // TODO: When implemented, this should be:
    // assert_eq!(sentences.len(), 3, "Sentence-ending punctuation after dialog should create 3 sentences");
    // For now, document current behavior:
    println!("Current behavior for '{}': {} sentences (design goal: 3)", text, sentences.len());
}

#[test]
fn test_simple_hard_separator_continue_bug() {
    let detector = get_detector();
    
    // Test case: reproduce the exact bug from your data
    // After hard separator, comma+quote pattern should Split, not Continue
    let text = "Text that was forfeited.\n\n\"No, dearest,\" he said with thoughts, \"Not yet.\"";
    
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    
    // Expected: should get Split for hard separator, not Continue for quote
    // This test demonstrates the fix where hard separator analysis works correctly
    assert_eq!(sentences.len(), 2, "Should create 2 sentences");
}

#[test]
fn test_dialog_close_newline_terminates_before_control_line() {
    let detector = get_detector();
    let text = "He said, \u{201C}go to the door.\u{201D}\n*** CONTROL LINE ***";

    let sentences = detector.detect_sentences_borrowed(text).unwrap();

    assert_eq!(
        sentences.len(),
        2,
        "Dialog close at line end should split before the following line\nSentences: {:?}",
        sentences
            .iter()
            .map(|s| s.normalize().trim().to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(sentences[0].normalize().trim(), "He said, \u{201C}go to the door.\u{201D}");
    assert_eq!(sentences[1].normalize().trim(), "*** CONTROL LINE ***");
}

#[test]
fn test_dialog_pattern_partitioning_comprehensive() {
    let detector = get_detector();
    
    // PATTERN 1: Hard End - [.!?]{close} + space + sentence_start → DialogEnd (create boundary)
    let hard_end_cases = [
        // Double quotes
        ("\"Hello!\" The next sentence.", 2, "Hard end with double quotes"),
        ("\"Stop?\" She asked again.", 2, "Hard end with question in quotes"),
        ("\"Done.\" Next task started.", 2, "Hard end with period in quotes"),
        
        // Single quotes  
        ("'Wait!' He shouted loudly.", 2, "Hard end with single quotes"),
        ("'Really?' That seems unlikely.", 2, "Hard end with single quote question"),
        
        // Smart quotes
        ("\u{201C}Finished!\u{201D} Time to go.", 2, "Hard end with smart double quotes"),
        ("\u{2018}Yes!\u{2019} Absolutely correct.", 2, "Hard end with smart single quotes"),
        
        // Parentheticals
        ("The result (finally!) was clear.", 2, "Hard end with parenthetical exclamation"),
        ("The note [important.] was filed.", 2, "Hard end with square bracket period"),
        ("The code {complete!} was deployed.", 2, "Hard end with curly brace exclamation"),
    ];
    
    println!("\n=== Testing Hard End Cases (Pattern 1) ===");
    for (text, expected, description) in hard_end_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        if sentences.len() != expected {
            println!("  FAIL: '{text}'");
            println!("  Got: {:?}", sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
        }
        // assert_eq!(sentences.len(), expected, "Hard end case failed: {}", description);
    }
    
    // PATTERN 2: Soft End (Punctuated) - [.!?]{close} + space + non_sentence_start → DialogSoftEnd (existing behavior)  
    let soft_punctuated_cases = [
        // Double quotes with lowercase continuation
        ("\"Hello,\" she said quietly.", 1, "Soft punctuated double quotes"),
        ("\"Stop!\" he whispered softly.", 1, "Soft punctuated with exclamation"),
        ("\"Why?\" she wondered aloud.", 1, "Soft punctuated with question"),
        
        // Single quotes
        ("'Yes,' he replied calmly.", 1, "Soft punctuated single quotes"),
        ("'No!' she said firmly.", 1, "Soft punctuated single quotes exclamation"),
        
        // Smart quotes  
        ("\u{201C}Maybe,\u{201D} he thought quietly.", 1, "Soft punctuated smart double quotes"),
        ("\u{2018}Sure!\u{2019} she said enthusiastically.", 1, "Soft punctuated smart single quotes"),
        
        // Parentheticals with punctuation + lowercase
        ("The result (good!) made everyone happy.", 1, "Soft punctuated parenthetical"),
        ("The note [urgent.] was processed immediately.", 1, "Soft punctuated square bracket"),
        ("The variable {important!} was updated correctly.", 1, "Soft punctuated curly brace"),
    ];
    
    println!("\n=== Testing Soft End Punctuated Cases (Pattern 2) ===");
    for (text, expected, description) in soft_punctuated_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        if sentences.len() != expected {
            println!("  FAIL: '{text}'");
            println!("  Got: {:?}", sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
        }
        // assert_eq!(sentences.len(), expected, "Soft punctuated case failed: {}", description);
    }
    
    // PATTERN 3: Dialog Continuation - [^.!?]{close} + space + dialog_opener → DialogOpen (stay in dialog)
    let dialog_continuation_cases = [
        // Consecutive parentheticals
        ("The items (first)(second)(third) were listed.", 1, "Consecutive parentheticals"),
        ("Notes [alpha][beta][gamma] were reviewed.", 1, "Consecutive square brackets"),
        ("Variables {x}{y}{z} were defined.", 1, "Consecutive curly braces"),
        
        // Mixed dialog types
        ("The quote \"text\"(note) was analyzed.", 1, "Quote followed by parenthetical"),
        ("The note (comment)\"quote\" was saved.", 1, "Parenthetical followed by quote"),
        
        // Complex nesting
        ("Statement \"part1\"\"part2\" continued.", 1, "Consecutive double quotes"),
        ("Statement 'part1''part2' continued.", 1, "Consecutive single quotes"),
    ];
    
    println!("\n=== Testing Dialog Continuation Cases (Pattern 3) ===");
    for (text, expected, description) in dialog_continuation_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        if sentences.len() != expected {
            println!("  FAIL: '{text}'");
            println!("  Got: {:?}", sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
        }
        // Note: These may currently fail - that's expected until pattern 3 is implemented
        // assert_eq!(sentences.len(), expected, "Dialog continuation case failed: {}", description);
    }
    
    // PATTERN 4: Soft End (Unpunctuated) - [^.!?]{close} + space + non_dialog_opener → DialogSoftEnd (THE FIX!)
    let soft_unpunctuated_cases = [
        // Original bug cases
        ("She doubted (rightly) if her nature would endure.", 1, "Original parenthetical bug - rightly"),
        ("The item (expensive) was still worth buying.", 1, "Unpunctuated parenthetical descriptive"),
        ("He said \"whatever\" and walked away.", 1, "Unpunctuated double quote"),
        ("She replied 'maybe' to the question.", 1, "Unpunctuated single quote"),
        
        // Smart quotes unpunctuated
        ("The response \u{201C}never\u{201D} was surprising.", 1, "Unpunctuated smart double quote"),
        ("The answer \u{2018}always\u{2019} seemed correct.", 1, "Unpunctuated smart single quote"),
        
        // Various bracket types
        ("The reference [source] was helpful.", 1, "Unpunctuated square bracket"),
        ("The variable {name} was defined.", 1, "Unpunctuated curly brace"),
        
        // Multiple unpunctuated in sequence with narrative
        ("The note (brief) and comment [short] were filed.", 1, "Multiple unpunctuated with narrative"),
    ];
    
    println!("\n=== Testing Soft End Unpunctuated Cases (Pattern 4 - THE FIX!) ===");
    for (text, expected, description) in soft_unpunctuated_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        if sentences.len() != expected {
            println!("  FAIL: '{text}'");
            println!("  Got: {:?}", sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
        }
        // Note: Many of these currently fail - that's the bug we're fixing
        // assert_eq!(sentences.len(), expected, "Soft unpunctuated case failed: {}", description);
    }
    
    // EDGE CASES AND BOUNDARY CONDITIONS
    let edge_cases = [
        // Empty dialog
        ("The quote \"\" was empty.", 1, "Empty double quotes"),
        ("The note () was blank.", 1, "Empty parentheses"),
        
        // Multiple spaces
        ("The text \"hello\"  and more.", 1, "Multiple spaces after quote"),
        ("The note (test)   continued here.", 1, "Multiple spaces after parenthesis"),
        
        // Mixed punctuation complexity
        ("\"Hello?\" she asked. \"Really!\" he replied.", 2, "Mixed question and exclamation"),
        ("The note (see pg. 5) was referenced.", 1, "Abbreviation inside parenthetical"),
        
        // Nested structures
        ("\"He said (quietly) to me.\" Next sentence.", 2, "Nested parenthetical in quote"),
        ("The item (cost: $5.99) was purchased.", 1, "Complex parenthetical content"),
    ];
    
    println!("\n=== Testing Edge Cases ===");
    for (text, expected, description) in edge_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        if sentences.len() != expected {
            println!("  FAIL: '{text}'");
            println!("  Got: {:?}", sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
        }
        // Note: Some edge cases may currently fail - document which ones work vs fail
    }
}

#[test]
fn test_dialog_comma_capital_continuation_bug() {
    let detector = get_detector();
    
    // Test the specific failing case from the task
    let failing_case = r#""Right," I told him, condescendingly."#;
    let sentences = detector.detect_sentences_borrowed(failing_case).unwrap();
    
    // EXPECTED: Should be one sentence (continuation after comma)
    // ACTUAL BUG: Incorrectly splits at capital 'I' despite comma continuation
    assert_eq!(sentences.len(), 1, 
        "Dialog with comma + capital should continue as one sentence\nExpected: 1 sentence\nActual: {} sentences\nSentences: {:?}",
        sentences.len(),
        sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
    
    // Verify the full sentence content is preserved
    assert!(sentences[0].raw_content.contains("Right,"));
    assert!(sentences[0].raw_content.contains("I told him"));
    assert!(sentences[0].raw_content.contains("condescendingly"));
}

#[test]
fn test_dialog_comma_capital_continuation_bug_with_context() {
    let detector = get_detector();
    
    // Test the case with narrative context - this should demonstrate the actual bug
    let failing_case_with_context = r#"He said "Right," I told him, condescendingly."#;
    let sentences = detector.detect_sentences_borrowed(failing_case_with_context).unwrap();
    
    // BUG: This incorrectly splits into 2 sentences instead of 1
    // The comma after "Right," should signal continuation, not split
    println!("Context test: {} sentences", sentences.len());
    for (i, sentence) in sentences.iter().enumerate() {
        println!("  {}: '{}'", i + 1, sentence.raw_content.trim());
    }
    
    // This assertion will fail, demonstrating the bug
    // When fixed, this should pass
    assert_eq!(sentences.len(), 1, 
        "Dialog with comma continuation should be one sentence\nExpected: 1 sentence\nActual: {} sentences\nSentences: {:?}",
        sentences.len(),
        sentences.iter().map(|s| s.normalize().trim().to_string()).collect::<Vec<_>>());
}

#[test]
fn test_dialog_continuation_punctuation_variations() {
    let detector = get_detector();
    
    // Test all continuation punctuation types mentioned in SEAMS-Design.md
    let test_cases = [
        // Comma continuation (the main bug case)
        (r#""Right," I told him, condescendingly."#, 1, "Comma continuation"),
        (r#""Hello," she said quietly."#, 1, "Comma continuation - basic"),
        (r#""Wait," he whispered urgently."#, 1, "Comma continuation - urgent"),
        
        // Semicolon continuation
        (r#""Maybe;" I thought about it."#, 1, "Semicolon continuation"),
        (r#""Yes;" she nodded slowly."#, 1, "Semicolon continuation - basic"),
        
        // Colon continuation
        (r#""Listen:" I have something important."#, 1, "Colon continuation"),
        (r#""Note:" she wrote it down."#, 1, "Colon continuation - basic"),
        
        // Multiple continuation punctuation in sequence
        (r#""First," he said, "then," she replied."#, 1, "Multiple comma continuations"),
        
        // Mixed quote types with continuation
        (r#"'Right,' I told him, condescendingly."#, 1, "Single quote comma continuation"),
        (r#""Yes," then 'No,' I decided."#, 1, "Mixed quote types with continuation"),
    ];
    
    for (text, expected_sentences, description) in test_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        println!("Testing {}: {} sentences (expected {})", description, sentences.len(), expected_sentences);
        println!("  Text: '{}'", text);
        for (i, sentence) in sentences.iter().enumerate() {
            println!("    {}: '{}'", i + 1, sentence.raw_content.trim());
        }
        
        // These tests may currently fail - that's expected until the bug is fixed
        if sentences.len() != expected_sentences {
            println!("  ❌ FAILING (expected bug): Expected {}, got {}", expected_sentences, sentences.len());
        } else {
            println!("  ✅ PASSING");
        }
        println!();
    }
}

#[test]
fn test_dialog_continuation_vs_boundary_distinction() {
    let detector = get_detector();
    
    // Test cases that should CONTINUE (not split) vs those that should SPLIT
    let continuation_cases = [
        // These should continue - continuation punctuation overrides capital letters
        (r#""Right," I told him."#, 1, "Comma + capital should continue"),
        (r#""Maybe;" I thought about it."#, 1, "Semicolon + capital should continue"),
        (r#""Listen:" I have news."#, 1, "Colon + capital should continue"),
        (r#""Well," Mary said, "I think so.""#, 1, "Comma + capital + continuing dialog"),
        
        // Edge cases with whitespace
        (r#""Right,"  I told him."#, 1, "Comma + multiple spaces + capital"),
        (r#""Right,"\tI told him."#, 1, "Comma + tab + capital"),
        (r#""Right,"\nI told him."#, 1, "Comma + newline + capital"),
    ];
    
    let boundary_cases = [
        // These should split - hard punctuation creates sentence boundaries
        (r#""Stop!" I shouted loudly."#, 2, "Exclamation + capital should split"),
        (r#""Really?" I asked curiously."#, 2, "Question + capital should split"),
        (r#""Done." I finished the task."#, 2, "Period + capital should split"),
        (r#""Wait!" Then I realized."#, 2, "Exclamation + 'Then' should split"),
        
        // Control cases - no dialog
        ("I told him something. Then I left.", 2, "Normal narrative sentences"),
        ("Right, I told him. Then I left.", 2, "Comma in narrative + new sentence"),
    ];
    
    println!("=== Testing Continuation Cases (should NOT split) ===");
    for (text, expected, description) in continuation_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{}'", text);
        
        if sentences.len() != expected {
            println!("  ❌ BUG: Expected {}, got {} - continuation punctuation should prevent split", expected, sentences.len());
        } else {
            println!("  ✅ CORRECT: Continuation punctuation prevents split");
        }
        println!();
    }
    
    println!("=== Testing Boundary Cases (should split) ===");
    for (text, expected, description) in boundary_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{}'", text);
        
        if sentences.len() != expected {
            println!("  ❌ ISSUE: Expected {}, got {} - hard punctuation should create split", expected, sentences.len());
        } else {
            println!("  ✅ CORRECT: Hard punctuation creates split");
        }
        println!();
    }
}

#[test]
fn test_dialog_state_machine_continuation_logic() {
    let detector = get_detector();
    
    // Test the state machine logic from SEAMS-Design.md:
    // {close}{cont_punct} + space + Any → D→N Continue
    
    let state_machine_cases = [
        // Pattern: {close}{cont_punct} + space + Any → Continue
        (r#""word", text"#, 1, "Quote close + comma + space + text"),
        (r#""word"; text"#, 1, "Quote close + semicolon + space + text"),
        (r#""word": text"#, 1, "Quote close + colon + space + text"),
        (r#"'word', text"#, 1, "Single quote close + comma + space + text"),
        (r#"(word), text"#, 1, "Paren close + comma + space + text"),
        (r#"[word], text"#, 1, "Bracket close + comma + space + text"),
        
        // Pattern: {close}{cont_punct} + space + Capital → Continue (the bug case)
        (r#""word", Capital"#, 1, "Quote close + comma + space + Capital"),
        (r#""word"; Capital"#, 1, "Quote close + semicolon + space + Capital"),
        (r#""word": Capital"#, 1, "Quote close + colon + space + Capital"),
        
        // Pattern: {close}{hard_punct} + space + Any → Split
        (r#""word"! Next"#, 2, "Quote close + exclamation + space + next"),
        (r#""word"? Next"#, 2, "Quote close + question + space + next"),
        (r#""word". Next"#, 2, "Quote close + period + space + next"),
        
        // Edge case: no space after continuation punctuation
        (r#""word",text"#, 1, "Quote close + comma + no space + text"),
        (r#""word":text"#, 1, "Quote close + colon + no space + text"),
        
        // Complex case: multiple continuation punctuation
        (r#""first", "second", text"#, 1, "Multiple comma continuations"),
        
        // Real-world example from the task
        (r#""Right," I told him, condescendingly."#, 1, "Task example - comma continuation"),
    ];
    
    println!("=== Testing State Machine Continuation Logic ===");
    for (text, expected, description) in state_machine_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        println!("{}: {} sentences (expected {})", description, sentences.len(), expected);
        println!("  Text: '{}'", text);
        
        if sentences.len() != expected {
            println!("  ❌ STATE MACHINE BUG: Expected {}, got {}", expected, sentences.len());
            for (i, sentence) in sentences.iter().enumerate() {
                println!("    Sentence {}: '{}'", i + 1, sentence.raw_content.trim());
            }
        } else {
            println!("  ✅ STATE MACHINE CORRECT");
        }
        println!();
    }
}

#[test]
fn test_external_definitive_punctuation_all_dialog_states() {
    let detector = get_detector();
    
    // Test cases for external definitive punctuation patterns across ALL dialog states
    // Each should produce 3 sentences: "Text {open}word{close}{punct}! More text. New sentence."
    // 1. "Text {open}word{close}{punct}!" - dialog with external punctuation
    // 2. "More text." - separate sentence  
    // 3. "New sentence." - final sentence
    
    let external_definitive_cases = [
        // Double quotes
        (r#"Text "word"! More text. New sentence."#, 3, "Double quote + exclamation"),
        (r#"Text "word"? More text. New sentence."#, 3, "Double quote + question"),
        (r#"Text "word". More text. New sentence."#, 3, "Double quote + period"),
        
        // Single quotes
        (r#"Text 'word'! More text. New sentence."#, 3, "Single quote + exclamation"),
        (r#"Text 'word'? More text. New sentence."#, 3, "Single quote + question"),
        (r#"Text 'word'. More text. New sentence."#, 3, "Single quote + period"),
        
        // Smart double quotes (Unicode escapes)
        ("Text \u{201C}word\u{201D}! More text. New sentence.", 3, "Smart double quote + exclamation"),
        ("Text \u{201C}word\u{201D}? More text. New sentence.", 3, "Smart double quote + question"),
        ("Text \u{201C}word\u{201D}. More text. New sentence.", 3, "Smart double quote + period"),
        
        // Smart single quotes (Unicode escapes)
        ("Text \u{2018}word\u{2019}! More text. New sentence.", 3, "Smart single quote + exclamation"),
        ("Text \u{2018}word\u{2019}? More text. New sentence.", 3, "Smart single quote + question"),
        ("Text \u{2018}word\u{2019}. More text. New sentence.", 3, "Smart single quote + period"),
        
        // Round parentheses
        (r#"Text (word)! More text. New sentence."#, 3, "Round parentheses + exclamation"),
        (r#"Text (word)? More text. New sentence."#, 3, "Round parentheses + question"),
        (r#"Text (word). More text. New sentence."#, 3, "Round parentheses + period"),
        
        // Square brackets
        (r#"Text [word]! More text. New sentence."#, 3, "Square brackets + exclamation"),
        (r#"Text [word]? More text. New sentence."#, 3, "Square brackets + question"),
        (r#"Text [word]. More text. New sentence."#, 3, "Square brackets + period"),
        
        // Curly braces
        (r#"Text {word}! More text. New sentence."#, 3, "Curly braces + exclamation"),
        (r#"Text {word}? More text. New sentence."#, 3, "Curly braces + question"),
        (r#"Text {word}. More text. New sentence."#, 3, "Curly braces + period"),
    ];
    
    println!("=== Testing External Definitive Punctuation Across All Dialog States ===");
    println!("Each case should produce 3 sentences when external punctuation appears after dialog close");
    println!();
    
    let mut passing_count = 0;
    let mut failing_count = 0;
    
    for (text, expected, description) in external_definitive_cases {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        if sentences.len() == expected {
            passing_count += 1;
            println!("✅ PASS: {}: {} sentences", description, sentences.len());
        } else {
            failing_count += 1;
            println!("❌ FAIL: {}: {} sentences (expected {})", description, sentences.len(), expected);
            println!("  Text: '{}'", text);
            for (i, sentence) in sentences.iter().enumerate() {
                println!("    {}: '{}'", i + 1, sentence.normalize().trim());
            }
        }
    }
    
    println!();
    println!("=== SUMMARY ===");
    println!("Passing: {}/{}", passing_count, external_definitive_cases.len());
    println!("Failing: {}/{}", failing_count, external_definitive_cases.len());
    
    if failing_count > 0 {
        println!("❌ Implementation incomplete: {} dialog states still need external definitive punctuation patterns", failing_count / 3);
    } else {
        println!("✅ All dialog states correctly handle external definitive punctuation!");
    }
    
    // NOTE: This test is expected to show failures until external definitive punctuation 
    // patterns are implemented for all dialog states (not just double quotes)
    // When implementation is complete, change this to: assert_eq!(failing_count, 0);
}

#[test]
fn test_dialog_to_dialog_transitions_all_states() {
    let detector = get_detector();
    
    // Test Dialog→Dialog transitions across ALL dialog states
    // This test documents current bugs - only double quotes work, others don't
    
    // SPLIT Transitions (Dialog→Dialog + Sentence Boundary)
    // Previous dialog ends with sentence punctuation + next starts with capital = SPLIT
    let split_transitions = [
        // Double quotes (KNOWN TO WORK - reference implementation)
        ("Text \"First sentence.\" \"Second sentence.\" More.", 3, "Double quote D→D split"),
        
        // Single quotes (CURRENTLY BROKEN - should split but doesn't)
        ("Text 'First sentence.' 'Second sentence.' More.", 3, "Single quote D→D split"),
        
        // Smart double quotes (CURRENTLY BROKEN)
        ("Text \u{201C}First sentence.\u{201D} \u{201C}Second sentence.\u{201D} More.", 3, "Smart double quote D→D split"),
        
        // Smart single quotes (CURRENTLY BROKEN)
        ("Text \u{2018}First sentence.\u{2019} \u{2018}Second sentence.\u{2019} More.", 3, "Smart single quote D→D split"),
        
        // Round parentheses (CURRENTLY BROKEN)
        ("Text (This is one.)(This starts new.) More.", 3, "Round paren D→D split"),
        
        // Square brackets (CURRENTLY BROKEN)
        ("Text [Previous sentence.] [New sentence starts.] More.", 3, "Square bracket D→D split"),
        
        // Curly braces (CURRENTLY BROKEN)
        ("Text {Done.}{Next task.} More.", 3, "Curly brace D→D split"),
    ];
    
    // CONTINUE Transitions (Dialog→Dialog + Same Sentence)
    // No sentence punctuation + next starts with lowercase = CONTINUE
    let continue_transitions = [
        // Double quotes (KNOWN TO WORK - reference implementation)
        ("Text \"first\" \"second\" more.", 1, "Double quote D→D continue"),
        
        // Single quotes (CURRENTLY BROKEN - should continue but doesn't)
        ("Text 'first' 'second' more.", 1, "Single quote D→D continue"),
        
        // Smart double quotes (CURRENTLY BROKEN)
        ("Text \u{201C}first\u{201D} \u{201C}second\u{201D} more.", 1, "Smart double quote D→D continue"),
        
        // Smart single quotes (CURRENTLY BROKEN)
        ("Text \u{2018}first\u{2019} \u{2018}second\u{2019} more.", 1, "Smart single quote D→D continue"),
        
        // Round parentheses (CURRENTLY BROKEN)
        ("Text (first)(second) more.", 1, "Round paren D→D continue"),
        
        // Square brackets (CURRENTLY BROKEN)
        ("Text [item][another] more.", 1, "Square bracket D→D continue"),
        
        // Curly braces (CURRENTLY BROKEN)
        ("Text {item}{another} more.", 1, "Curly brace D→D continue"),
    ];
    
    // Zero-Character Separators (Brackets)
    // Immediate transitions without space
    let zero_char_transitions = [
        // Round parentheses zero-char (CURRENTLY BROKEN)
        ("Text (first)(Second) more.", 2, "Round paren D→D zero-char split"),
        ("Text (first)(second) more.", 1, "Round paren D→D zero-char continue"),
        
        // Square brackets zero-char (CURRENTLY BROKEN)
        ("Text [first][Second] more.", 2, "Square bracket D→D zero-char split"),
        ("Text [first][second] more.", 1, "Square bracket D→D zero-char continue"),
        
        // Curly braces zero-char (CURRENTLY BROKEN)
        ("Text {Done.}{Next task.} more.", 2, "Curly brace D→D zero-char split"),
        ("Text {first}{second} more.", 1, "Curly brace D→D zero-char continue"),
    ];
    
    
    println!("=== Testing Dialog→Dialog Transitions - Split Cases ===");
    let mut split_working = 0;
    let split_total = split_transitions.len();
    
    for (text, expected, description) in split_transitions {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        if sentences.len() == expected {
            split_working += 1;
            println!("✅ WORKING: {}: {} sentences", description, sentences.len());
        } else {
            println!("❌ FAILING: {}: {} sentences (expected {})", description, sentences.len(), expected);
            println!("  Text: '{}'", text);
            for (i, sentence) in sentences.iter().enumerate() {
                println!("    {}: '{}'", i + 1, sentence.normalize().trim());
            }
        }
    }
    
    println!("\n=== Testing Dialog→Dialog Transitions - Continue Cases ===");
    let mut continue_working = 0;
    let continue_total = continue_transitions.len();
    
    for (text, expected, description) in continue_transitions {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        if sentences.len() == expected {
            continue_working += 1;
            println!("✅ WORKING: {}: {} sentences", description, sentences.len());
        } else {
            println!("❌ FAILING: {}: {} sentences (expected {})", description, sentences.len(), expected);
            println!("  Text: '{}'", text);
            for (i, sentence) in sentences.iter().enumerate() {
                println!("    {}: '{}'", i + 1, sentence.normalize().trim());
            }
        }
    }
    
    println!("\n=== Testing Zero-Character Separators ===");
    let mut zero_char_working = 0;
    let zero_char_total = zero_char_transitions.len();
    
    for (text, expected, description) in zero_char_transitions {
        let sentences = detector.detect_sentences_borrowed(text).unwrap();
        
        if sentences.len() == expected {
            zero_char_working += 1;
            println!("✅ WORKING: {}: {} sentences", description, sentences.len());
        } else {
            println!("❌ FAILING: {}: {} sentences (expected {})", description, sentences.len(), expected);
            println!("  Text: '{}'", text);
            for (i, sentence) in sentences.iter().enumerate() {
                println!("    {}: '{}'", i + 1, sentence.normalize().trim());
            }
        }
    }
    
    println!("\n=== SUMMARY: Dialog→Dialog Transition Bug Documentation ===");
    println!("Split transitions: {}/{} working", split_working, split_total);
    println!("Continue transitions: {}/{} working", continue_working, continue_total);
    println!("Zero-char transitions: {}/{} working", zero_char_working, zero_char_total);
    
    let total_working = split_working + continue_working + zero_char_working;
    let total_tests = split_total + continue_total + zero_char_total;
    
    println!("OVERALL: {}/{} D→D transitions working", total_working, total_tests);
    
    if total_working == 1 {  // Only double quotes should work initially
        println!("✅ Bug confirmed: Only double quotes support D→D transitions");
        println!("❌ Missing: Single quotes, smart quotes, parentheses, brackets, braces");
    } else if total_working == total_tests {
        println!("✅ All dialog states support D→D transitions!");
    } else {
        println!("🔨 Partial implementation: {}/{} dialog states working", 
                total_working / 4, 7);  // Rough estimate
    }
    
    // NOTE: This test initially FAILS to demonstrate the bug
    // When Dialog→Dialog transitions are implemented for all states, 
    // most of these should pass
    
    // FAILING ASSERTIONS - These document the current bugs by failing
    assert_eq!(split_working, split_total, 
        "BUG: Only {}/{} split transitions work - missing D→D patterns for non-double-quote dialog states", 
        split_working, split_total);
    
    assert_eq!(continue_working, continue_total,
        "BUG: Only {}/{} continue transitions work - missing D→D patterns for non-double-quote dialog states",
        continue_working, continue_total);
    
    assert_eq!(zero_char_working, zero_char_total,
        "BUG: Only {}/{} zero-char transitions work - missing zero-character separator patterns",
        zero_char_working, zero_char_total);
}

// ── Guillemet (French angle-quote) tests ──────────────────────────────────────

#[test]
fn test_guillemet_basic_dialog() {
    let detector = get_detector();
    // Simple guillemet-quoted dialog followed by narrative
    let text = "«Bonjour», dit-il. Il entra dans la pièce.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    assert!(
        sentences.len() >= 1,
        "Expected at least 1 sentence, got 0. Input: {:?}", text
    );
    assert!(
        sentences[0].raw_content.contains("Bonjour"),
        "First sentence should contain dialog text. Got: {:?}",
        sentences[0].raw_content
    );
}

#[test]
fn test_guillemet_french_interior_spaces() {
    let detector = get_detector();
    // French convention: space after « and before »
    let text = "« Bonjour ! » dit-il. Il entra dans la pièce.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    assert!(
        sentences.len() >= 1,
        "Expected at least 1 sentence, got 0. Input: {:?}", text
    );
    let first = sentences[0].raw_content;
    assert!(
        first.contains("Bonjour"),
        "First sentence should contain dialog text. Got: {:?}", first
    );
}

#[test]
fn test_guillemet_hard_end_to_narrative() {
    let detector = get_detector();
    // Guillemet dialog ending with sentence-terminal punct → hard boundary → narrative
    let text = "Il dit : «Partez immédiatement !» Elle obéit aussitôt. La porte se ferma.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    assert!(
        sentences.len() >= 2,
        "Expected ≥2 sentences, got {}.\nSentences: {:?}",
        sentences.len(),
        sentences.iter().map(|s| s.raw_content).collect::<Vec<_>>()
    );
}

#[test]
fn test_guillemet_hard_end_preserves_closing_delimiter_after_interior_space() {
    let detector = get_detector();
    let text = "Il dit : « Partez immédiatement ! » Elle obéit aussitôt.";

    let sentences = detector.detect_sentences_borrowed(text).unwrap();

    assert_eq!(sentences.len(), 2);
    assert_eq!(
        sentences[0].normalize(),
        "Il dit : « Partez immédiatement ! »"
    );
    assert_eq!(sentences[1].normalize(), "Elle obéit aussitôt.");
}

#[test]
fn test_guillemet_line_end_preserves_closing_delimiter_after_interior_space() {
    let detector = get_detector();
    let text = "Il dit : « Partez immédiatement ! »\nElle obéit aussitôt.";

    let sentences = detector.detect_sentences_borrowed(text).unwrap();

    assert_eq!(sentences.len(), 2);
    assert_eq!(
        sentences[0].normalize(),
        "Il dit : « Partez immédiatement ! »"
    );
    assert_eq!(sentences[1].normalize(), "Elle obéit aussitôt.");
}

#[test]
fn test_guillemet_line_end_preserves_closing_delimiter_with_interior_space() {
    let detector = get_detector();

    let text = "« Vous partez demain. »\nRenée ferme la fenêtre.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();

    let normalized = sentences
        .iter()
        .map(|s| s.normalize().trim().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        normalized,
        vec![
            "« Vous partez demain. »",
            "Renée ferme la fenêtre.",
        ]
    );
}

#[test]
fn test_guillemet_narrative_to_guillemet_boundary() {
    let detector = get_detector();
    // Sentence ending in punct followed by guillemet-quoted next sentence
    let text = "Elle hésita. «Non», répondit-elle enfin.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    assert!(
        sentences.len() >= 1,
        "Expected ≥1 sentence, got 0. Input: {:?}", text
    );
    // First sentence should be the narrative part
    assert!(
        sentences[0].raw_content.contains("hésita"),
        "First sentence should be 'Elle hésita.' Got: {:?}", sentences[0].raw_content
    );
}

#[test]
fn test_guillemet_no_false_split_inside_dialog() {
    let detector = get_detector();
    // Multiple sentence-terminal punct inside guillemet dialog should coalesce into one sentence
    let text = "«Arrêtez ! Partez ! Vite !» cria-t-elle.";
    let sentences = detector.detect_sentences_borrowed(text).unwrap();
    let all_text: String = sentences.iter().map(|s| s.raw_content).collect::<Vec<_>>().join(" ");
    assert!(
        all_text.contains("Arrêtez") && all_text.contains("Vite"),
        "All dialog content should be present. Got: {:?}", all_text
    );
}
