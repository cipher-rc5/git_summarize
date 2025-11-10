// file: src/mcp/mod.rs
// description: MCP (Model Context Protocol) server for agentic tool integration
// reference: https://docs.rs/rmcp

pub mod persistence;
pub mod server;

pub use persistence::{MetadataStore, RepositoryMetadata};
pub use server::GitSummarizeMcp;
