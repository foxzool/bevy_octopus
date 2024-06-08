use bevy::ecs::prelude::*;

use crate::channels::{ChannelId, ChannelPacket};
use crate::connections::NetworkPeer;
use crate::network::{ConnectTo, NetworkRawPacket};
use crate::network_node::NetworkNode;

pub(crate) fn send_channel_message_system(
    q_net: Query<(&ChannelId, &NetworkNode, &ConnectTo), With<NetworkPeer>>,
    mut channel_events: EventReader<ChannelPacket>,
) {
    for channel_ev in channel_events.read() {
        q_net
            .par_iter()
            .for_each(|(channel_id, net_node, connect_to)| {
                if channel_id == &channel_ev.channel_id {
                    net_node
                        .send_message_channel
                        .sender
                        .send(NetworkRawPacket {
                            bytes: channel_ev.bytes.clone(),
                            addr: connect_to.peer_addr(),
                        })
                        .expect("send message channel has closed");
                }
            });
    }
}
