//! Events this shard publishes.
//!
//! Define one struct per topic and bind it with [`Payload`]: the SDK then
//! checks topics for you in `Event::from_payload` and `Event::decode`.
//! Every topic you publish must be listed in `shard.json` `publishes_to`.

use brok_sdk::Payload;
use serde::{Deserialize, Serialize};

/// A TODO / FIXME / HACK marker found in a changed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoFound {
    /// Workspace relative path (`/` separators).
    pub path: String,
    /// 1-based line number.
    pub line: u32,
    /// `TODO`, `FIXME` or `HACK`.
    pub marker: String,
    /// The note after the marker.
    pub text: String,
}

impl Payload for TodoFound {
    const TOPIC: &'static str = "todo:found";
}
