# AGENTS.md

Operating guide for coding agents and contributors working in `brokspace/brok-shard-template`: the starting point for writing your own Brokspace shard. It is a GitHub template repository containing one small, working shard (a TODO/FIXME scanner that reacts to `file:changed` and publishes `todo:found`).

If you are an agent helping someone **build their own shard from this template**, read the README first: it is the step-by-step guide. Rename the crate in `Cargo.toml`, the `name`, `entrypoint` and `build.artifact` in `shard.json`, and the `BIN` variable in `.github/workflows/release.yml` together.

If you are changing **the template itself**, keep it minimal: one shard that teaches the moving parts (manifest, capabilities, subscribing, a custom payload, tests, CI, releases). Larger examples belong in their own repositories. See [brok's SCOPE.md](https://github.com/brokspace/brok/blob/main/SCOPE.md).

## Layout

```text
src/
  main.rs      connect to the bus, subscribe, handle events, exit on system:shutdown
  payload.rs   the custom TodoFound payload (topic todo:found)
  scanner.rs   pure scanning logic, unit tested
shard.json     manifest: identity, capabilities, build command, releases
```

## Build and test

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
brok shard install .   # install into the current Brokspace workspace for a real run
brok run -v            # in another terminal: brok bus tail 'todo:*'
```

The template depends on `brok-sdk` from crates.io. For polyrepo development of Brokspace itself, check out [brok-sdk](https://github.com/brokspace/brok-sdk) next to this repository and run `brok/scripts/bootstrap.sh`.

## Conventions

- Shards log to stderr; `brok run` prefixes each line with the shard name.
- Request only the capabilities the shard needs in `shard.json`; the user sees them at install time.
- Exit promptly on `system:shutdown` and when the inbox closes.
- Keep logic in pure, unit-tested functions; `main.rs` only wires the bus.
- Commit messages and PR titles follow Conventional Commits.
- No em-dashes in prose.

## Git workflow

Every change reaches `main` through `staging`, the default branch:

```text
your branch --PR, squash--> staging --release PR, merge commit--> main --> tag
```

1. Branch from staging: `git fetch origin && git switch -c <type>/<topic> origin/staging` (for example `feat/kotlin-chunker`, `fix/empty-config`).
2. Commit in small steps with Conventional Commits. End every commit message with a `Co-Authored-By:` trailer naming the agent, for example `Co-Authored-By: Claude <noreply@anthropic.com>`: the PR gets the `ai-agent` label from it.
3. Before pushing, run the fmt, clippy and test commands from "Build and test" above.
4. Push, open the PR against staging with the template filled in, and turn on auto-merge:

   ```sh
   git push -u origin HEAD
   gh pr create --base staging --fill
   gh pr merge --auto --squash
   ```

   The PR merges itself once `ubuntu-latest`, `macos-latest`, `windows-latest` and `policy / title` pass and review conversations are resolved. No approval is needed.
5. A check failed? Read it (`gh pr checks --watch`, `gh run view --log-failed`), fix it on the same branch and push; auto-merge stays on. Cannot fix it? Say what you tried in a PR comment and stop. Never weaken or skip a check to get green.

Never push to `staging` or `main` (both reject it), force-push a branch someone else pushed to, open or merge PRs into `main`, or merge with `--admin`. The release PR into `main` is opened by the Promote workflow and merged by a maintainer.

Do not commit a `Cargo.lock` change that only adds `[[patch.unused]]`: it comes from the local `../.cargo/config.toml` patch, and CI rejects it. Restore the file (`git restore Cargo.lock`) before committing.

## Agent skills configuration

- **Issue tracker**: GitHub Issues on [brokspace/brok-shard-template](https://github.com/brokspace/brok-shard-template/issues) via `gh`. See [docs/agents/issue-tracker.md](docs/agents/issue-tracker.md).
- **Triage labels**: [docs/agents/triage-labels.md](docs/agents/triage-labels.md). Issues are judged against [brok's SCOPE.md](https://github.com/brokspace/brok/blob/main/SCOPE.md).
- **Domain docs**: the shared glossary is [brok's GLOSSARY.md](https://github.com/brokspace/brok/blob/main/GLOSSARY.md). The `write-shard` built-in skill (`brok skill show write-shard`) teaches agents how to build shards.
