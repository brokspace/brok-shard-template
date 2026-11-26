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
    let markers = report_markers(root, change.changed, async |todo| {
        client.emit(&todo).await?;
        Ok(())
    })
    .await?;
    if markers > 0 {
        tracing::info!(markers, "published todo:found events");
    }
    Ok(())
}

/// Scans `paths` one at a time and hands each marker to `publish` before the
/// next file is read: one event can list thousands of files (a branch switch),
/// so results are never collected for the whole batch. Returns their number.
async fn report_markers(
    root: &Path,
    paths: Vec<String>,
    mut publish: impl AsyncFnMut(TodoFound) -> anyhow::Result<()>,
) -> anyhow::Result<usize> {
    let mut markers = 0;
    for path in paths {
        let root = root.to_path_buf();
        // File I/O is blocking: keep it off the async runtime threads.
        let found = tokio::task::spawn_blocking(move || {
            scanner::scan_file(&root, &path).unwrap_or_else(|e| {
                tracing::warn!(path, error = %e, "cannot scan file");
                Vec::new()
            })
        })
        .await?;
        for todo in found {
            publish(todo).await?;
            markers += 1;
        }
    }
    Ok(markers)
}

#[cfg(test)]
mod tests {
    use brok_sdk::{Payload, ShardManifest, Topic};

    use super::*;

    #[tokio::test]
    async fn a_large_batch_is_published_file_by_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<String> = (0..500).map(|i| format!("f{i}.rs")).collect();
        for path in &paths {
            std::fs::write(dir.path().join(path), "// TODO: before\n// FIXME: before\n").unwrap();
        }
        // An unreadable path (a directory) and a missing one do not stop the rest.
        std::fs::create_dir(dir.path().join("folder.rs")).unwrap();
        let mut batch = vec!["folder.rs".to_owned(), "missing.rs".to_owned()];
        batch.extend(paths);

        let mut published = Vec::new();
        let count = report_markers(dir.path(), batch, async |todo| {
            // Rewrite the next file while publishing this one: its new text is
            // reported only if it had not been read yet.
            let number: usize = todo.path.trim_start_matches('f').trim_end_matches(".rs").parse()?;
            if number + 1 < 500 {
                let next = dir.path().join(format!("f{}.rs", number + 1));
                std::fs::write(next, "// TODO: after\n// FIXME: after\n")?;
            }
            published.push((todo.path, todo.marker, todo.text));
            Ok(())
        })
        .await
        .unwrap();

        assert_eq!(count, 1_000);
        assert_eq!(published.len(), 1_000);
        assert_eq!(published[0], ("f0.rs".to_owned(), "TODO".to_owned(), "before".to_owned()));
        assert!(published[2..].iter().all(|(_, _, text)| text == "after"), "a file was read ahead of its turn");
        assert_eq!(published[999].0, "f499.rs");
    }

    #[test]
    fn manifest_matches_the_code() {
        let manifest = ShardManifest::from_json(include_str!("../shard.json")).unwrap();
        assert_eq!(manifest.name, NAME);
        assert_eq!(manifest.version.to_string(), env!("CARGO_PKG_VERSION"));

        // The scaffold must install on the current brok: keep the `brok` requirement
        // in shard.json up to date (`brok shard install` refuses a shard that excludes it).
        let requirement = manifest.brok.as_ref().expect("shard.json declares the brok versions it supports");
        assert!(requirement.matches(&"0.8.0".parse().unwrap()), "{requirement} excludes the current brok");

        // Every topic we publish or subscribe to must be granted by the manifest,
        // otherwise the bus rejects it at runtime.
        let published = Topic::new(TodoFound::TOPIC).unwrap();
        assert!(manifest.publishes_to.iter().any(|p| p.matches(&published)));
        let subscribed = Topic::new(FileChanged::TOPIC).unwrap();
        assert!(manifest.subscribes_to.iter().any(|p| p.matches(&subscribed)));
    }
}
