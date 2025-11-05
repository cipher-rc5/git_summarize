// file: src/exporter/json.rs
// description: json export utilities for clickhouse data

use crate::database::client::ClickHouseClient;
use crate::error::{PipelineError, Result};
use crate::models::{CryptoAddress, Document, Incident, Ioc};
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct JsonExporter {
    output_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct ExportedDocument {
    #[serde(flatten)]
    pub document: Document,
    pub incidents: Vec<Incident>,
    pub crypto_addresses: Vec<CryptoAddress>,
    pub iocs: Vec<Ioc>,
}

#[derive(Debug, Serialize)]
pub struct ExportManifest {
    pub exported_at: String,
    pub total_documents: usize,
    pub total_incidents: usize,
    pub total_addresses: usize,
    pub total_iocs: usize,
    pub files: Vec<String>,
}

impl JsonExporter {
    pub fn new(output_dir: impl Into<PathBuf>) -> Result<Self> {
        let output_dir = output_dir.into();
        fs::create_dir_all(&output_dir)?;
        Ok(Self { output_dir })
    }

    pub async fn export_all(
        &self,
        client: &ClickHouseClient,
        pretty: bool,
    ) -> Result<ExportManifest> {
        info!("Starting JSON export to {:?}", self.output_dir);

        let documents = self.fetch_all_documents(client).await?;
        let total_docs = documents.len();

        info!("Exporting {} documents", total_docs);

        let mut manifest = ExportManifest {
            exported_at: Utc::now().to_rfc3339(),
            total_documents: total_docs,
            total_incidents: 0,
            total_addresses: 0,
            total_iocs: 0,
            files: Vec::new(),
        };

        for (index, document) in documents.into_iter().enumerate() {
            let file_path = document.file_path.clone();
            let incidents = self
                .fetch_incidents_for_document(client, &file_path)
                .await?;
            let addresses = self
                .fetch_addresses_for_document(client, &file_path)
                .await?;
            let iocs = self.fetch_iocs_for_document(client, &file_path).await?;

            manifest.total_incidents += incidents.len();
            manifest.total_addresses += addresses.len();
            manifest.total_iocs += iocs.len();

            let exported = ExportedDocument {
                document,
                incidents,
                crypto_addresses: addresses,
                iocs,
            };

            let filename = format!("document_{:06}.json", index + 1);
            self.write_json_file(&filename, &exported, pretty)?;
            manifest.files.push(filename);

            if (index + 1) % 100 == 0 {
                debug!("Exported {}/{} documents", index + 1, total_docs);
            }
        }

        self.write_json_file("manifest.json", &manifest, true)?;

        info!(
            "Export complete: {} documents, {} incidents, {} addresses, {} IOCs",
            manifest.total_documents,
            manifest.total_incidents,
            manifest.total_addresses,
            manifest.total_iocs
        );

        Ok(manifest)
    }

    pub async fn export_filtered(
        &self,
        client: &ClickHouseClient,
        query: &str,
        pretty: bool,
    ) -> Result<usize> {
        info!("Exporting filtered results with custom query");

        let documents: Vec<Document> = client
            .get_client()
            .query(query)
            .fetch_all::<Document>()
            .await?;

        for (index, document) in documents.iter().enumerate() {
            let filename = format!("filtered_{:06}.json", index + 1);
            self.write_json_file(&filename, document, pretty)?;
        }

        Ok(documents.len())
    }

    pub async fn export_single(
        &self,
        client: &ClickHouseClient,
        content_hash: &str,
        pretty: bool,
    ) -> Result<()> {
        let document = self.fetch_document_by_hash(client, content_hash).await?;
        let file_path = document.file_path.clone();
        let incidents = self
            .fetch_incidents_for_document(client, &file_path)
            .await?;
        let addresses = self
            .fetch_addresses_for_document(client, &file_path)
            .await?;
        let iocs = self.fetch_iocs_for_document(client, &file_path).await?;

        let exported = ExportedDocument {
            document,
            incidents,
            crypto_addresses: addresses,
            iocs,
        };

        let filename = format!("{}.json", content_hash);
        self.write_json_file(&filename, &exported, pretty)?;

        info!("Exported document to {}", filename);
        Ok(())
    }

    fn write_json_file<T: Serialize>(&self, filename: &str, data: &T, pretty: bool) -> Result<()> {
        let path = self.output_dir.join(filename);
        let json = if pretty {
            serde_json::to_string_pretty(data)
                .map_err(|e| PipelineError::Serialization(e.to_string()))?
        } else {
            serde_json::to_string(data).map_err(|e| PipelineError::Serialization(e.to_string()))?
        };

        fs::write(path, json)?;
        Ok(())
    }

    async fn fetch_all_documents(&self, client: &ClickHouseClient) -> Result<Vec<Document>> {
        let documents: Vec<Document> = client
            .get_client()
            .query(
                "SELECT file_path,\
                        relative_path,\
                        content,\
                        content_hash,\
                        file_size,\
                        last_modified,\
                        parsed_at,\
                        normalized \
                 FROM documents ORDER BY file_path",
            )
            .fetch_all::<Document>()
            .await?;

        Ok(documents)
    }

    async fn fetch_document_by_hash(
        &self,
        client: &ClickHouseClient,
        content_hash: &str,
    ) -> Result<Document> {
        client
            .get_client()
            .query(
                "SELECT file_path,\
                        relative_path,\
                        content,\
                        content_hash,\
                        file_size,\
                        last_modified,\
                        parsed_at,\
                        normalized \
                 FROM documents WHERE content_hash = ? LIMIT 1",
            )
            .bind(content_hash)
            .fetch_one::<Document>()
            .await
            .map_err(PipelineError::Database)
    }

    async fn fetch_incidents_for_document(
        &self,
        client: &ClickHouseClient,
        file_path: &str,
    ) -> Result<Vec<Incident>> {
        let incidents: Vec<Incident> = client
            .get_client()
            .query(
                "SELECT document_id,\
                        title,\
                        date,\
                        date_precision,\
                        victim,\
                        attack_vector,\
                        amount_usd,\
                        description,\
                        source_file,\
                        extracted_at \
                 FROM incidents WHERE source_file = ?",
            )
            .bind(file_path)
            .fetch_all::<Incident>()
            .await?;

        Ok(incidents)
    }

    async fn fetch_addresses_for_document(
        &self,
        client: &ClickHouseClient,
        file_path: &str,
    ) -> Result<Vec<CryptoAddress>> {
        let addresses: Vec<CryptoAddress> = client
            .get_client()
            .query(
                "SELECT address,\
                        chain,\
                        document_id,\
                        file_path,\
                        context,\
                        attribution,\
                        parsed_at \
                 FROM crypto_addresses WHERE file_path = ?",
            )
            .bind(file_path)
            .fetch_all::<CryptoAddress>()
            .await?;

        Ok(addresses)
    }

    async fn fetch_iocs_for_document(
        &self,
        client: &ClickHouseClient,
        file_path: &str,
    ) -> Result<Vec<Ioc>> {
        if let Some(document_id) = self.resolve_document_id(client, file_path).await? {
            let iocs: Vec<Ioc> = client
                .get_client()
                .query(
                    "SELECT ioc_type,\
                            value,\
                            document_id,\
                            context,\
                            extracted_at \
                     FROM iocs WHERE document_id = ?",
                )
                .bind(document_id)
                .fetch_all::<Ioc>()
                .await?;

            Ok(iocs)
        } else {
            Ok(Vec::new())
        }
    }

    async fn resolve_document_id(
        &self,
        client: &ClickHouseClient,
        file_path: &str,
    ) -> Result<Option<String>> {
        let from_incidents: Vec<String> = client
            .get_client()
            .query("SELECT document_id FROM incidents WHERE source_file = ? LIMIT 1")
            .bind(file_path)
            .fetch_all::<String>()
            .await?;

        if let Some(id) = from_incidents.into_iter().next()
            && !id.is_empty()
        {
            return Ok(Some(id));
        }

        let from_addresses: Vec<String> = client
            .get_client()
            .query("SELECT document_id FROM crypto_addresses WHERE file_path = ? LIMIT 1")
            .bind(file_path)
            .fetch_all::<String>()
            .await?;

        if let Some(id) = from_addresses.into_iter().next()
            && !id.is_empty()
        {
            return Ok(Some(id));
        }

        Ok(None)
    }
}
