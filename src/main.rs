//! A Brokspace shard that reports TODO / FIXME / HACK markers in changed files.
//!
//! The shape of every shard is the same:
//!
//! 1. connect to the bus with the credentials `brok run` injects,
//! 2. subscribe to the topics declared in `shard.json`,
//! 3. loop over the inbox, react, publish,
//! 4. exit on `system:shutdown` or when the connection closes.

mod payload;
mod scanner;

use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

use anyhow::Context;
use brok_sdk::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::payload::TodoFound;

/// Must match `name` in shard.json.
const NAME: &str = env!("CARGO_PKG_NAME");

#[tokio::main]
async fn main() -> ExitCode {
    // Logs go to stderr; `brok run` captures them line by line.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_target(false)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let (client, mut inbox) =
        BrokClient::connect_from_env(NAME, env!("CARGO_PKG_VERSION")).await.context("cannot join the brok bus")?;
    let root = client.context().workspace_root.clone();
    client.subscribe([TopicPattern::new(topics::FILE_CHANGED)?]).await?;
    tracing::info!(workspace = %root.display(), "{NAME} ready");

    loop {
        tokio::select! {
            event = inbox.recv() => match event {
                None => break,
                Some(event) if event.is::<Shutdown>() => break,
                Some(event) if event.is::<FileChanged>() => on_file_changed(&client, &root, event.decode()?).await?,
                Some(event) => tracing::debug!(topic = %event.topic, "ignoring event"),
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}

/// Scans every changed file and publishes one `todo:found` event per marker.
async fn on_file_changed(client: &BrokClient, root: &Path, change: FileChanged) -> anyhow::Result<()> {
    let root = root.to_path_buf();
    // File I/O is blocking: keep it off the async runtime threads.
    let found = tokio::task::spawn_blocking(move || {
        change
            .changed
            .iter()
            .flat_map(|path| {
                scanner::scan_file(&root, path).unwrap_or_else(|e| {
                    tracing::warn!(path, error = %e, "cannot scan file");
                    Vec::new()
                })
            })
            .collect::<Vec<TodoFound>>()
    })
    .await?;

    for todo in &found {
        client.emit(todo).await?;
    }
    if !found.is_empty() {
        tracing::info!(markers = found.len(), "published todo:found events");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use brok_sdk::{Payload, ShardManifest, Topic};

    use super::*;

    #[test]
    fn manifest_matches_the_code() {
        let manifest = ShardManifest::from_json(include_str!("../shard.json")).unwrap();
        assert_eq!(manifest.name, NAME);
        assert_eq!(manifest.version.to_string(), env!("CARGO_PKG_VERSION"));

        // Every topic we publish or subscribe to must be granted by the manifest,
        // otherwise the bus rejects it at runtime.
        let published = Topic::new(TodoFound::TOPIC).unwrap();
        assert!(manifest.publishes_to.iter().any(|p| p.matches(&published)));
        let subscribed = Topic::new(FileChanged::TOPIC).unwrap();
        assert!(manifest.subscribes_to.iter().any(|p| p.matches(&subscribed)));
    }
}
