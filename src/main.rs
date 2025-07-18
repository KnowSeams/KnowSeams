use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::info;

mod discovery;
mod sentence_detector;
mod incremental;
mod parallel_processing;
mod restart_log;

use crate::incremental::create_complete_aux_file;
use crate::parallel_processing::FileStats;
use crate::restart_log::{RestartLog, should_process_file};
use futures::stream::StreamExt;
use futures::future::join_all;
use std::sync::Arc;
use std::collections::VecDeque;

// Types and implementations moved to parallel_processing module

/// Aggregate run statistics
/// WHY: Provides overall metrics for the entire run per PRD F-8 requirements
#[derive(Serialize, Deserialize, Debug)]
struct RunStats {
    /// Timestamp when run started
    run_start: String,
    /// Total processing time in milliseconds
    total_processing_time_ms: u64,
    /// Total sentence detection time in milliseconds (subset of total_processing_time_ms)
    total_sentence_detection_time_ms: u64,
    /// Total characters processed across all files
    total_chars_processed: u64,
    /// Total sentences detected across all files
    total_sentences_detected: u64,
    /// Aggregated sentence length statistics across all files
    aggregate_sentence_length_stats: Option<crate::parallel_processing::SentenceLengthStats>,
    /// Overall throughput in characters per second
    overall_chars_per_sec: f64,
    /// Sentence detection throughput in characters per second
    sentence_detection_chars_per_sec: f64,
    /// Number of files successfully processed
    files_processed: u64,
    /// Number of files skipped (already complete)
    files_skipped: u64,
    /// Number of files that failed processing
    files_failed: u64,
    /// Per-file statistics
    file_stats: Vec<FileStats>,
}

/// Process files using overlapped discovery and processing pipeline
/// WHY: Overlaps file discovery with processing to reduce total pipeline time
async fn process_with_overlapped_pipeline(
    root_dir: &std::path::Path,
    args: &Args,
    restart_log: &mut RestartLog,
) -> Result<(u64, u64, u64, u64, u64, Vec<FileStats>, std::time::Duration)> {
    let pipeline_start = std::time::Instant::now();
    
    // Always perform fresh discovery - parallel discovery is fast enough
    let discovery_config = discovery::DiscoveryConfig {
        fail_fast: args.fail_fast,
        max_threads: args.max_cpus,
    };
    
    // Perform overlapped discovery and processing
    info!("Starting overlapped file discovery and processing in: {}", root_dir.display());
    
    let fail_fast = args.fail_fast;
    let quiet = args.quiet;
    let calculate_length_stats = args.sentence_length_stats;
    let mut discovery_stream = Box::pin(discovery::discover_files_parallel(root_dir, discovery_config));
    let mut discovered_files = Vec::new();
    let mut valid_files = Vec::new();
    let mut invalid_files = Vec::new();
    let mut processing_queue = VecDeque::new();
    let mut processing_results = Vec::new();
    
    // WHY: Use bounded concurrency to prevent resource exhaustion
    let max_concurrent = if let Some(max_cpus) = args.max_cpus {
        max_cpus.max(1) // Ensure at least 1 CPU
    } else {
        (num_cpus::get().saturating_sub(1)).max(1) // Leave one core free for system tasks
    };
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let detector = Arc::new(
        crate::sentence_detector::dialog_detector::SentenceDetectorDialog::new()
            .map_err(|e| anyhow::anyhow!("Failed to initialize dialog sentence detector: {}", e))?
    );
    
    // Process files as they're discovered
    while let Some(discovery_result) = discovery_stream.next().await {
        match discovery_result {
            Ok(file_validation) => {
                discovered_files.push(file_validation.clone());
                
                if file_validation.error.is_none() {
                    valid_files.push(file_validation.clone());
                    
                    // Start processing this file immediately
                    let semaphore_clone = semaphore.clone();
                    let detector_clone = detector.clone();
                    let path = file_validation.path.clone();
                    let overwrite_all = args.overwrite_all;
                    let debug_seams = args.debug_seams;
                    
                    // Check if file should be processed using restart log
                    let should_process = should_process_file(&path, restart_log, overwrite_all).await.unwrap_or(true);
                    
                    if should_process {
                        let task = tokio::spawn(async move {
                            let _permit = semaphore_clone.acquire().await.unwrap();
                            process_single_file_restart(
                                &path,
                                &detector_clone,
                                overwrite_all,
                                quiet,
                                calculate_length_stats,
                                debug_seams,
                            ).await
                        });
                        
                        processing_queue.push_back(task);
                    } else {
                        // File is already processed, create a skip result
                        let skip_result = Ok((0u64, 0u64, 0u64, true, FileStats {
                            path: path.to_string_lossy().to_string(),
                            chars_processed: 0,
                            sentences_detected: 0,
                            sentence_length_stats: None,
                            processing_time_ms: 0,
                            sentence_detection_time_ms: 0,
                            chars_per_sec: 0.0,
                            status: "skipped".to_string(),
                            error: None,
                        }));
                        processing_results.push(skip_result);
                    }
                } else {
                    invalid_files.push(file_validation.clone());
                    let validation_failure_stats = FileStats {
                        path: file_validation.path.to_string_lossy().to_string(),
                        chars_processed: 0,
                        sentences_detected: 0,
                        sentence_length_stats: None,
                        processing_time_ms: 0,
                        sentence_detection_time_ms: 0,
                        chars_per_sec: 0.0,
                        status: "failed_validation".to_string(),
                        error: file_validation.error,
                    };
                    processing_results.push(Ok((0, 0, 0, false, validation_failure_stats)));
                }
            }
            Err(e) => {
                if fail_fast {
                    return Err(e);
                }
                info!("Discovery error (continuing): {}", e);
            }
        }
        
        // Check for completed processing tasks
        while let Some(task) = processing_queue.front() {
            if task.is_finished() {
                let task = processing_queue.pop_front().unwrap();
                match task.await {
                    Ok(result) => processing_results.push(result),
                    Err(e) => {
                        info!("Processing task error: {}", e);
                        if fail_fast {
                            return Err(anyhow::anyhow!("Processing task failed: {}", e));
                        }
                    }
                }
            } else {
                break;
            }
        }
    }
    
    // Wait for remaining processing tasks to complete
    let remaining_tasks: Vec<_> = processing_queue.into_iter().collect();
    let remaining_results = join_all(remaining_tasks).await;
    
    for result in remaining_results {
        match result {
            Ok(processing_result) => processing_results.push(processing_result),
            Err(e) => {
                info!("Remaining processing task error: {}", e);
                if fail_fast {
                    return Err(anyhow::anyhow!("Processing task failed: {}", e));
                }
            }
        }
    }
    
    // Process results and update restart log
    let mut total_sentences = 0u64;
    let mut total_bytes = 0u64;
    let mut processed_files = 0u64;
    let mut skipped_files = 0u64;
    let mut failed_files = 0u64;
    let mut file_stats = Vec::new();
    
    for result in processing_results {
        match result {
            Ok((sentences, bytes, files, skipped, stats)) => {
                total_sentences += sentences;
                total_bytes += bytes;
                if skipped {
                    skipped_files += 1;
                } else {
                    processed_files += files;
                    // Mark file as completed in restart log
                    let path = std::path::PathBuf::from(&stats.path);
                    restart_log.mark_completed(&path);
                }
                file_stats.push(stats);
            }
            Err(e) => {
                failed_files += 1;
                info!("Processing error: {}", e);
                
                let failed_stats = FileStats {
                    path: "unknown".to_string(),
                    chars_processed: 0,
                    sentences_detected: 0,
                    sentence_length_stats: None,
                    processing_time_ms: 0,
                    sentence_detection_time_ms: 0,
                    chars_per_sec: 0.0,
                    status: "failed".to_string(),
                    error: Some(e.to_string()),
                };
                file_stats.push(failed_stats);
            }
        }
    }
    
    // Save restart log
    if let Err(e) = restart_log.save(root_dir).await {
        info!("Warning: Failed to save restart log: {}", e);
    } else {
        info!("Saved restart log to {}", root_dir.join(".seams_restart.json").display());
    }
    
    let processing_duration = pipeline_start.elapsed();
    
    // Log and display results
    info!("File discovery completed: {} total files found", discovered_files.len());
    info!("Valid UTF-8 files: {}", valid_files.len());
    
    if !invalid_files.is_empty() {
        info!("Files with issues: {}", invalid_files.len());
        for file in &invalid_files {
            if let Some(ref error) = file.error {
                info!("Issue with {}: {}", file.path.display(), error);
            }
        }
    }
    
    if !quiet {
        println!("seams v{} - Overlapped discovery and processing complete", env!("CARGO_PKG_VERSION"));
        println!("Found {} files matching pattern *-0.txt", discovered_files.len());
        println!("Valid files: {}, Files with issues: {}", valid_files.len(), invalid_files.len());
        println!("Restart log: {} files tracked as completed", restart_log.completed_count());
        
        // WHY: Provide helpful guidance when no files are found  
        if discovered_files.is_empty() {
            println!("\nNo *-0.txt files found in directory tree.");
            println!("SUGGESTIONS:");
            println!("• Verify your directory contains Project Gutenberg files ending in '-0.txt'");
            println!("• Check subdirectories - seams searches recursively");
            println!("• Example valid filenames: 'pg1234-0.txt', 'alice-0.txt'");
            println!("• Try: find {} -name '*-0.txt' -type f", root_dir.display());
        }
    }
    
    // WHY: Demonstrate public API usage for external developers (minimal example)
    if std::env::var("SEAMS_DEBUG_API").is_ok() && !valid_files.is_empty() {
        let example_path = &valid_files[0].path;
        let demo_content = "0\tExample usage of public API.\t(1,1,1,27)\n";
        if create_complete_aux_file(example_path, demo_content).is_ok() {
            info!("Created demo aux file using public API for {}", example_path.display());
        }
    }
    
    if !quiet && !valid_files.is_empty() {
        println!("File processing complete:");
        println!("  Successfully processed: {processed_files} files");
        if skipped_files > 0 {
            println!("  Skipped (complete aux files): {skipped_files} files");
        }
        if failed_files > 0 {
            println!("  Failed to process: {failed_files} files");
        }
        println!("  Total bytes processed: {total_bytes}");
        println!("  Total sentences detected: {total_sentences}");
        
        // Display sentence length statistics if available and requested
        if args.sentence_length_stats {
            if let Some(ref length_stats) = crate::parallel_processing::calculate_aggregate_sentence_length_stats(&file_stats) {
                println!("  Sentence length statistics:");
                println!("    Min: {} chars, Max: {} chars", length_stats.min_length, length_stats.max_length);
                println!("    Mean: {:.1} chars, Median: {:.1} chars", length_stats.mean_length, length_stats.median_length);
                println!("    P25: {:.1}, P75: {:.1}, P90: {:.1} chars", length_stats.p25_length, length_stats.p75_length, length_stats.p90_length);
                println!("    Std Dev: {:.1} chars", length_stats.std_dev);
            }
        }
        
        println!("  Total time spent: {:.2}s", processing_duration.as_secs_f64());
        
        // WHY: Show performance metrics - Total characters / Total time spent
        if total_bytes > 0 && processing_duration.as_secs_f64() > 0.0 {
            let throughput_chars_per_sec = total_bytes as f64 / processing_duration.as_secs_f64();
            let throughput_mb_per_sec = throughput_chars_per_sec / 1_000_000.0;
            println!("  Overall throughput: {throughput_chars_per_sec:.0} chars/sec ({throughput_mb_per_sec:.2} MB/s)");
            
            // WHY: Show sentence detection throughput for just the detection algorithm
            let total_sentence_detection_time_ms: u64 = file_stats.iter()
                .map(|fs| fs.sentence_detection_time_ms)
                .sum();
            
            if total_sentence_detection_time_ms > 0 && total_sentences > 0 {
                let sentence_detection_time_sec = total_sentence_detection_time_ms as f64 / 1000.0;
                let sentence_detection_throughput_chars_per_sec = total_bytes as f64 / sentence_detection_time_sec;
                let sentence_detection_throughput_mb_per_sec = sentence_detection_throughput_chars_per_sec / 1_000_000.0;
                println!("  Sentence detection throughput: {sentence_detection_throughput_chars_per_sec:.0} chars/sec ({sentence_detection_throughput_mb_per_sec:.2} MB/s)");
                println!("  Sentence detection time: {sentence_detection_time_sec:.2}s of {:.2}s total ({:.1}%)", 
                         processing_duration.as_secs_f64(), 
                         (sentence_detection_time_sec / processing_duration.as_secs_f64()) * 100.0);
            }
        }
    }
        
    info!("Overlapped pipeline completed: {} processed, {} skipped, {} failed, {} sentences detected", 
          processed_files, skipped_files, failed_files, total_sentences);
    
    Ok((total_sentences, total_bytes, processed_files, skipped_files, failed_files, file_stats, processing_duration))
}

/// Process a single file - simplified version without cache dependencies
async fn process_single_file_restart(
    path: &std::path::Path,
    detector: &crate::sentence_detector::dialog_detector::SentenceDetectorDialog,
    _overwrite_all: bool,
    quiet: bool,
    calculate_length_stats: bool,
    debug_seams: bool,
) -> Result<(u64, u64, u64, bool, FileStats)> {
    let start_time = std::time::Instant::now();
    
    // Process the file directly
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!(
            "Cannot read file: {}\nError: {}\n\nSUGGESTIONS:\n• Check file permissions (should be readable)\n• Ensure file is not locked by another process\n• Verify disk space is available",
            path.display(), e
        ))?;
    let mmap = unsafe { memmap2::MmapOptions::new().map(&file) }
        .map_err(|e| anyhow::anyhow!(
            "Cannot memory-map file: {}\nError: {}\n\nSUGGESTIONS:\n• File may be too large for available memory\n• Check file is not corrupted\n• Ensure sufficient virtual memory is available",
            path.display(), e
        ))?;
    let content = std::str::from_utf8(&mmap)
        .map_err(|_| anyhow::anyhow!(
            "File contains invalid UTF-8: {}\n\nSUGGESTIONS:\n• Ensure file is a text file, not binary\n• Check file encoding - Project Gutenberg files should be UTF-8\n• File may be corrupted during download",
            path.display()
        ))?;

    // WHY: Measure sentence detection time separately from total processing time
    let sentence_detection_start = std::time::Instant::now();
    let (sentences, debug_info) = if debug_seams {
        // Use debug-enabled detection when debug mode is active
        detector.detect_sentences_borrowed_with_debug(content)
            .map_err(|e| anyhow::anyhow!(
                "Sentence detection failed for: {}\nError: {}\n\nSUGGESTIONS:\n• File may contain unusual text patterns that confuse the detector\n• Ensure file contains valid Project Gutenberg text format\n• Check file is not corrupted or truncated",
                path.display(), e
            ))?
    } else {
        // Use standard detection for performance when debug is disabled
        let sentences = detector.detect_sentences_borrowed(content)
            .map_err(|e| anyhow::anyhow!(
                "Sentence detection failed for: {}\nError: {}\n\nSUGGESTIONS:\n• File may contain unusual text patterns that confuse the detector\n• Ensure file contains valid Project Gutenberg text format\n• Check file is not corrupted or truncated",
                path.display(), e
            ))?;
        (sentences, Vec::new())
    };
    let sentence_detection_time = sentence_detection_start.elapsed();
    
    let sentence_count = sentences.len() as u64;
    let byte_count = content.len() as u64;
    let char_count = content.chars().count() as u64;
    
    // Generate auxiliary file
    let aux_path = crate::incremental::generate_aux_file_path(path);
    let debug_info_opt = if debug_seams && !debug_info.is_empty() {
        Some(debug_info.as_slice())
    } else {
        None
    };
    crate::parallel_processing::write_auxiliary_file_borrowed(&aux_path, &sentences, detector, debug_seams, debug_info_opt).await?;
    
    // Calculate sentence length statistics only if requested
    let sentence_length_stats = if calculate_length_stats {
        crate::parallel_processing::calculate_sentence_length_stats(&sentences)
    } else {
        None
    };
    
    let processing_time = start_time.elapsed();
    let processing_time_ms = processing_time.as_millis() as u64;
    let chars_per_sec = if processing_time.as_secs_f64() > 0.0 {
        char_count as f64 / processing_time.as_secs_f64()
    } else {
        0.0
    };
    
    let file_stats = FileStats {
        path: path.to_string_lossy().to_string(),
        chars_processed: char_count,
        sentences_detected: sentence_count,
        sentence_length_stats,
        processing_time_ms,
        sentence_detection_time_ms: sentence_detection_time.as_millis() as u64,
        chars_per_sec,
        status: "success".to_string(),
        error: None,
    };
    
    //info!("Processed {}: {} sentences, {} chars", path.display(), sentence_count, char_count);
    let detection_ms = sentence_detection_time.as_millis();
    if !quiet {
        println!(
            "[Processed {}: {} sentences, {} bytes, detection {} ms",
            path.display(),
            sentence_count,
            byte_count,
            detection_ms
        );
    }
    Ok((sentence_count, byte_count, 1u64, false, file_stats))
}

/// Process a single file when given a file path instead of directory
async fn process_single_file_mode(
    file_path: &std::path::Path,
    args: &Args,
) -> Result<()> {
    // Validate file exists and matches pattern
    if !file_path.exists() {
        anyhow::bail!(
            "File not found: {}\n\nSUGGESTIONS:\n• Check the file path spelling\n• Ensure you have read permissions for the file\n• Use an absolute path if using relative paths",
            file_path.display()
        );
    }
    
    if !file_path.is_file() {
        anyhow::bail!(
            "Path exists but is not a file: {}\n\nSUGGESTIONS:\n• Provide a file path, not a directory path\n• Use seams with a directory to process multiple files\n• Example: seams /path/to/file-0.txt",
            file_path.display()
        );
    }
    
    // Check if it matches the *-0.txt pattern
    let file_name = file_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    
    if !file_name.ends_with("-0.txt") {
        anyhow::bail!(
            "File does not match expected pattern: {}\n\nSUGGESTIONS:\n• Seams processes Project Gutenberg files ending in '-0.txt'\n• Example valid filenames: 'pg1234-0.txt', 'alice-0.txt'\n• If this is the correct file, rename it to match the pattern",
            file_path.display()
        );
    }
    
    if !args.quiet {
        println!("seams v{} - Processing single file", env!("CARGO_PKG_VERSION"));
        println!("File: {}", file_path.display());
    }
    
    let start_time = std::time::Instant::now();
    
    // Initialize detector
    let detector = crate::sentence_detector::dialog_detector::SentenceDetectorDialog::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize dialog sentence detector: {}", e))?;
    
    // Process the single file
    let result = process_single_file_restart(
        file_path,
        &detector,
        args.overwrite_all,
        args.quiet,
        args.sentence_length_stats,
        args.debug_seams,
    ).await;
    
    match result {
        Ok((sentence_count, byte_count, _, _, file_stats)) => {
            let processing_duration = start_time.elapsed();
            
            if !args.quiet {
                println!("Single file processing complete:");
                println!("  Successfully processed: 1 file");
                println!("  Total bytes processed: {byte_count}");
                println!("  Total sentences detected: {sentence_count}");
                
                // Display sentence length statistics if available and requested
                if args.sentence_length_stats {
                    if let Some(ref length_stats) = file_stats.sentence_length_stats {
                        println!("  Sentence length statistics:");
                        println!("    Min: {} chars, Max: {} chars", length_stats.min_length, length_stats.max_length);
                        println!("    Mean: {:.1} chars, Median: {:.1} chars", length_stats.mean_length, length_stats.median_length);
                        println!("    P25: {:.1}, P75: {:.1}, P90: {:.1} chars", length_stats.p25_length, length_stats.p75_length, length_stats.p90_length);
                        println!("    Std Dev: {:.1} chars", length_stats.std_dev);
                    }
                }
                
                println!("  Total time spent: {:.2}s", processing_duration.as_secs_f64());
                
                if byte_count > 0 && processing_duration.as_secs_f64() > 0.0 {
                    let throughput_chars_per_sec = byte_count as f64 / processing_duration.as_secs_f64();
                    let throughput_mb_per_sec = throughput_chars_per_sec / 1_000_000.0;
                    println!("  Overall throughput: {throughput_chars_per_sec:.0} chars/sec ({throughput_mb_per_sec:.2} MB/s)");
                    
                    // Show sentence detection throughput
                    if file_stats.sentence_detection_time_ms > 0 {
                        let sentence_detection_time_sec = file_stats.sentence_detection_time_ms as f64 / 1000.0;
                        let sentence_detection_throughput_chars_per_sec = byte_count as f64 / sentence_detection_time_sec;
                        let sentence_detection_throughput_mb_per_sec = sentence_detection_throughput_chars_per_sec / 1_000_000.0;
                        println!("  Sentence detection throughput: {sentence_detection_throughput_chars_per_sec:.0} chars/sec ({sentence_detection_throughput_mb_per_sec:.2} MB/s)");
                    }
                }
            }
            
            // Generate stats for single file
            let overall_chars_per_sec = if processing_duration.as_secs_f64() > 0.0 {
                byte_count as f64 / processing_duration.as_secs_f64()
            } else {
                0.0
            };
            
            let sentence_detection_chars_per_sec = if file_stats.sentence_detection_time_ms > 0 {
                byte_count as f64 / (file_stats.sentence_detection_time_ms as f64 / 1000.0)
            } else {
                0.0
            };
            
            let file_stats_vec = vec![file_stats];
            let aggregate_sentence_length_stats = crate::parallel_processing::calculate_aggregate_sentence_length_stats(&file_stats_vec);
            
            let run_stats = RunStats {
                run_start: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs().to_string(),
                total_processing_time_ms: processing_duration.as_millis() as u64,
                total_sentence_detection_time_ms: file_stats_vec[0].sentence_detection_time_ms,
                total_chars_processed: byte_count,
                total_sentences_detected: sentence_count,
                aggregate_sentence_length_stats,
                overall_chars_per_sec,
                sentence_detection_chars_per_sec,
                files_processed: 1,
                files_skipped: 0,
                files_failed: 0,
                file_stats: file_stats_vec,
            };
            
            // Write stats to JSON file
            match serde_json::to_string_pretty(&run_stats) {
                Ok(json_content) => {
                    match tokio::fs::write(&args.stats_out, json_content).await {
                        Ok(()) => {
                            info!("Stats written to {}", args.stats_out.display());
                            if !args.quiet {
                                println!("Stats written to {}", args.stats_out.display());
                            }
                        }
                        Err(e) => {
                            info!("Warning: Failed to write stats file: {e}");
                            if !args.quiet {
                                println!("Warning: Failed to write stats file: {e}");
                            }
                        }
                    }
                }
                Err(e) => {
                    info!("Warning: Failed to serialize stats: {e}");
                    if !args.quiet {
                        println!("Warning: Failed to serialize stats: {e}");
                    }
                }
            }
            
            Ok(())
        }
        Err(e) => {
            if !args.quiet {
                println!("Failed to process file: {e}");
            }
            Err(e)
        }
    }
}

/// Write debug output to stdout in the same format as _seams-debug.txt files
/// WHY: Provides immediate debug feedback without temporary file creation
fn write_debug_output_to_stdout(
    sentences: &[crate::sentence_detector::DetectedSentenceBorrowed<'_>],
    debug_info: Option<&[crate::sentence_detector::dialog_detector::DebugTransitionInfo]>,
) {
    if let Some(debug_transitions) = debug_info {
        // Use real debug information when available
        for (sentence_idx, sentence) in sentences.iter().enumerate() {
            // Find debug transitions for this sentence
            let sentence_transitions: Vec<_> = debug_transitions.iter()
                .filter(|t| t.sentence_index == sentence_idx)
                .collect();
            
            if sentence_transitions.is_empty() {
                // No transitions found for this sentence - write placeholder
                println!(
                    "{}\t{}\t({},{},{},{})\tUnknown\tUnknown\tUnknown\tno_pattern\tno_pattern_name\tno_seam_text",
                    sentence.index,
                    sentence.normalize(),
                    sentence.span.start_line,
                    sentence.span.start_col,
                    sentence.span.end_line,
                    sentence.span.end_col, // seam_text
                );
            } else {
                // Write all transitions for this sentence
                for transition in sentence_transitions {
                    println!(
                        "{}\t{}\t({},{},{},{})\t{:?}\t{:?}\t{:?}\t{}\t{}\t{}",
                        sentence.index,
                        sentence.normalize(),
                        sentence.span.start_line,
                        sentence.span.start_col,
                        sentence.span.end_line,
                        sentence.span.end_col,
                        transition.state_before,
                        transition.state_after,
                        transition.transition_type,
                        transition.matched_pattern.replace('\n', "\\n").replace('\t', "\\t"), // Escape TSV special chars
                        transition.pattern_name,
                        transition.seam_text.replace('\n', "\\n").replace('\t', "\\t"), // Escape TSV special chars
                    );
                }
            }
        }
    } else {
        // Fallback to placeholder data when debug info is not available
        for sentence in sentences {
            println!(
                "{}\t{}\t({},{},{},{})\tplaceholder_state_before\tplaceholder_state_after\tplaceholder_transition_type\tplaceholder_pattern\tplaceholder_name\tplaceholder_seam",
                sentence.index,
                sentence.normalize(),
                sentence.span.start_line,
                sentence.span.start_col,
                sentence.span.end_line,
                sentence.span.end_col, // seam_text - placeholder
            );
        }
    }
}

/// Process text in debug mode and output results to stdout
/// WHY: Enables immediate debugging without creating temporary files
async fn process_debug_mode(text: &str) -> Result<()> {
    // Initialize detector
    let detector = crate::sentence_detector::dialog_detector::SentenceDetectorDialog::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize dialog sentence detector: {}", e))?;
    
    // Process with debug information
    let (sentences, debug_info) = detector.detect_sentences_borrowed_with_debug(text)
        .map_err(|e| anyhow::anyhow!("Sentence detection failed: {}", e))?;
    
    // Output debug information to stdout
    write_debug_output_to_stdout(&sentences, Some(&debug_info));
    
    Ok(())
}

// Cache-based discovery logic removed - using fresh parallel discovery only

// Functions moved to parallel_processing module

#[derive(Parser, Debug)]
#[command(name = "seams")]
#[command(about = "High-throughput sentence extractor for Project Gutenberg texts")]
#[command(long_about = "Seams is a high-performance CLI tool for extracting sentences from Project Gutenberg texts.

It recursively scans for *-0.txt files, detects sentence boundaries using a dialog-aware
sentence detector, and outputs normalized sentences with span metadata to _seams.txt files.

Designed for narrative analysis pipelines.

BASIC USAGE:
  seams /path/to/gutenberg/                    # Process all *-0.txt files in directory tree
  seams ~/Downloads/gutenberg-mirror/          # Process entire Gutenberg mirror
  seams /path/to/single-file-0.txt             # Process a single *-0.txt file

COMMON WORKFLOWS:
  seams ./texts --overwrite-all                # Reprocess all files (ignore existing outputs)
  seams ./texts --fail-fast                    # Stop immediately on any error
  seams ./texts --quiet                        # Minimal output for scripts/automation

PERFORMANCE & DEBUGGING:
  seams ./texts --stats-out benchmark.json     # Save detailed performance metrics
  seams ./texts --clear-restart-log            # Clear resume state, reprocess everything
  seams ./texts --max-cpus 1                   # Single-CPU benchmarking for baselines
  seams ./texts --max-cpus 4                   # Limit to 4 CPU cores

EXPECTED BEHAVIOR:
  • Finds files matching *-0.txt pattern recursively
  • Creates _seams.txt files alongside each input file  
  • Resumes interrupted runs automatically (use --overwrite-all to disable)
  • Outputs performance statistics and file counts

TROUBLESHOOTING:
  • No files found? Check your directory contains *-0.txt files
  • Permission errors? Ensure write access to input directory
  • Want to reprocess? Use --overwrite-all or --clear-restart-log")]
#[command(version)]
struct Args {
    /// Root directory to scan for *-0.txt files, or single file path
    #[arg(value_name = "PATH", help = "Directory to scan recursively for *-0.txt files, or single *-0.txt file to process")]
    root_dir: Option<PathBuf>,
    
    /// Overwrite even complete aux files
    #[arg(long, help = "Reprocess all files, even those with complete _seams.txt files")]
    overwrite_all: bool,
    
    
    /// Abort on first error
    #[arg(long, help = "Stop processing immediately on first I/O, UTF-8, or detection error")]
    fail_fast: bool,
    
    
    /// Quiet mode - minimal output for benchmarking
    #[arg(long, short = 'q', help = "Suppress all non-error output")]
    quiet: bool,
    
    /// Stats output file path
    #[arg(long, default_value = "run_stats.json", value_name = "FILE", help = "Write performance statistics to JSON file")]
    stats_out: PathBuf,
    
    /// Clear the restart log before processing
    #[arg(long, help = "Clear the restart log and reprocess all files")]
    clear_restart_log: bool,
    
    /// Maximum number of CPUs/threads to use for processing
    #[arg(long, help = "Limit processing to specified number of CPUs/threads (default: use all available)")]
    max_cpus: Option<usize>,
    
    /// Calculate sentence length statistics (adds processing overhead)
    #[arg(long, help = "Calculate and display sentence length statistics (min, max, mean, median, percentiles)")]
    sentence_length_stats: bool,

    /// Enable debug mode for SEAM detection (outputs _seams-debug.txt with state transitions)
    #[arg(long, help = "Generate debug TSV files with state transition and pattern match details")]
    debug_seams: bool,

    /// Debug text input directly from command line
    #[arg(long, help = "Debug sentence detection on provided text string (outputs to stdout)")]
    debug_text: Option<String>,

    /// Debug text input from standard input
    #[arg(long, help = "Debug sentence detection on text from stdin (outputs to stdout)")]
    debug_stdin: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // WHY: structured JSON logging enables observability and debugging in production
    tracing_subscriber::fmt()
        .with_target(false)
        .json()
        .init();
    
    let args = Args::parse();
    
    info!("Starting seams");
    info!(?args, "Parsed CLI arguments");

    // Handle debug mode flags early - these bypass normal file processing
    if args.debug_text.is_some() || args.debug_stdin {
        if args.debug_text.is_some() && args.debug_stdin {
            anyhow::bail!("Cannot use both --debug-text and --debug-stdin flags simultaneously");
        }
        
        let text = if let Some(debug_text) = &args.debug_text {
            debug_text.clone()
        } else {
            // Read from stdin
            use tokio::io::{AsyncReadExt, stdin};
            let mut buffer = String::new();
            stdin().read_to_string(&mut buffer).await
                .map_err(|e| anyhow::anyhow!("Failed to read from stdin: {}", e))?;
            buffer
        };
        
        return process_debug_mode(&text).await;
    }

    // Ensure root_dir is provided for normal processing
    let root_dir = args.root_dir.as_ref().ok_or_else(|| {
        anyhow::anyhow!("PATH argument is required when not using debug flags (--debug-text or --debug-stdin)")
    })?;
    
    // WHY: validate root directory exists early to fail fast with clear error
    if !root_dir.exists() {
        anyhow::bail!(
            "Directory not found: {}\n\nSUGGESTIONS:\n• Check the path spelling and try again\n• Ensure you have read permissions for the parent directory\n• Use an absolute path like '/home/user/gutenberg' if using relative paths",
            root_dir.display()
        );
    }
    
    // Check if input is a file or directory
    if root_dir.is_file() {
        // Single file mode
        return process_single_file_mode(root_dir, &args).await;
    }
    
    if !root_dir.is_dir() {
        anyhow::bail!(
            "Path exists but is neither a file nor a directory: {}\n\nSUGGESTIONS:\n• Provide a directory path to process multiple files\n• Provide a single *-0.txt file path to process one file\n• Example: seams /path/to/gutenberg-texts/ or seams /path/to/file-0.txt",
            root_dir.display()
        );
    }
    
    info!("Project setup validation completed successfully");
    
    // WHY: Load restart log to track completed files
    let mut restart_log = RestartLog::load(root_dir).await;
    
    // WHY: Clear restart log if requested by user
    if args.clear_restart_log {
        let cleared_count = restart_log.completed_count();
        restart_log.clear();
        info!("Cleared {} entries from restart log", cleared_count);
        if !args.quiet {
            println!("Restart log cleared - will reprocess all files");
        }
    } else {
        // WHY: Verify restart log integrity and clean up stale entries
        let initial_count = restart_log.completed_count();
        if initial_count > 0 {
            info!("Loaded restart log with {} completed files", initial_count);
            let invalid_files = restart_log.verify_completed_files().await?;
            if !invalid_files.is_empty() {
                info!("Cleaned {} stale entries from restart log", invalid_files.len());
            }
        }
    }
    
    // WHY: Use overlapped discovery and processing pipeline for optimal performance
    let (total_sentences, total_bytes, processed_files, skipped_files, failed_files, file_stats, processing_duration) = 
        process_with_overlapped_pipeline(root_dir, &args, &mut restart_log).await?;
    
    // WHY: Generate run statistics per PRD F-8 requirement
    let overall_chars_per_sec = if processing_duration.as_secs_f64() > 0.0 {
        total_bytes as f64 / processing_duration.as_secs_f64()
    } else {
        0.0
    };
    
    // WHY: Calculate sentence detection throughput for benchmarking
    let total_sentence_detection_time_ms: u64 = file_stats.iter()
        .map(|fs| fs.sentence_detection_time_ms)
        .sum();
    
    let sentence_detection_chars_per_sec = if total_sentence_detection_time_ms > 0 {
        total_bytes as f64 / (total_sentence_detection_time_ms as f64 / 1000.0)
    } else {
        0.0
    };
    
    // Calculate aggregate sentence length statistics only if requested
    let aggregate_sentence_length_stats = if args.sentence_length_stats {
        crate::parallel_processing::calculate_aggregate_sentence_length_stats(&file_stats)
    } else {
        None
    };
    
    let run_stats = RunStats {
        run_start: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs().to_string(),
        total_processing_time_ms: processing_duration.as_millis() as u64,
        total_sentence_detection_time_ms,
        total_chars_processed: total_bytes,
        total_sentences_detected: total_sentences,
        aggregate_sentence_length_stats,
        overall_chars_per_sec,
        sentence_detection_chars_per_sec,
        files_processed: processed_files,
        files_skipped: skipped_files,
        files_failed: failed_files,
        file_stats,
    };
    
    // WHY: Write stats to JSON file as specified by --stats-out flag
    match serde_json::to_string_pretty(&run_stats) {
        Ok(json_content) => {
            match tokio::fs::write(&args.stats_out, json_content).await {
                Ok(()) => {
                    info!("Stats written to {}", args.stats_out.display());
                    if !args.quiet {
                        println!("Stats written to {}", args.stats_out.display());
                    }
                }
                Err(e) => {
                    info!("Warning: Failed to write stats file: {e}");
                    if !args.quiet {
                        println!("Warning: Failed to write stats file: {e}");
                    }
                }
            }
        }
        Err(e) => {
            info!("Warning: Failed to serialize stats: {e}");
            if !args.quiet {
                println!("Warning: Failed to serialize stats: {e}");
            }
        }
    }
    
    Ok(())
}