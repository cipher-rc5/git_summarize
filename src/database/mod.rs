// file: src/database/mod.rs
// description: database operations module exports
// reference: internal module structure

pub mod client;
pub mod insert;
pub mod schema;

pub use client::LanceDbClient;
pub use insert::{BatchInserter, InsertStats};
pub use schema::SchemaManager;
