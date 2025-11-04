// file: src/parser/mod.rs
// description: Markdown parsing module exports
// reference: Internal module structure

pub mod frontmatter;
pub mod markdown;
pub mod normalizer;

pub use frontmatter::{Frontmatter, FrontmatterParser};
pub use markdown::{CodeBlock, Heading, Link, MarkdownParser, ParsedMarkdown};
pub use normalizer::MarkdownNormalizer;
