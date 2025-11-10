# git_summarize

A high-performance RAG (Retrieval-Augmented Generation) pipeline for GitHub repositories using LanceDB vector storage. This tool downloads any public or private GitHub repository, processes its content, generates embeddings, and stores them in LanceDB for efficient semantic search and LLM context retrieval.

## Features

- 🚀 **Universal GitHub Support**: Download and process any public or private GitHub repository
- 🔍 **Vector Search**: LanceDB-powered semantic search with embeddings
- ⚡ **High Performance**: Parallel processing with configurable worker pools
- 📊 **RAG Pipeline**: Production-ready retrieval pipeline for LLM applications
- 🔄 **Incremental Updates**: Smart sync with deduplication
- 📝 **Markdown Processing**: Advanced parsing and normalization
- 🤖 **MCP Integration**: Model Context Protocol server for agentic tools and local LLMs
- 🎯 **Flexible Configuration**: TOML config files with environment variable overrides
- 🌐 **Groq API Support**: Optional integration with Groq embeddings API

## What is RAG?

RAG (Retrieval-Augmented Generation) enhances LLMs by providing relevant context from a knowledge base. This tool:
1. Ingests repository content
2. Generates vector embeddings
3. Stores them in LanceDB
4. Enables semantic search for LLM context retrieval

## Prerequisites

- Rust 1.80+ (install from https://rustup.rs)
- Git (for repository synchronization)

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/cipher-rc5/git_summarize
cd git_summarize
```

### 2. Build the Application

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release
```

## Quick Start

### 1. Configure Your Repository

Edit `config/default.toml`:

```toml
[repository]
source_url = "https://github.com/username/your-repo"
local_path = "./data_repo"
branch = "main"
sync_on_start = true

[database]
uri = "data/lancedb"
table_name = "documents"
batch_size = 100
embedding_dim = 384
```

For private repositories, use a personal access token:
```toml
source_url = "https://YOUR_TOKEN@github.com/username/private-repo"
```

### 2. Run the Pipeline

```bash
# Sync repository and ingest
cargo run --release -- ingest

# Force reprocess all files
cargo run --release -- ingest --force

# Process with custom config
cargo run --release -- --config my-config.toml ingest
```

### 3. Query the Database

The vector database is now ready for semantic search! You can:
- Use LanceDB Python SDK for queries
- Build a REST API on top
- Integrate with LLM applications

## Usage

### Command-Line Interface

```bash
git_summarize [OPTIONS] <COMMAND>

Commands:
  sync     Synchronize repository
  ingest   Run full ingestion pipeline
  verify   Verify database schema
  stats    Show database statistics
  reset    Reset database (WARNING: deletes all data)
  export   Export data to JSON
  help     Print help information

Options:
  -c, --config <FILE>   Configuration file [default: config/default.toml]
  --color              Enable colored output [default: true]
  -v, --verbose        Enable verbose logging
  -h, --help           Print help
  -V, --version        Print version
```

### Examples

```bash
# Verify connection
cargo run -- verify

# Sync repository only
cargo run -- sync

# Run full pipeline
cargo run --release -- ingest

# Skip sync, just process files
cargo run -- ingest --skip-sync

# Process limited number of files (testing)
cargo run -- ingest --limit 10

# Force reprocess all files
cargo run -- ingest --force

# Show statistics
cargo run -- stats

# Export to JSON
cargo run -- export --output ./exports --pretty

# Reset database
cargo run -- reset --confirm
```

## Configuration

### Configuration Priority

Settings are loaded in this order (later overrides earlier):

1. `config/default.toml` (default settings)
2. Custom config file via `--config` flag
3. Environment variables (prefixed with `GIT_SUMMARIZE__`)
4. Command-line arguments

### Environment Variables

```bash
export GIT_SUMMARIZE_DATABASE__URI="data/lancedb"
export GIT_SUMMARIZE_DATABASE__TABLE_NAME="documents"
export GIT_SUMMARIZE_DATABASE__BATCH_SIZE=100
export GIT_SUMMARIZE_DATABASE__EMBEDDING_DIM=384

cargo run -- ingest
```

### Repository Configuration

```toml
[repository]
# Public repo
source_url = "https://github.com/rust-lang/rust"

# Private repo with token
source_url = "https://ghp_TOKEN@github.com/org/private-repo"

# Local path for cloning
local_path = "./data_repo"

# Branch to track
branch = "main"

# Auto-sync on start
sync_on_start = true
```

### Database Configuration

```toml
[database]
# LanceDB URI (local or remote)
uri = "data/lancedb"

# Table name
table_name = "documents"

# Batch size for insertions
batch_size = 100

# Embedding dimensions
# 384: all-MiniLM-L6-v2 (default, fast)
# 768: BERT-base
# 1536: OpenAI text-embedding-ada-002
embedding_dim = 384
```

### Pipeline Configuration

```toml
[pipeline]
# Parallel workers (adjust based on CPU cores)
parallel_workers = 4

# Files/directories to skip
skip_patterns = [
  "*.zip",
  "*.pdf",
  ".git/*",
  "node_modules/*",
  "target/*",
]

# Force reprocess (ignore deduplication)
force_reprocess = false

# Maximum file size in MB
max_file_size_mb = 10
```

## Architecture

### Data Flow

```
GitHub Repository
    ↓
Git Clone/Sync
    ↓
File Scanner (*.md, *.txt, etc.)
    ↓
Parallel Processing Pool
    ↓
┌─────────────────────────┐
│  Document Processor     │
│  ├─ Read file          │
│  ├─ Parse markdown     │
│  ├─ Normalize content  │
│  ├─ Generate embedding │
│  └─ Extract entities   │
└─────────────────────────┘
    ↓
LanceDB Vector Storage
    ↓
RAG / Semantic Search
```

### Vector Embeddings

Currently uses a simple deterministic embedding (placeholder). For production:

**Recommended embedding models:**
- **Local**: all-MiniLM-L6-v2 (384 dims, fast)
- **Cloud**: OpenAI text-embedding-ada-002 (1536 dims)
- **Custom**: sentence-transformers, Cohere, etc.

**To integrate real embeddings:**
```rust
// In src/database/insert.rs, replace generate_embedding()
// with your embedding model:
use fastembed::TextEmbedding;

fn generate_embedding(text: &str, dim: usize) -> Vec<f32> {
    let model = TextEmbedding::try_new(Default::default()).unwrap();
    model.embed(vec![text], None).unwrap()[0].clone()
}
```

## Schema

### Documents Table

```
id: String              - Content hash (unique identifier)
file_path: String       - Absolute file path
relative_path: String   - Repository-relative path
content: String         - Full file content
content_hash: String    - SHA256 hash
file_size: UInt64       - File size in bytes
last_modified: UInt64   - Unix timestamp
parsed_at: UInt64       - Processing timestamp
normalized: Boolean     - Markdown normalization flag
embedding: Vec<f32>     - Vector embedding (384 dims default)
title: String?          - Optional extracted title
description: String?    - Optional description
language: String?       - Optional language detection
repository_url: String? - Optional source URL
```

## Querying with Python

```python
import lancedb

# Connect to database
db = lancedb.connect("data/lancedb")
table = db.open_table("documents")

# Semantic search
query_embedding = model.encode("your search query")
results = table.search(query_embedding) \
    .limit(5) \
    .to_pandas()

print(results[['relative_path', 'content']])
```

## Performance Tuning

### Parallel Workers

```toml
[pipeline]
# Set based on CPU cores
parallel_workers = 8  # For 8-core CPU
```

### Batch Size

```toml
[database]
batch_size = 200  # Larger = faster, more memory
```

### Release Optimizations

```bash
# Maximum performance
cargo build --release

# Profile-guided optimization
cargo pgo build
```

## Development

### Running Tests

```bash
# All tests
cargo test

# Specific module
cargo test database

# With output
cargo test -- --nocapture
```

### Code Style

```bash
# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings

# Check
cargo check
```

## Use Cases

- **Documentation RAG**: Query codebases with natural language
- **Code Analysis**: Semantic search across repositories
- **Knowledge Bases**: Build searchable documentation
- **Research**: Analyze open-source projects at scale
- **Security**: Track vulnerabilities and incidents
- **Compliance**: Monitor code for policy violations

## Roadmap

- [ ] Real embedding model integration (sentence-transformers)
- [ ] REST API for queries
- [ ] Web UI for browsing
- [ ] Multi-repository support
- [ ] Advanced filtering and search
- [ ] Export to FAISS/Pinecone/Weaviate
- [ ] Incremental embedding updates
- [ ] Language detection and filtering

## Contributing

Contributions welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` passes
5. Run `cargo fmt` and `cargo clippy`
6. Submit a pull request

## License

This project is licensed under the MIT License.

## Acknowledgments

- Built with [LanceDB](https://lancedb.com/) for vector storage
- Powered by [Apache Arrow](https://arrow.apache.org/)
- Repository management via [gix](https://github.com/Byron/gitoxide)

## Support

For issues and questions:
- GitHub Issues: https://github.com/cipher-rc5/git_summarize/issues
- Documentation: https://docs.rs/lancedb

## Authors

- [ℭ𝔦𝔭𝔥𝔢𝔯](https://github.com/cipher-rc5)

## MCP (Model Context Protocol) Integration

Git Summarize includes an MCP server that allows integration with agentic tools and local LLM units like Claude Desktop, Cline, and other MCP-compatible clients.

### What is MCP?

Model Context Protocol (MCP) is an open protocol that standardizes how applications provide context to LLMs. It enables LLMs to securely access tools, data sources, and services through a consistent interface.

### Starting the MCP Server

```bash
# Start MCP server with stdio transport
cargo run --release -- mcp

# Or with custom config
cargo run --release -- --config my-config.toml mcp
```

### Available MCP Tools

1. **ingest_repository**: Ingest a GitHub repository into the RAG pipeline
   - Parameters: `repo_url`, `branch` (optional), `force` (optional)
   - Example: Ingest `https://github.com/rust-lang/rust` on branch `main`

2. **get_stats**: Get statistics about ingested documents
   - No parameters required
   - Returns: Document count, storage info

3. **search_documents**: Search for documents by content
   - Parameters: `query`, `limit` (optional)
   - Note: Vector search coming soon

4. **get_config**: View current configuration
   - No parameters required
   - Returns: Repository, database, and pipeline settings

5. **verify_database**: Check database connection and schema
   - No parameters required
   - Returns: Connection status and schema validity

### Using with Claude Desktop

Add to your Claude Desktop configuration (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "git_summarize": {
      "command": "/path/to/git_summarize",
      "args": ["--config", "/path/to/config.toml", "mcp"]
    }
  }
}
```

### Using with Cline

Configure in Cline's MCP settings:

```json
{
  "name": "git_summarize",
  "command": "/path/to/git_summarize",
  "args": ["mcp"],
  "transport": "stdio"
}
```

### Example MCP Usage

Once configured, you can ask your LLM assistant:

- "Ingest the repository https://github.com/anthropics/anthropic-sdk-python"
- "What repositories have been ingested? Show me the stats"
- "Search for documents about authentication"
- "Verify the database connection"

