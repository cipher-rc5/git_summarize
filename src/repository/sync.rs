// file: src/repository/sync.rs
// description: repository synchronization using gix (pure rust git implementation)
// reference: https://docs.rs/gix

use crate::config::RepositoryConfig;
use crate::error::{PipelineError, Result};
use gix::remote::Name;
use gix::repository::merge_base;
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

        let mut prepare =
            gix::prepare_clone(self.config.source_url.clone(), &self.config.local_path)?;

        if !self.config.branch.is_empty() && self.config.branch != "main" {
            let branch_ref = <&gix::refs::PartialNameRef>::try_from(self.config.branch.as_str())
                .map_err(|e| {
                    PipelineError::RepositorySync(format!("Invalid branch name: {}", e))
                })?;
            prepare = prepare
                .with_ref_name(Some(branch_ref))
                .expect("with_ref_name should not fail");
        }

        let (mut checkout, _outcome) = prepare
            .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
            .map_err(|e| {
                PipelineError::RepositorySync(format!("Failed to fetch and checkout: {}", e))
            })?;

        let (_repo, _outcome) = checkout
            .main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
            .map_err(|e| {
                PipelineError::RepositorySync(format!("Failed to checkout main worktree: {}", e))
            })?;

        info!("Repository cloned successfully");
        Ok(())
    }

    fn pull(&self, path: &Path) -> Result<()> {
        let repo = gix::open(path)?;

        let remote = repo.find_fetch_remote(None).map_err(|e| {
            PipelineError::RepositorySync(format!("Failed to resolve remote: {}", e))
        })?;

        info!("Fetching latest changes");

        let outcome = remote
            .connect(gix::remote::Direction::Fetch)?
            .prepare_fetch(gix::progress::Discard, Default::default())?
            .receive(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
            .map_err(|e| {
                PipelineError::RepositorySync(format!("Failed to fetch from remote: {}", e))
            })?;

        debug!("Fetched {} refs", outcome.ref_map.mappings.len());

        let remote_name = remote_symbolic_name(&remote).unwrap_or_else(|| "origin".to_string());

        let local_branch_ref = format!("refs/heads/{}", self.config.branch);
        let mut local_ref = repo.find_reference(&local_branch_ref).map_err(|e| {
            PipelineError::GitReference(format!(
                "Failed to open local branch {local_branch_ref}: {e}"
            ))
        })?;
        let local_commit_id = local_ref
            .peel_to_id()
            .map_err(|e| {
                PipelineError::GitReference(format!(
                    "Failed to peel local branch {local_branch_ref}: {e}"
                ))
            })?
            .detach();

        let remote_branch_ref = format!("refs/remotes/{}/{}", remote_name, self.config.branch);
        let mut remote_ref = repo.find_reference(&remote_branch_ref).map_err(|e| {
            PipelineError::GitReference(format!(
                "Failed to open remote tracking branch {remote_branch_ref}: {e}"
            ))
        })?;
        let remote_commit_id = remote_ref
            .peel_to_id()
            .map_err(|e| {
                PipelineError::GitReference(format!(
                    "Failed to peel remote tracking branch {remote_branch_ref}: {e}"
                ))
            })?
            .detach();

        if local_commit_id == remote_commit_id {
            info!("Repository is up to date");
            return Ok(());
        }

        let is_fast_forward = match repo.merge_base(remote_commit_id, local_commit_id) {
            Ok(base) => base == local_commit_id,
            Err(merge_base::Error::NotFound { .. }) => {
                warn!("No merge base found; manual merge required");
                false
            }
            Err(err) => {
                return Err(PipelineError::RepositorySync(format!(
                    "Failed to compute merge-base: {err}"
                )));
            }
        };

        if is_fast_forward {
            info!("Fast-forward merge");

            local_ref
                .set_target_id(remote_commit_id, "Fast-forward")
                .map_err(|e| {
                    PipelineError::RepositorySync(format!(
                        "Failed to update local branch {local_branch_ref}: {e}"
                    ))
                })?;

            info!("Repository updated successfully");
        } else {
            warn!("Repository requires manual merge");
        }

        Ok(())
    }

    pub fn get_current_commit(&self) -> Result<String> {
        let repo = gix::open(&self.config.local_path)?;

        let mut head = repo.head()?;

        let commit = head.peel_to_commit().map_err(|e| {
            PipelineError::RepositorySync(format!("Failed to peel HEAD to commit: {}", e))
        })?;

        Ok(commit.id().to_string())
    }
}

fn remote_symbolic_name(remote: &gix::Remote<'_>) -> Option<String> {
    match remote.name()? {
        Name::Symbol(symbol) => Some(symbol.to_string()),
        Name::Url(_) => None,
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
