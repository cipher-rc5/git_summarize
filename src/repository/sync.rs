// file: src/repository/sync.rs
// description: Repository synchronization using git2
// reference: https://docs.rs/git2

use crate::config::RepositoryConfig;
use crate::error::{PipelineError, Result};
use git2::{FetchOptions, RemoteCallbacks, Repository};
use std::path::Path;
use tracing::{debug, info, warn};

pub struct RepositorySync {
    config: RepositoryConfig,
}

impl RepositorySync {
    pub fn new(config: RepositoryConfig) -> Self {
        Self { config }
    }

    pub fn sync(&self) -> Result<()> {
        let path = &self.config.local_path;

        if path.exists() {
            info!("Repository exists, pulling latest changes");
            self.pull(path)?;
        } else {
            info!("Repository does not exist, cloning");
            self.clone()?;
        }

        Ok(())
    }

    fn clone(&self) -> Result<()> {
        info!("Cloning repository from {}", self.config.source_url);

        let mut callbacks = RemoteCallbacks::new();
        callbacks.transfer_progress(|stats| {
            if stats.received_objects() == stats.total_objects() {
                debug!(
                    "Resolving deltas {}/{}",
                    stats.indexed_deltas(),
                    stats.total_deltas()
                );
            } else if stats.total_objects() > 0 {
                debug!(
                    "Received {}/{} objects",
                    stats.received_objects(),
                    stats.total_objects()
                );
            }
            true
        });

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);

        if !self.config.branch.is_empty() && self.config.branch != "main" {
            builder.branch(&self.config.branch);
        }

        builder
            .clone(&self.config.source_url, &self.config.local_path)
            .map_err(|e| PipelineError::RepositorySync(format!("Clone failed: {}", e)))?;

        info!("Repository cloned successfully");
        Ok(())
    }

    fn pull(&self, path: &Path) -> Result<()> {
        let repo = Repository::open(path)
            .map_err(|e| PipelineError::RepositorySync(format!("Failed to open repo: {}", e)))?;

        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| PipelineError::RepositorySync(format!("Failed to find remote: {}", e)))?;

        let mut callbacks = RemoteCallbacks::new();
        callbacks.transfer_progress(|stats| {
            if stats.received_objects() == stats.total_objects() {
                debug!(
                    "Resolving deltas {}/{}",
                    stats.indexed_deltas(),
                    stats.total_deltas()
                );
            }
            true
        });

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        info!("Fetching latest changes");
        remote
            .fetch(&[&self.config.branch], Some(&mut fetch_options), None)
            .map_err(|e| PipelineError::RepositorySync(format!("Fetch failed: {}", e)))?;

        let fetch_head = repo.find_reference("FETCH_HEAD").map_err(|e| {
            PipelineError::RepositorySync(format!("Failed to find FETCH_HEAD: {}", e))
        })?;

        let fetch_commit = repo
            .reference_to_annotated_commit(&fetch_head)
            .map_err(|e| PipelineError::RepositorySync(format!("Failed to get commit: {}", e)))?;

        let analysis = repo
            .merge_analysis(&[&fetch_commit])
            .map_err(|e| PipelineError::RepositorySync(format!("Merge analysis failed: {}", e)))?;

        if analysis.0.is_fast_forward() {
            info!("Fast-forward merge");
            let refname = format!("refs/heads/{}", self.config.branch);
            let mut reference = repo.find_reference(&refname).map_err(|e| {
                PipelineError::RepositorySync(format!("Failed to find reference: {}", e))
            })?;

            reference
                .set_target(fetch_commit.id(), "Fast-Forward")
                .map_err(|e| {
                    PipelineError::RepositorySync(format!("Failed to set target: {}", e))
                })?;

            repo.set_head(&refname)
                .map_err(|e| PipelineError::RepositorySync(format!("Failed to set HEAD: {}", e)))?;

            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                .map_err(|e| PipelineError::RepositorySync(format!("Checkout failed: {}", e)))?;

            info!("Repository updated successfully");
        } else if analysis.0.is_up_to_date() {
            info!("Repository is up to date");
        } else {
            warn!("Repository requires manual merge");
        }

        Ok(())
    }

    pub fn get_current_commit(&self) -> Result<String> {
        let repo = Repository::open(&self.config.local_path)
            .map_err(|e| PipelineError::RepositorySync(format!("Failed to open repo: {}", e)))?;

        let head = repo
            .head()
            .map_err(|e| PipelineError::RepositorySync(format!("Failed to get HEAD: {}", e)))?;

        let commit = head
            .peel_to_commit()
            .map_err(|e| PipelineError::RepositorySync(format!("Failed to get commit: {}", e)))?;

        Ok(commit.id().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sync_creation() {
        let temp = TempDir::new().unwrap();
        let config = RepositoryConfig {
            source_url: "https://github.com/example/repo".to_string(),
            local_path: temp.path().to_path_buf(),
            branch: "main".to_string(),
            sync_on_start: true,
        };

        let sync = RepositorySync::new(config);
        assert_eq!(sync.config.branch, "main");
    }
}
