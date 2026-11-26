# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- The scaffold installs on current brok: `shard.json` accepts brok 0.8 up to 1.0 (it required brok below 0.2, so `brok shard new` produced a shard that could not be installed), and uses `brok-sdk` 0.3.

### Changed

- The README is a verified step-by-step guide with troubleshooting, and CI installs the shard on the latest brok release and waits for its event.

## [0.1.0] - Unreleased

### Added

- Example shard that subscribes to `file:changed` and publishes `todo:found` for every TODO, FIXME and
  HACK marker found in changed files.
- Minimal-permission `shard.json`, CI and release workflows that print the `releases` block for the manifest.
