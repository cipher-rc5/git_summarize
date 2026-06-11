// file: src/parser/mod.rs
// description: markdown parsing module exports
// reference: internal module structure

pub mod chunker;
pub mod frontmatter;
pub mod markdown;
pub mod normalizer;

pub use chunker::{Chunk, ChunkOptions, chunk_markdown};
pub use frontmatter::{Frontmatter, FrontmatterParser};
pub use markdown::{CodeBlock, Heading, Link, MarkdownParser, ParsedMarkdown};
pub use normalizer::MarkdownNormalizer;
