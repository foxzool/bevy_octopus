[![crates.io](https://img.shields.io/crates/v/bevy_octopus)](https://crates.io/crates/bevy_octopus)
[![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/Seldom-SE/seldom_pixel#license)
[![crates.io](https://img.shields.io/crates/d/bevy_octopus)](https://crates.io/crates/bevy_octopus)
[![CI](https://github.com/foxzool/bevy_octopus/workflows/CI/badge.svg)](https://github.com/foxzool/bevy_octopus/actions)
[![Documentation](https://docs.rs/bevy_octopus/badge.svg)](https://docs.rs/bevy_octopus)

# bevy_octopus

A Low-level ECS-driven network plugin for Bevy.

## Usage

Add this in your Cargo.toml:

```toml
[dependencies]
bevy_octopus = { version = "0.4", "features" = ["serde_json", "bincode"] } # or your custom format
```

## Example

```ignore,rust 
use bevy::prelude::*;
use bevy_octopus::{
    prelude::*,
    transports::{tcp::TcpAddress, udp::UdpAddress},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerInformation {
    pub health: usize,
    pub position: (u32, u32, u32),
}

const TCP_CHANNEL: ChannelId = ChannelId("tcp");
const UDP_CHANNEL: ChannelId = ChannelId("udp");

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(OctopusPlugin)
        // UDP CHANNEL use json tranformer for PlayerInformation struct
        .add_transformer::<PlayerInformation, JsonTransformer>(UDP_CHANNEL)
        // TCP_CHANNEL use json tranformer for PlayerInformation struct
        .add_transformer::<PlayerInformation, BincodeTransformer>(TCP_CHANNEL)
        .add_systems(Startup, setup)
        .add_systems(Update, resend_udp_to_tcp)
        .observe(on_node_event)
        .run();
}

fn setup(mut commands: Commands) {
    // tcp client
    commands.spawn((
        NetworkBundle::new(TCP_CHANNEL),
        ClientNode(TcpAddress::new("127.0.0.1:4321")),
    ));

    // udp server
    commands.spawn((
        NetworkBundle::new(UDP_CHANNEL),
        ServerNode(UdpAddress::new("127.0.0.1:4002")),
    ));
}

pub fn on_node_event(trigger: Trigger<NetworkEvent>) {
    info!("{:?} trigger {:?}", trigger.entity(), trigger.event());
}

pub fn resend_udp_to_tcp(
    mut channel_recviced: EventReader<ReceiveChannelMessage<PlayerInformation>>,
    mut ev_send: EventWriter<SendChannelMessage<PlayerInformation>>,
) {
    for event in channel_recviced.read() {
        info!("recevice {:?}", event.message);
        if event.channel_id == UDP_CHANNEL {
            ev_send.send(SendChannelMessage {
                channel_id: TCP_CHANNEL,
                message: event.message.clone(),
            });
        }
    }
}

```

## Features

### ECS driven network

Every network node is a component, so you can easily manage network entities with Bevy ECS.

Apps can be many servers and many clients at the same time.

### Flexible network protocol decoder

You can define channel transformers for data serialization and deserialization.

### UDP Communication Types

Support UDP [unicast](https://github.com/foxzool/bevy_octopus/blob/main/examples/udp/client_raw.rs), broadcast,
multicast. [example](https://github.com/foxzool/bevy_octopus/blob/main/examples/udp/udp_complex.rs)

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
| 0.15 | 0.4          |
| 0.14 | 0.2 , 0.3    |
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