[![crates.io](https://img.shields.io/crates/v/bevy_octopus)](https://crates.io/crates/bevy_octopus)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/Seldom-SE/seldom_pixel#license)
[![crates.io](https://img.shields.io/crates/d/bevy_octopus)](https://crates.io/crates/bevy_octopus)
[![CI](https://github.com/foxzool/bevy_octopus/workflows/CI/badge.svg)](https://github.com/foxzool/bevy_octopus/actions)
[![Documentation](https://docs.rs/bevy_octopus/badge.svg)](https://docs.rs/bevy_octopus)

# bevy_octopus

A Low-level ECS-driven network plugin for Bevy.

## Features

### ECS driven network

Every network node is a component, so you can easily manage network entities with Bevy ECS.

Apps can be many servers and many clients at the same time.

### Flexible network protocol decoder

You can define channel transformers for data serialization and deserialization.

### UDP Communication Types

Support UDP [unicast](https://github.com/foxzool/bevy_octopus/blob/main/examples/udp_send_and_recv.rs), broadcast,
multicast. [example](https://github.com/foxzool/bevy_octopus/blob/main/examples/udp_complex.rs)

### No tokio runtime

## Supported Network Protocol

| Protocol  | Server | Client | Sever with SSL | Client with SSL |
|-----------|--------|--------|----------------|-----------------|
| UDP       | ✅      | ✅      | ✘              | ✘               |
| TCP       | ✅      | ✅      | ☐              | ☐               |
| Websocket | ✅      | ✅      | ☐              | ☐               |

## Network Components

|                | ServerNode | ClientNode | NetworkPeer |
|----------------|------------|------------|-------------|
| server         | ✓          |            |             |
| client         |            | ✓          |             |
| client session |            | ✓          | ✓           |

## Supported Versions

| bevy | bevy_octopus |
|------|--------------|
| 0.14 | 0.2          |
| 0.13 | 0.1          |

# License

All code in this repository is dual-licensed under either:

- MIT License (LICENSE-MIT or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option. This means you can select the license you prefer.

## Your contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.