#!/usr/bin/env python3
"""
Find ALL cases where SEAMS differs from pysbd and/or nupunkt.

No prioritization, no filtering, no artificial limits.
Present all differences for human judgment of correctness.

Usage:
    python find_seams_differences.py --max-files 50 --output seams_differences.json

Requires:
    - nupunkt: pip install nupunkt  
    - pysbd: pip install pysbd
"""

import os
import re
import json
import argparse
from pathlib import Path
from typing import List, Dict, Tuple, Set, Optional
from dataclasses import dataclass

# Sentence segmentation imports
try:
    import nupunkt
    NUPUNKT_AVAILABLE = True
except ImportError:
    NUPUNKT_AVAILABLE = False
    print("ERROR: nupunkt not available. Install with: pip install nupunkt")

try:
    import pysbd
    PYSBD_AVAILABLE = True
    pysbd_segmenter = pysbd.Segmenter(language="en", clean=False)
except ImportError:
    PYSBD_AVAILABLE = False
    print("ERROR: pysbd not available. Install with: pip install pysbd")

if not (NUPUNKT_AVAILABLE and PYSBD_AVAILABLE):
    print("Both nupunkt and pysbd are required for this script.")
    exit(1)


@dataclass
class SeamsDifference:
    """A case where SEAMS produces different segmentation"""
    original_text: str
    seams_sentences: List[str]
    pysbd_sentences: List[str] 
    nupunkt_sentences: List[str]
    source_file: str
    text_start_line: int
    text_start_col: int
    text_end_line: int
    text_end_col: int
    differs_from_pysbd: bool
    differs_from_nupunkt: bool


def extract_original_text_from_coordinates(original_file: Path, start_line: int, start_col: int, 
                                         end_line: int, end_col: int) -> str:
    """Extract original text using 1-based coordinates from seams file"""
    try:
        with open(original_file, 'r', encoding='utf-8') as f:
            lines = f.readlines()
        
        # Convert to 0-based indexing
        start_line_idx = start_line - 1
        end_line_idx = end_line - 1
        start_col_idx = start_col - 1
        end_col_idx = end_col - 1
        
        if start_line_idx == end_line_idx:
            # Single line
            return lines[start_line_idx][start_col_idx:end_col_idx]
        else:
            # Multi-line: start of first line + middle lines + end of last line
            result = lines[start_line_idx][start_col_idx:]
            for line_idx in range(start_line_idx + 1, end_line_idx):
                result += lines[line_idx]
            result += lines[end_line_idx][:end_col_idx]
            return result
            
    except Exception as e:
        print(f"Error extracting text from {original_file}: {e}")
        return ""


def parse_seams_file(seams_file: Path) -> List[Tuple[str, int, int, int, int]]:
    """Parse seams2.txt file and return sentences with coordinates"""
    sentences_with_coords = []
    
    try:
        with open(seams_file, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                    
                parts = line.split('\t')
                if len(parts) >= 3:
                    sentence_text = parts[1]
                    coords_str = parts[2]
                    
                    # Parse coordinates: (start_line,start_col,end_line,end_col)
                    coord_match = re.match(r'\((\d+),(\d+),(\d+),(\d+)\)', coords_str)
                    if coord_match:
                        start_line, start_col, end_line, end_col = map(int, coord_match.groups())
                        sentences_with_coords.append((sentence_text, start_line, start_col, end_line, end_col))
                        
    except Exception as e:
        print(f"Error parsing seams file {seams_file}: {e}")
        
    return sentences_with_coords


def segment_with_pysbd(text: str) -> List[str]:
    """Segment text using pysbd"""
    try:
        sentences = pysbd_segmenter.segment(text)
        return [s.strip() for s in sentences if s.strip()]
    except Exception as e:
        print(f"pysbd segmentation error: {e}")
        return []


def segment_with_nupunkt(text: str) -> List[str]:
    """Segment text using nupunkt"""  
    try:
        sentences = nupunkt.sent_tokenize(text)
        return [s.strip() for s in sentences if s.strip()]
    except Exception as e:
        print(f"nupunkt segmentation error: {e}")
        return []


def normalize_text(text: str) -> str:
    """Normalize text for comparison"""
    return ' '.join(text.split()).strip()


def segmentations_differ(seams_sentences: List[str], other_sentences: List[str]) -> bool:
    """Check if two segmentations are different"""
    # Different sentence counts = different
    if len(seams_sentences) != len(other_sentences):
        return True
    
    # Compare normalized concatenated text
    seams_text = ' '.join(normalize_text(s) for s in seams_sentences)
    other_text = ' '.join(normalize_text(s) for s in other_sentences)
    
    return seams_text != other_text


def find_natural_text_boundaries(seams_data: List[Tuple[str, int, int, int, int]]) -> List[Tuple[int, int]]:
    """Find natural text boundaries for comparison (paragraph breaks, dialog changes, etc.)"""
    boundaries = []
    
    # Look for natural breaks in the text
    i = 0
    while i < len(seams_data):
        start_idx = i
        
        # Find a reasonable chunk (until we hit a natural break)
        while i < len(seams_data) - 1:
            current_sentence = seams_data[i][0]
            next_sentence = seams_data[i + 1][0]
            
            # Break at paragraph boundaries (hard separators)
            current_coords = seams_data[i]
            next_coords = seams_data[i + 1]
            
            # If there's a line gap between sentences, that's a natural break
            if next_coords[1] > current_coords[3] + 1:  # next start line > current end line + 1
                break
                
            # Break after dialog attribution
            if re.search(r'(said|asked|replied|shouted|whispered)[.!?]?\s*$', current_sentence.lower()):
                break
                
            # Break at quote endings followed by non-dialog
            if current_sentence.endswith('"') and not next_sentence.startswith('"'):
                break
                
            # Limit chunk size to avoid huge comparisons
            if i - start_idx >= 10:
                break
                
            i += 1
        
        # Always include at least one sentence
        if i == start_idx:
            i += 1
            
        boundaries.append((start_idx, i))
    
    return boundaries


def find_differences_in_file_pair(original_file: Path, seams_file: Path) -> List[SeamsDifference]:
    """Find all differences between SEAMS and other segmenters for a file pair"""
    differences = []
    
    print(f"Analyzing {original_file.name}...")
    
    # Parse seams file
    seams_data = parse_seams_file(seams_file)
    if not seams_data:
        print(f"  ❌ No seams data found in {seams_file}")
        return differences
    
    print(f"  📄 Found {len(seams_data)} sentences in seams file")
    
    # Find natural text boundaries for comparison
    boundaries = find_natural_text_boundaries(seams_data)
    print(f"  🔍 Found {len(boundaries)} natural text chunks to compare")
    
    for chunk_idx, (start_idx, end_idx) in enumerate(boundaries):
        chunk_sentences = seams_data[start_idx:end_idx]
        
        # Extract the original text for this chunk
        first_sentence = chunk_sentences[0]
        last_sentence = chunk_sentences[-1]
        
        # Get the full text span
        original_text = extract_original_text_from_coordinates(
            original_file, 
            first_sentence[1], first_sentence[2],  # start line, start col
            last_sentence[3], last_sentence[4]     # end line, end col
        )
        
        if not original_text or len(original_text.strip()) < 10:
            continue
            
        # Get seams sentences for this chunk
        seams_sentences = [sent[0] for sent in chunk_sentences]
        
        # Segment the original text with other methods
        pysbd_sentences = segment_with_pysbd(original_text)
        nupunkt_sentences = segment_with_nupunkt(original_text)
        
        if not pysbd_sentences or not nupunkt_sentences:
            continue
            
        # Check for differences
        differs_from_pysbd = segmentations_differ(seams_sentences, pysbd_sentences)
        differs_from_nupunkt = segmentations_differ(seams_sentences, nupunkt_sentences)
        
        # If SEAMS differs from either, record it
        if differs_from_pysbd or differs_from_nupunkt:
            difference = SeamsDifference(
                original_text=original_text,
                seams_sentences=seams_sentences,
                pysbd_sentences=pysbd_sentences,
                nupunkt_sentences=nupunkt_sentences,
                source_file=str(original_file),
                text_start_line=first_sentence[1],
                text_start_col=first_sentence[2],
                text_end_line=last_sentence[3],
                text_end_col=last_sentence[4],
                differs_from_pysbd=differs_from_pysbd,
                differs_from_nupunkt=differs_from_nupunkt
            )
            
            differences.append(difference)
            
            print(f"    🔍 Difference found in chunk {chunk_idx}")
            print(f"        SEAMS: {len(seams_sentences)} sentences")
            print(f"        pysbd: {len(pysbd_sentences)} sentences (differs: {differs_from_pysbd})")
            print(f"        nupunkt: {len(nupunkt_sentences)} sentences (differs: {differs_from_nupunkt})")
            print(f"        Text: {original_text[:80]}...")
    
    return differences


def find_all_differences(gutenberg_root: str, max_files: int) -> List[SeamsDifference]:
    """Find all differences across multiple file pairs"""
    
    differences = []
    files_processed = 0
    
    # Find all original/seams file pairs
    gutenberg_path = Path(gutenberg_root)
    
    for original_file in gutenberg_path.rglob("*-0.txt"):
        if files_processed >= max_files:
            break
            
        seams_file = original_file.with_name(original_file.stem + "_seams2.txt")
        if seams_file.exists():
            file_differences = find_differences_in_file_pair(original_file, seams_file)
            differences.extend(file_differences)
            files_processed += 1
    
    print(f"\nProcessed {files_processed} file pairs")
    print(f"Found {len(differences)} total differences")
    
    return differences


def export_differences(differences: List[SeamsDifference], output_file: str):
    """Export all differences to JSON format for human review"""
    
    export_data = {
        "metadata": {
            "total_differences": len(differences),
            "pysbd_differences": len([d for d in differences if d.differs_from_pysbd]),
            "nupunkt_differences": len([d for d in differences if d.differs_from_nupunkt]),
            "both_differences": len([d for d in differences if d.differs_from_pysbd and d.differs_from_nupunkt]),
            "generation_method": "unfiltered_difference_detection",
            "segmenters_compared": ["seams", "pysbd", "nupunkt"]
        },
        "differences": []
    }
    
    for i, diff in enumerate(differences):
        export_data["differences"].append({
            "id": i,
            "original_text": diff.original_text,
            "seams_sentences": diff.seams_sentences,
            "pysbd_sentences": diff.pysbd_sentences,
            "nupunkt_sentences": diff.nupunkt_sentences,
            "source_file": diff.source_file,
            "coordinates": {
                "start_line": diff.text_start_line,
                "start_col": diff.text_start_col,
                "end_line": diff.text_end_line,
                "end_col": diff.text_end_col
            },
            "differs_from_pysbd": diff.differs_from_pysbd,
            "differs_from_nupunkt": diff.differs_from_nupunkt,
            "sentence_counts": {
                "seams": len(diff.seams_sentences),
                "pysbd": len(diff.pysbd_sentences),
                "nupunkt": len(diff.nupunkt_sentences)
            }
        })
    
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(export_data, f, indent=2, ensure_ascii=False)
    
    print(f"\nExported {len(differences)} differences to {output_file}")


def main():
    parser = argparse.ArgumentParser(description="Find ALL cases where SEAMS differs from pysbd/nupunkt")
    parser.add_argument('--gutenberg-root', default='/data/gutenberg', 
                       help='Root directory of Gutenberg corpus')
    parser.add_argument('--output', default='seams_differences.json',
                       help='Output JSON file')
    parser.add_argument('--max-files', type=int, default=50,
                       help='Maximum number of file pairs to analyze')
    
    args = parser.parse_args()
    
    print("🔍 Finding ALL cases where SEAMS differs from pysbd/nupunkt...")
    print(f"📁 Gutenberg root: {args.gutenberg_root}")
    print(f"📊 Max files: {args.max_files}")
    print(f"📄 Output: {args.output}")
    print()
    
    # Find all differences
    differences = find_all_differences(args.gutenberg_root, args.max_files)
    
    if differences:
        export_differences(differences, args.output)
        
        # Print summary
        print("\n📊 SUMMARY:")
        print(f"Total differences found: {len(differences)}")
        print(f"Differs from pysbd: {len([d for d in differences if d.differs_from_pysbd])}")
        print(f"Differs from nupunkt: {len([d for d in differences if d.differs_from_nupunkt])}")
        print(f"Differs from both: {len([d for d in differences if d.differs_from_pysbd and d.differs_from_nupunkt])}")
        print(f"Differs from only one: {len([d for d in differences if d.differs_from_pysbd != d.differs_from_nupunkt])}")
        
    else:
        print("No differences found!")


if __name__ == "__main__":
    main()