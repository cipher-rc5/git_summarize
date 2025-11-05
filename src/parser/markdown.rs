// file: src/parser/markdown.rs
// description: markdown parsing with pulldown-cmark
// reference: https://docs.rs/pulldown-cmark

use crate::error::Result;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

pub struct MarkdownParser;

#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    pub plain_text: String,
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    pub code_blocks: Vec<CodeBlock>,
}

#[derive(Debug, Clone)]
pub struct Heading {
    pub level: u32,
    pub text: String,
    pub position: usize,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub text: String,
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub content: String,
}

impl MarkdownParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, content: &str) -> Result<ParsedMarkdown> {
        let parser = Parser::new(content);

        let mut plain_text = String::new();
        let mut headings = Vec::new();
        let mut links = Vec::new();
        let mut code_blocks = Vec::new();

        let mut current_heading: Option<(u32, String)> = None;
        let mut current_link: Option<(String, String)> = None;
        let mut current_code: Option<String> = None;
        let mut in_code_block = false;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    current_heading = Some((level as u32, String::new()));
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some((level, text)) = current_heading.take() {
                        headings.push(Heading {
                            level,
                            text: text.trim().to_string(),
                            position: plain_text.len(),
                        });
                    }
                }
                Event::Start(Tag::Link {
                    dest_url, title, ..
                }) => {
                    current_link = Some((dest_url.to_string(), title.to_string()));
                }
                Event::End(TagEnd::Link) => {
                    if let Some((url, title)) = current_link.take() {
                        let link_text = plain_text
                            .split_whitespace()
                            .last()
                            .unwrap_or("")
                            .to_string();

                        links.push(Link {
                            text: link_text,
                            url,
                            title: if title.is_empty() { None } else { Some(title) },
                        });
                    }
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block = true;
                    current_code = Some(String::new());
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    if let Some(code) = current_code.take() {
                        code_blocks.push(CodeBlock {
                            language: None,
                            content: code,
                        });
                    }
                }
                Event::Text(text) => {
                    if let Some((_, ref mut heading_text)) = current_heading {
                        heading_text.push_str(&text);
                    }

                    if in_code_block {
                        if let Some(ref mut code) = current_code {
                            code.push_str(&text);
                        }
                    } else {
                        plain_text.push_str(&text);
                        plain_text.push(' ');
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    plain_text.push('\n');
                }
                _ => {}
            }
        }

        Ok(ParsedMarkdown {
            plain_text: plain_text.trim().to_string(),
            headings,
            links,
            code_blocks,
        })
    }

    pub fn extract_section(&self, content: &str, heading: &str) -> Option<String> {
        let parser = Parser::new(content);
        let mut in_target_section = false;
        let mut section_content = String::new();
        let mut current_heading_text = String::new();
        let mut in_heading = false;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { .. }) => {
                    if in_target_section {
                        break;
                    }
                    in_heading = true;
                    current_heading_text.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    in_heading = false;
                    if current_heading_text.trim().eq_ignore_ascii_case(heading) {
                        in_target_section = true;
                    }
                }
                Event::Text(text) => {
                    if in_heading {
                        current_heading_text.push_str(&text);
                    } else if in_target_section {
                        section_content.push_str(&text);
                        section_content.push(' ');
                    }
                }
                _ => {}
            }
        }

        if section_content.is_empty() {
            None
        } else {
            Some(section_content.trim().to_string())
        }
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let parser = MarkdownParser::new();
        let content = "# Title\n\nSome content here.";
        let parsed = parser.parse(content).unwrap();

        assert_eq!(parsed.headings.len(), 1);
        assert_eq!(parsed.headings[0].text, "Title");
        assert!(parsed.plain_text.contains("Some content"));
    }

    #[test]
    fn test_link_extraction() {
        let parser = MarkdownParser::new();
        let content = "[Example](https://example.com)";
        let parsed = parser.parse(content).unwrap();

        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].url, "https://example.com");
    }

    #[test]
    fn test_section_extraction() {
        let parser = MarkdownParser::new();
        let content = "# Section 1\n\nContent 1\n\n# Section 2\n\nContent 2";
        let section = parser.extract_section(content, "Section 1");

        assert!(section.is_some());
        assert!(section.unwrap().contains("Content 1"));
    }
}
