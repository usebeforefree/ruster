use crate::{Args, dir::DirArgs};

use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone, clap::Subcommand)]
pub(crate) enum Subommand {
    /// Uses directory/file enumeration mode
    Dir(DirArgs),
    ///Uses VHOST enumeration mode (you most probably want to use the IP address as the URL parameter)
    Vhost,
    /// Uses DNS subdomain enumeration mode
    Dns,
    /// Uses fuzzing mode. Replaces the keyword FUZZ in the URL, Headers and the request body
    Fuzz,
    /// Uses TFTP enumeration mode
    Tftp,
    /// Uses aws bucket enumeration mode
    S3,
    /// Uses gcs bucket enumeration mode
    Qcs,
}

#[async_trait]
pub(crate) trait Subcommand: Send + Sync {
    async fn pre_run(&self, _args: &Args) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn process_word(
        &self,
        args: &Args,
        word: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub(crate) fn to_subcommand(cmd: Subommand) -> Result<Arc<dyn Subcommand>, String> {
    match cmd {
        Subommand::Dir(dir_args) => Ok(Arc::new(dir_args)),
        _ => Err(format!("Subcommand is not implemented yet")),
    }
}
