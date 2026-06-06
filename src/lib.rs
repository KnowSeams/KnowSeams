pub mod discovery;
pub mod sentence_detector;
pub mod incremental;
pub mod parallel_processing;
pub mod restart_log;

// Re-export main types for convenient access
pub use sentence_detector::{
    DetectedSentenceBorrowed,
    Span
};

// Re-export primary entry point and debug output type for library consumers
pub use sentence_detector::dialog_detector::{
    SentenceDetectorDialog,
    DebugTransitionInfo,
};

// Re-export incremental processing utilities
pub use incremental::{
    generate_aux_file_path,
    create_complete_aux_file
};

// Re-export parallel processing types and functions for benchmarking
pub use parallel_processing::{
    FileStats
};

// Re-export restart log for external use
pub use restart_log::{
    RestartLog, should_process_file as should_process_file_restart
};