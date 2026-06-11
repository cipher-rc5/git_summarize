// file: src/parser/chunker.rs
// description: heading-aware markdown chunking for RAG retrieval granularity
// reference: https://docs.rs/pulldown-cmark

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// A retrieval unit: a slice of a document scoped to a heading, carrying the
/// heading breadcrumb so embeddings and answers have local context.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    /// Ordinal position of this chunk within its source document.
    pub index: usize,
    /// Heading breadcrumb from document root to this section, e.g.
    /// `["Installation", "From source"]`. Empty for preamble before any heading.
    pub heading_path: Vec<String>,
    /// Raw markdown of the section (heading line included).
    pub content: String,
}

impl Chunk {
    /// Text used for embedding/retrieval: breadcrumb prefix + content. The
    /// breadcrumb gives an otherwise-isolated section the context of where it
    /// sits in the document, which materially improves retrieval.
    pub fn embedding_text(&self) -> String {
        if self.heading_path.is_empty() {
            self.content.clone()
        } else {
            format!("{}\n\n{}", self.heading_path.join(" > "), self.content)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChunkOptions {
    /// Sections longer than this (in chars) are split into overlapping windows.
    pub max_chars: usize,
    /// Overlap (in chars) between adjacent windows when a section is split.
    pub overlap: usize,
    /// Sections shorter than this are merged into the previous chunk to avoid
    /// tiny, low-signal vectors.
    pub min_chars: usize,
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self {
            max_chars: 2000,
            overlap: 200,
            min_chars: 200,
        }
    }
}

/// Split markdown into heading-scoped chunks.
///
/// A section runs from one heading up to the next heading of the same or higher
/// level. Oversized sections are windowed with overlap; undersized sections are
/// merged forward. If the document has no headings, the whole document is
/// windowed by size.
pub fn chunk_markdown(content: &str, opts: &ChunkOptions) -> Vec<Chunk> {
    let sections = split_into_sections(content);

    // Merge tiny sections into the preceding one to avoid low-signal fragments.
    let mut merged: Vec<Section> = Vec::new();
    for section in sections {
        if let Some(last) = merged.last_mut()
            && section.body.trim().len() < opts.min_chars
            && last.heading_path == section.heading_path
        {
            last.body.push_str(&section.body);
            continue;
        }
        merged.push(section);
    }

    let mut chunks = Vec::new();
    let mut index = 0;
    for section in merged {
        let text = section.body.trim();
        if text.is_empty() {
            continue;
        }
        for window in window_text(text, opts.max_chars, opts.overlap) {
            chunks.push(Chunk {
                index,
                heading_path: section.heading_path.clone(),
                content: window,
            });
            index += 1;
        }
    }

    chunks
}

#[derive(Debug)]
struct Section {
    heading_path: Vec<String>,
    body: String,
}

fn split_into_sections(content: &str) -> Vec<Section> {
    let parser = Parser::new_ext(content, Options::all()).into_offset_iter();

    // Collect heading boundaries: (byte_offset, level, text).
    let mut headings: Vec<(usize, u32, String)> = Vec::new();
    let mut current: Option<(usize, u32, String)> = None;
    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((range.start, level as u32, String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(h) = current.take() {
                    headings.push(h);
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, ref mut text)) = current {
                    text.push_str(&t);
                }
            }
            _ => {}
        }
    }

    if headings.is_empty() {
        return vec![Section {
            heading_path: Vec::new(),
            body: content.to_string(),
        }];
    }

    let mut sections = Vec::new();

    // Preamble before the first heading, if any.
    let first_offset = headings[0].0;
    if !content[..first_offset].trim().is_empty() {
        sections.push(Section {
            heading_path: Vec::new(),
            body: content[..first_offset].to_string(),
        });
    }

    // Running breadcrumb stack of (level, text).
    let mut stack: Vec<(u32, String)> = Vec::new();
    for (i, (offset, level, text)) in headings.iter().enumerate() {
        // Pop deeper-or-equal levels so the stack reflects ancestors only.
        while let Some((lvl, _)) = stack.last() {
            if *lvl >= *level {
                stack.pop();
            } else {
                break;
            }
        }
        stack.push((*level, text.trim().to_string()));

        let end = headings
            .get(i + 1)
            .map(|(next_offset, _, _)| *next_offset)
            .unwrap_or(content.len());

        sections.push(Section {
            heading_path: stack.iter().map(|(_, t)| t.clone()).collect(),
            body: content[*offset..end].to_string(),
        });
    }

    sections
}

/// Split `text` into overlapping windows of at most `max_chars`, preferring to
/// break on paragraph then line boundaries. Operates on char boundaries.
fn window_text(text: &str, max_chars: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return vec![text.to_string()];
    }

    let step = max_chars.saturating_sub(overlap).max(1);
    let mut windows = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let hard_end = (start + max_chars).min(chars.len());
        // Try to end on a paragraph or newline boundary within the window.
        let end = if hard_end < chars.len() {
            let slice = &chars[start..hard_end];
            find_break(slice).map(|b| start + b).unwrap_or(hard_end)
        } else {
            hard_end
        };
        let window: String = chars[start..end].iter().collect();
        if !window.trim().is_empty() {
            windows.push(window.trim().to_string());
        }
        if end >= chars.len() {
            break;
        }
        start = end.saturating_sub(overlap).max(start + step.min(end - start));
    }
    windows
}

/// Find a good break point (end-of-paragraph, else end-of-line) in the latter
/// half of `slice`, returning an index into `slice`.
fn find_break(slice: &[char]) -> Option<usize> {
    let half = slice.len() / 2;
    // Prefer a blank line (paragraph) break.
    for i in (half..slice.len().saturating_sub(1)).rev() {
        if slice[i] == '\n' && slice[i + 1] == '\n' {
            return Some(i + 1);
        }
    }
    // Fall back to any newline.
    for i in (half..slice.len()).rev() {
        if slice[i] == '\n' {
            return Some(i + 1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_headings_with_breadcrumb() {
        let md = "# Title\n\nIntro paragraph that is reasonably long so it is kept.\n\n## Setup\n\nDo the setup steps here with enough text to survive the min filter.\n\n## Usage\n\nUse it like so, again with sufficient body text to be retained.";
        let chunks = chunk_markdown(md, &ChunkOptions::default());
        assert!(chunks.len() >= 2, "expected multiple chunks, got {:?}", chunks);
        let paths: Vec<_> = chunks.iter().map(|c| c.heading_path.clone()).collect();
        assert!(paths.iter().any(|p| p == &vec!["Title".to_string()]));
        assert!(paths.iter().any(|p| p.contains(&"Setup".to_string())));
    }

    #[test]
    fn nested_headings_build_path() {
        let md = "# A\n\nbody a is long enough to be kept as its own section here.\n\n## B\n\nbody b also long enough to be retained as a separate chunk here.\n\n### C\n\nbody c with plenty of words so the min char filter keeps it around.";
        let chunks = chunk_markdown(md, &ChunkOptions::default());
        let c = chunks
            .iter()
            .find(|c| c.heading_path.last() == Some(&"C".to_string()))
            .expect("C chunk");
        assert_eq!(
            c.heading_path,
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
    }

    #[test]
    fn no_headings_returns_whole_doc() {
        let md = "just some text with no headings at all in it whatsoever.";
        let chunks = chunk_markdown(md, &ChunkOptions::default());
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].heading_path.is_empty());
    }

    #[test]
    fn oversized_section_is_windowed() {
        let body = "word ".repeat(2000); // ~10k chars
        let md = format!("# Big\n\n{}", body);
        let opts = ChunkOptions {
            max_chars: 1000,
            overlap: 100,
            min_chars: 50,
        };
        let chunks = chunk_markdown(&md, &opts);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.content.chars().count() <= 1000));
    }
}
