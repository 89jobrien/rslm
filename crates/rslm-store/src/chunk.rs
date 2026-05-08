/// Strategy for splitting a document into chunks.
#[derive(Debug, Clone)]
pub enum ChunkStrategy {
    /// Split by byte size with `overlap` bytes between consecutive chunks (aligned to char
    /// boundaries).
    Fixed { size: usize, overlap: usize },
    /// Split on blank lines (`\n\n` or `\r\n\r\n`), trim each chunk.
    Paragraph,
    /// Split every `count` lines.
    Line { count: usize },
}

/// A single chunk of a document.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// SQLite rowid; 0 if not yet persisted.
    pub id: i64,
    /// Position in original document (0-based).
    pub index: usize,
    pub text: String,
    pub embedding: Option<Vec<f32>>,
}

/// Split `text` into chunks according to `strategy`.
pub fn chunk_text(text: &str, strategy: &ChunkStrategy) -> Vec<String> {
    match strategy {
        ChunkStrategy::Fixed { size, overlap } => chunk_fixed(text, *size, *overlap),
        ChunkStrategy::Paragraph => chunk_paragraph(text),
        ChunkStrategy::Line { count } => chunk_lines(text, *count),
    }
}

fn chunk_fixed(text: &str, size: usize, overlap: usize) -> Vec<String> {
    if size == 0 || text.is_empty() {
        return vec![];
    }
    let bytes = text.as_bytes();
    let len = bytes.len();
    let step = if size > overlap { size - overlap } else { 1 };
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < len {
        let end = (start + size).min(len);
        // Align end to a char boundary (scan back from end)
        let end = align_char_boundary(bytes, end);
        let slice = &text[start..end];
        if !slice.is_empty() {
            chunks.push(slice.to_string());
        }
        if end == len {
            break;
        }
        start += step;
        // Align start to char boundary
        start = align_char_boundary(bytes, start.min(len));
    }
    chunks
}

/// Return the largest index <= `pos` that is a valid UTF-8 char start boundary.
fn align_char_boundary(bytes: &[u8], pos: usize) -> usize {
    if pos >= bytes.len() {
        return bytes.len();
    }
    let mut p = pos;
    while p > 0 && (bytes[p] & 0xC0) == 0x80 {
        p -= 1;
    }
    p
}

fn chunk_paragraph(text: &str) -> Vec<String> {
    // Split on \n\n or \r\n\r\n
    let chunks: Vec<String> = text
        .split("\n\n")
        .flat_map(|s| s.split("\r\n\r\n"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    chunks
}

fn chunk_lines(text: &str, count: usize) -> Vec<String> {
    if count == 0 {
        return vec![];
    }
    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(count)
        .map(|group| group.join("\n"))
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_overlap() {
        let text = "a".repeat(100);
        let chunks = chunk_text(
            &text,
            &ChunkStrategy::Fixed {
                size: 20,
                overlap: 5,
            },
        );
        // Each chunk is 20 chars; consecutive chunks share 5 chars
        for pair in chunks.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            assert!(
                a.ends_with(&b[..5.min(b.len())]),
                "overlap expected: a tail={} b head={}",
                &a[a.len().saturating_sub(5)..],
                &b[..5.min(b.len())]
            );
        }
    }

    #[test]
    fn test_paragraph_three_chunks() {
        let text = "first paragraph\n\nsecond paragraph\n\nthird paragraph";
        let chunks = chunk_text(text, &ChunkStrategy::Paragraph);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn test_line_chunks() {
        let text = "line1\nline2\nline3\nline4\nline5";
        let chunks = chunk_text(text, &ChunkStrategy::Line { count: 2 });
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "line1\nline2");
    }
}
