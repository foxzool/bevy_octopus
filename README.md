# bevy_ecs_net

The ECS driven network plugin for Bevy.

## Features

### ECS driven network

Every network node is a component, so you can easily manage network entities with Bevy ECS.

App can be a server or client or both.

### Flexible network protocol decoder

You can define your own network protocol decoder.

### UDP unicast broadcast multicast

Support UDP [unicast](https://github.com/foxzool/bevy_ecs_net/blob/main/examples/udp_example.rs), broadcast,
multicast. [example](https://github.com/foxzool/bevy_ecs_net/blob/main/examples/udp_complex.rs)

## Supported Network Protocol

- [x] UDP
- [ ] TCP
- [ ] WebSocket
- [ ] SSL
- [ ] WebSocket SSL

## Supported Versions

| bevy | bevy_ecs_net |
|------|--------------|
| 0.13 | 0.1          |

## License

Dual-licensed under either

- MIT
- Apache 2.0