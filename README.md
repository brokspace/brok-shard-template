# brok-shard-template

A ready-to-fork starting point for building a [Brokspace](https://github.com/brokspace/brok) **shard**: a
small standalone program that joins the workspace event bus, reacts to events and publishes new ones.

The example shard is useful on its own: it listens to `file:changed` and publishes a `todo:found` event for
every `TODO`, `FIXME` or `HACK` marker in the changed files.

```text
brok-agent ──file:changed──▶ bus ──▶ brok-shard-template ──todo:found──▶ bus ──▶ anyone subscribed
```

## Build your own shard in 5 minutes

### 1. Create your repository

Click **Use this template** on GitHub (or clone this repository), and clone
[`brok-sdk`](https://github.com/brokspace/brok-sdk) next to it — until the SDK is on crates.io it is a path
dependency:

```text
my-shards/
├── brok-sdk/
└── brok-todo-alerts/      ← your copy of this template
```

### 2. Rename it

Pick a name matching `^[a-z][a-z0-9-]{1,63}$` and replace `brok-shard-template` in:

| File | Field |
|---|---|
| `Cargo.toml` | `name`, `description`, `repository` |
| `shard.json` | `name`, `description`, `repository`, `entrypoint` (`bin/<name>`), `build.artifact` (`target/release/<name>`) |
| `.github/workflows/release.yml` | `BIN` |

`cargo test` checks that the manifest and the code agree, so a missed spot fails fast.

### 3. Implement your handler

The code is split so that the interesting part stays easy to test:

| File | Role |
|---|---|
| `src/scanner.rs` | Your domain logic: plain synchronous functions with unit tests. |
| `src/payload.rs` | The events you publish: one struct per topic, bound with `brok_sdk::Payload`. |
| `src/main.rs` | Wiring: connect, subscribe, loop over the inbox, publish. |

The whole event loop is a dozen lines:

```rust
let (client, mut inbox) = BrokClient::connect_from_env(NAME, env!("CARGO_PKG_VERSION")).await?;
client.subscribe([TopicPattern::new("file:changed")?]).await?;

while let Some(event) = inbox.recv().await {
    if event.is::<Shutdown>() { break; }
    if let Ok(change) = event.decode::<FileChanged>() {
        for todo in scan(&change) {
            client.emit(&todo).await?;            // published on TodoFound::TOPIC
        }
    }
}
```

Other things the client can do:

- `client.context()` — workspace root, your `settings` from `brok.yml`, the LLM routing table.
- `client.call::<SearchRequest, SearchResults>(&req, timeout)` — typed request/reply, e.g. hybrid search
  through the core index.
- `client.reply(&request, &payload)` — answer a request; replies go only to the requester.

Update `shard.json` whenever you subscribe or publish a new topic — the bus enforces it.

### 4. Try it in a workspace

```sh
cd ~/my-project            # any directory initialized with `brok init`
brok shard install ../brok-todo-alerts     # builds with the manifest's build command
brok run                                   # starts the bus, brok-agent and your shard
```

In another terminal, watch your events and trigger some:

```sh
brok bus tail 'todo:*'
echo "// TODO: try brokspace" >> src/main.rs
```

Logs written to stderr appear in the `brok run` output, prefixed with your shard name. Set
`RUST_LOG=debug` for more.

### 5. Publish it

1. Push a tag (`git tag v0.1.0 && git push --tags`). The release workflow builds Linux, macOS and Windows
   binaries and attaches them with `.sha256` checksums.
2. Copy the `releases` block printed in the workflow summary into `shard.json` and commit it.
3. Anyone can now install your shard; `brok` downloads the binary for their platform and verifies its
   checksum, or falls back to building from source:

```sh
brok shard install https://github.com/you/brok-todo-alerts --tag v0.1.0
```

## The manifest (`shard.json`)

```json
{
  "name": "brok-shard-template",
  "version": "0.1.0",
  "brok": ">=0.1.0, <0.2.0",
  "entrypoint": "bin/brok-shard-template",
  "runtime": "native",
  "permissions": {
    "filesystem": { "read": ["."], "write": [] },
    "network": false
  },
  "subscribes_to": ["file:changed"],
  "publishes_to": ["todo:found"],
  "build": { "command": ["cargo", "build", "--release", "--locked"], "artifact": "target/release/brok-shard-template" }
}
```

| Field | Meaning |
|---|---|
| `name`, `version` | Identity. `version` is semver and must match `Cargo.toml` (tested). |
| `brok` | Compatible runtime versions (semver requirement). |
| `entrypoint` | Executable path inside the installed shard directory; `.exe` is added on Windows. |
| `args` | Extra command line arguments (optional). |
| `permissions` | Capabilities, all denied by default — see below. |
| `subscribes_to` / `publishes_to` | Topic patterns the shard may receive / send. |
| `build` | How to compile from source: a program and its arguments (no shell) plus the produced artifact. |
| `releases` | Prebuilt binaries per platform (`linux-x86_64`, `macos-aarch64`, `windows-x86_64`, ...), each with an `https` URL and a SHA-256. |

The schema and its validation rules are documented in
[`brok_sdk::manifest`](https://docs.rs/brok-sdk/latest/brok_sdk/manifest/).

## Capabilities

Shards follow least privilege. Everything not requested is denied, and `brok shard install` shows the list
and asks for consent before installing.

| Capability | Example | Effect |
|---|---|---|
| `filesystem.read` / `write` | `["src/"]` | Workspace relative paths the shard may read / write. Absolute paths and `..` are rejected. |
| `network` | `false` | Whether the shard may open network connections. |
| `process` | `["git"]` | Programs the shard may spawn. |
| `env` | `["*_API_KEY"]` | Environment variables passed through; everything else is scrubbed. `BROK_*` is reserved. |
| `subscribes_to` | `["file:*"]` | Enforced by the bus: other subscriptions are rejected. |
| `publishes_to` | `["todo:found"]` | Enforced by the bus: other publications are rejected. The `system:` namespace is reserved. |

The bus enforces topic permissions on every frame and stamps each event with the authenticated shard name,
so receivers can trust `event.source`.

## Topic naming

Topics look like `namespace:name`: lowercase letters, digits, `-`, `_` and `.`, where a dot creates
sub-topics (`agent:ask.chunk`). Patterns may end with `*`: `todo:*` matches every topic in your namespace,
`agent:ask.*` every sub-topic of `agent:ask`.

- Use your own namespace for your events (`todo:found`, `security:alert`).
- Subscribe to well-known topics from the SDK (`brok_sdk::payload::topics`): `file:changed`,
  `index:updated`, `review:result`, ...
- `system:*` events (such as `system:shutdown`) are delivered to every shard without subscribing.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your
option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
