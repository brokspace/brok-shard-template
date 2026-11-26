<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/brokspace-on-dark.svg">
    <img src=".github/assets/brokspace.svg" alt="Brokspace" height="44">
  </picture>
</p>
# brok-shard-template

A working starting point for a [Brokspace](https://github.com/brokspace/brok) **shard**: a small program
that joins the workspace event bus, reacts to events and publishes new ones.

The example shard is useful on its own: it listens to `file:changed` and publishes a `todo:found` event for
every `TODO`, `FIXME` or `HACK` marker in the changed files.

```text
brok-agent ──file:changed──▶ bus ──▶ brok-shard-template ──todo:found──▶ bus ──▶ anyone subscribed
```

New to shards? The [tutorial](https://github.com/brokspace/brok/blob/staging/docs/shard-tutorial.md) explains each idea; this README is the
short path.

## Build your own shard, step by step

You need [brok](https://brokspace.org/docs/getting-started) 0.8 or newer, [Rust](https://rustup.rs) 1.88 or
newer and git.

### 1. Create the project

```sh
brok shard new todo-alerts     # clones this template and renames it everywhere
cd todo-alerts
cargo test
```

Or click **Use this template** on GitHub, then replace `brok-shard-template` in `Cargo.toml`, `shard.json`
and the `BIN` variable of `.github/workflows/release.yml`. Names match `^[a-z][a-z0-9-]{1,63}$`.
`cargo test` fails if the manifest and the code disagree.

| File | What you change |
|---|---|
| `src/scanner.rs` | Your logic: plain functions with unit tests, no bus involved. |
| `src/payload.rs` | The events you publish: one struct per topic. |
| `src/main.rs` | The wiring: connect, subscribe, handle events, exit on `system:shutdown`. |
| `shard.json` | Name, permissions, topics and how to build. |

### 2. Run it in a workspace

From a project you can experiment in (any git repository):

```sh
cd ~/my-project
brok init                                   # once per project
brok shard install ../todo-alerts           # shows the permissions and asks before installing
brok start                                  # bus, brok-agent and your shard, in the background
```

In another terminal watch your events:

```sh
brok bus tail 'todo:*'
```

Then trigger the shard, either by editing a file (brok-agent publishes `file:changed`) or by publishing the
event yourself:

```sh
echo '// TODO: try brokspace' >> src/main.rs
brok bus emit file:changed '{"changed":["src/main.rs"],"deleted":[]}'
```

You should see something like:

```text
todo:found from todo-alerts {"line":2,"marker":"TODO","path":"src/main.rs","text":"try brokspace"}
```

### 3. Change it

Edit the code, then reinstall and restart:

```sh
brok shard install ../todo-alerts --force   # --force: same version, new build
brok restart
```

Your shard's log (everything it writes to stderr) is in `.brokspace/logs/todo-alerts.log`. For more detail,
stop the background brok and run `RUST_LOG=debug brok run`.

To make something configurable, read your settings from `brok.yml`:

```yaml
shards:
  todo-alerts:
    settings:
      markers: [TODO, FIXME, XXX]
```

```rust
// In `run()`, after connecting: `settings` is your `shards.<name>.settings` section.
let markers: Vec<String> = client
    .context()
    .settings
    .get("markers")
    .and_then(|value| value.as_array())
    .map(|list| list.iter().filter_map(|m| m.as_str().map(str::to_owned)).collect())
    .unwrap_or_default();
```

To react to other events or publish new ones, change the code **and** `shard.json`: the bus rejects any
topic the manifest does not list (see [Capabilities](#capabilities)).

### 4. Check it

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### 5. Publish it

1. Push your repository, then a tag: `git tag v0.1.0 && git push --tags`. The release workflow builds Linux,
   macOS and Windows binaries and attaches them with `.sha256` checksums.
2. Copy the `releases` block from the workflow summary into `shard.json` and commit it.
3. Anyone can now install it. brok downloads the binary for their platform and verifies the checksum, or
   builds from source:

```sh
brok shard install https://github.com/you/todo-alerts --tag v0.1.0
```

## When something does not work

| You see | Why and what to do |
|---|---|
| `requires brok >=…, this is brok …` | The `brok` range in `shard.json` excludes your version. Widen it. |
| Nothing arrives | Is brok running (`brok status`)? Is the topic in `subscribes_to`? Publish the event with `brok bus emit` to test without brok-agent. |
| A publish or subscribe is rejected | Add the topic to `publishes_to` or `subscribes_to`, reinstall and restart. |
| A variable is empty inside the shard | The environment is scrubbed: list it in `permissions.env`. |
| Your change has no effect | Reinstall with `--force`, then `brok restart`. |

## The manifest (`shard.json`)

```json
{
  "name": "brok-shard-template",
  "version": "0.1.0",
  "brok": ">=0.8.0, <1.0.0",
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
