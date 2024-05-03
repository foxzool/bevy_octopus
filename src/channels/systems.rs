use bevy::ecs::prelude::*;

use crate::channels::{ChannelId, ChannelPacket};
use crate::connections::NetworkPeer;
use crate::network::NetworkRawPacket;
use crate::network_manager::NetworkNode;

pub(crate) fn send_channel_message_system(
    q_net: Query<(&ChannelId, &NetworkNode), With<NetworkPeer>>,
    mut channel_events: EventReader<ChannelPacket>,
) {
    for channel_ev in channel_events.read() {
        q_net.par_iter().for_each(|(channel_id, net_node)| {
            if channel_id == &channel_ev.channel_id {
                net_node
                    .send_message_channel
                    .sender
                    .send(NetworkRawPacket {
                        bytes: channel_ev.bytes.clone(),
                        addr: net_node
                            .peer_addr
                            .unwrap_or_else(|| "127.0.0.1:0".parse().unwrap()),
                    })
                    .expect("send message channel has closed");
            }
        });
    }
}
