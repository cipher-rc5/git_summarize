# lazarus_ingest

Rust application for ingesting, parsing, and analyzing threat intelligence data from the Lazarus/BlueNoroff APT research repository. Extracts cryptocurrency addresses, incident reports, and indicators of compromise (IOCs) into ClickHouse for analytical queries.

## Features

- Automated Git repository synchronization
- Parallel markdown file processing
- Entity extraction (crypto addresses, incidents, IOCs)
- Markdown normalization and parsing
- Batch insertion into ClickHouse
- Progress tracking with real-time statistics
- Comprehensive error handling and logging
- Idempotent processing with deduplication

## Prerequisites

- Rust 1.80+ (install from https://rustup.rs)
- ClickHouse Server (local or remote instance)
- Git (for repository synchronization)

## Project Structure

```
lazarus_ingest/
├── src/
│   ├── database/          # ClickHouse client and schema management
│   ├── extractor/         # Entity extraction (crypto, incidents, IOCs)
│   ├── models/            # Data models and structures
│   ├── parser/            # Markdown parsing and normalization
│   ├── pipeline/          # Processing orchestration
│   ├── repository/        # Git sync and file scanning
│   ├── utils/             # Utility functions
│   ├── config.rs          # Configuration management
│   ├── error.rs           # Error types
│   ├── lib.rs             # Library exports
│   └── main.rs            # CLI entry point
├── config/
│   └── default.toml       # Default configuration
├── scripts/
│   ├── setup_db.sql       # Database schema
│   └── example_queries.sql # Example analytical queries
├── Cargo.toml
└── README.md
```

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/cipher-rc5/lazarus_ingest.git
cd lazarus_ingest
```

### 2. Install ClickHouse

#### macOS (Homebrew)
```bash
brew install clickhouse
brew services start clickhouse
```

#### Linux (Ubuntu/Debian)
```bash
sudo apt-get install -y apt-transport-https ca-certificates dirmngr
sudo apt-key adv --keyserver hkp://keyserver.ubuntu.com:80 --recv 8919F6BD2B48D754

echo "deb https://packages.clickhouse.com/deb stable main" | sudo tee \
    /etc/apt/sources.list.d/clickhouse.list
sudo apt-get update

sudo apt-get install -y clickhouse-server clickhouse-client
sudo service clickhouse-server start
```

#### Docker
```bash
docker run -d --name clickhouse-server \
  -p 8123:8123 -p 9000:9000 \
  --ulimit nofile=262144:262144 \
  clickhouse/clickhouse-server
```

#### Verify Installation
```bash
curl http://localhost:8123/
# Should return: Ok.
```

### 3. Set Up the Database

Create the database and schema using the provided SQL script:

```bash
# Using clickhouse-client
clickhouse-client --multiquery < scripts/setup_db.sql

# Or using curl
curl -X POST http://localhost:8123 --data-binary @scripts/setup_db.sql
```

Verify the tables were created:

```bash
clickhouse-client --query "SHOW TABLES FROM lazarus_research"
```

Expected output:
```
crypto_addresses
documents
incidents
iocs
processing_log
```

### 4. Configure the Application

Create a configuration file from the example:

```bash
cp config/default.toml config/local.toml
```

Edit `config/local.toml` to match your environment:

```toml
[repository]
source_url = "https://github.com/tayvano/lazarus-bluenoroff-research"
local_path = "./data_repo"
branch = "main"
sync_on_start = true

[database]
host = "localhost"
port = 8123
database = "lazarus_research"
batch_size = 1000

[pipeline]
parallel_workers = 4
skip_patterns = ["*.zip", "*.pdf", ".git/*"]
force_reprocess = false
max_file_size_mb = 10

[extraction]
extract_crypto_addresses = true
extract_incidents = true
extract_iocs = true
normalize_markdown = true
```

### 5. Build the Application

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release
```

## Usage

### Basic Commands

```bash
# Show help
cargo run -- --help

# Verify database schema
cargo run -- verify

# Create schema if missing
cargo run -- verify --create-schema

# Sync repository only
cargo run -- sync

# Run full ingestion pipeline
cargo run -- ingest

# Force reprocess all files
cargo run -- ingest --force

# Show database statistics
cargo run -- stats

# Reset database (WARNING: deletes all data)
cargo run -- reset --confirm
```

### Using the Release Binary

```bash
# Build release version
cargo build --release

# Run from target directory
./target/release/lazarus_ingest ingest
```

### Environment Variables

You can override configuration using environment variables:

```bash
export CLICKHOUSE_URL="http://localhost:8123"
export CLICKHOUSE_DATABASE="lazarus_research"
export CLICKHOUSE_USERNAME="default"
export CLICKHOUSE_PASSWORD=""

cargo run -- ingest
```

### Configuration Priority

The application loads configuration in this order (later sources override earlier):

1. `config/default.toml` (default settings)
2. `config/local.toml` (if exists)
3. Environment variables (prefixed with `APP_`)
4. Command-line arguments

## Workflow

### Initial Setup

```bash
# 1. Verify ClickHouse is running
curl http://localhost:8123/

# 2. Create database and schema
clickhouse-client --multiquery < scripts/setup_db.sql

# 3. Verify schema
cargo run -- verify

# 4. Run initial ingestion
cargo run --release -- ingest
```

### Regular Updates

```bash
# Sync repository and process new/changed files
cargo run --release -- ingest
```

### Development Workflow

```bash
# Set up logging for development
export RUST_LOG=debug

# Run with verbose output
cargo run -- ingest

# Process a limited number of files for testing
# (modify main.rs to add --limit flag if needed)
```

## Database Queries

### Basic Statistics

```sql
USE lazarus_research;

-- Total documents
SELECT count() FROM documents;

-- Crypto addresses by chain
SELECT chain, count()
FROM crypto_addresses
GROUP BY chain;

-- Recent incidents
SELECT title, date, victim, amount_usd
FROM incidents
ORDER BY date DESC
LIMIT 10;

-- IOCs by type
SELECT ioc_type, count()
FROM iocs
GROUP BY ioc_type;
```

### Advanced Analytics

See `scripts/example_queries.sql` for comprehensive examples:

- Document categorization
- Incident timeline analysis
- Top victims by stolen amount
- Attack vector frequency
- Cryptocurrency usage patterns
- Processing performance metrics

### Running Example Queries

```bash
# Interactive mode
clickhouse-client --database=lazarus_research

# From file
clickhouse-client --database=lazarus_research < scripts/example_queries.sql

# Web UI (if enabled)
open http://localhost:8123/play
```

## Performance Tuning

### ClickHouse Configuration

For better performance, adjust ClickHouse settings in `/etc/clickhouse-server/config.xml`:

```xml
<max_memory_usage>20000000000</max_memory_usage>
<max_threads>8</max_threads>
<max_insert_threads>4</max_insert_threads>
```

### Application Configuration

Adjust `config/local.toml`:

```toml
[pipeline]
parallel_workers = 8  # Increase for more CPU cores
batch_size = 5000     # Larger batches for bulk inserts

[database]
batch_size = 2000     # Tune based on network/memory
```

### Release Build Optimizations

The project includes aggressive release optimizations:

```toml
[profile.release]
opt-level = 3        # Maximum optimization
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization at cost of compile time
strip = true         # Remove debug symbols
```

## Troubleshooting

### ClickHouse Connection Failed

```bash
# Check if ClickHouse is running
curl http://localhost:8123/

# Check logs
tail -f /var/log/clickhouse-server/clickhouse-server.log

# Restart service
sudo service clickhouse-server restart
```

### Schema Issues

```bash
# Drop and recreate all tables
cargo run -- reset --confirm

# Or manually
clickhouse-client --query "DROP DATABASE lazarus_research"
clickhouse-client --multiquery < scripts/setup_db.sql
```

### Git Sync Failures

```bash
# Disable auto-sync and use manual repository
# In config/local.toml:
[repository]
sync_on_start = false
local_path = "/path/to/manually/cloned/repo"
```

### Out of Memory

```bash
# Reduce parallel workers
[pipeline]
parallel_workers = 2

# Reduce batch size
[database]
batch_size = 500

# Increase ClickHouse memory limit
# In /etc/clickhouse-server/users.xml:
<max_memory_usage>10000000000</max_memory_usage>
```

### File Processing Errors

Check logs for specific errors:

```bash
# Enable debug logging
export RUST_LOG=debug
cargo run -- ingest

# Check processing log in ClickHouse
SELECT * FROM processing_log WHERE status = 'failed' ORDER BY processed_at DESC;
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

# Integration tests only
cargo test --test '*'
```

### Code Style

```bash
# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings

# Check without building
cargo check
```

### Adding New Extractors

1. Create new module in `src/extractor/`
2. Implement extraction logic with regex patterns
3. Add tests
4. Update `src/extractor/mod.rs`
5. Integrate into `src/pipeline/processor.rs`

## Architecture

### Data Flow

```
Repository (GitHub)
    ↓
Git Sync
    ↓
File Scanner (*.md files)
    ↓
Parallel Processing Pool
    ↓
┌─────────────────────────┐
│  FileProcessor          │
│  ├─ Read file          │
│  ├─ Parse markdown     │
│  ├─ Extract entities   │
│  └─ Normalize content  │
└─────────────────────────┘
    ↓
Batch Insertion
    ↓
ClickHouse Database
    ↓
Analytics & Queries
```

### Key Components

- **PipelineOrchestrator**: Coordinates entire ingestion process
- **FileProcessor**: Handles individual file processing
- **ProgressTracker**: Real-time progress monitoring
- **ClickHouseClient**: Database connection and operations
- **Extractors**: Pattern-based entity extraction

## Contributing

Contributions are welcome. Please follow these guidelines:

1. Fork the repository
2. Create a feature branch
3. Write tests for new functionality
4. Ensure `cargo test` passes
5. Run `cargo fmt` and `cargo clippy`
6. Submit a pull request

## License

This project is licensed under the MIT License.

## Acknowledgments

- Threat intelligence data from https://github.com/tayvano/lazarus-bluenoroff-research
- Built with Rust and ClickHouse
- Entity extraction powered by regex patterns

## Support

For issues and questions:

- GitHub Issues: https://github.com/cipher-rc5/lazarus_ingest/issues
- Documentation: https://docs.rs/lazarus_ingest

## Roadmap

- [ ] Web UI for browsing ingested data
- [ ] Real-time monitoring dashboard
- [ ] Advanced graph analytics
- [ ] Export to STIX/MISP formats
- [ ] Machine learning for entity classification
- [ ] Multi-source repository support
