# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.6.0] - 2025-10-07

### Changed

- Upgrade Bevy to `0.17`.
- Migrate to Messages/Observers API (Bevy 0.17):
  - Derive `Message` for `ChannelPacket`, `SendChannelMessage<T>`, `ReceiveChannelMessage<T>`.
  - Replace `EventReader`/`EventWriter` with `MessageReader`/`MessageWriter`.
  - Replace `Trigger<T>` with `On<T>` observers and use `add_observer(...)`.
  - Replace `Commands::trigger_targets` with `Commands::trigger`.
  - Introduce `NodeEvent { entity, event }` as an `EntityEvent` for node-scoped notifications.
  - Replace `register_component_hooks` with `Component::on_insert` / `Component::on_remove`.
- Update examples to new API; update README accordingly.
- Pass `cargo clippy --all-targets --all-features -D warnings`.

### Notes

- Supported versions mapping updated: Bevy `0.17` → `bevy_octopus` `0.6`; Bevy `0.16` → `0.5`.


## [0.5.0] 

-- bevy bumped version to `0.16`

## [0.2.0] - 2024-07-10

### Added

- can add encoder and decoder stand alone ; 
