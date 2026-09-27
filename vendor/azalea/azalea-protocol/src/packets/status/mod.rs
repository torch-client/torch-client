use azalea_protocol_macros::declare_state_packets;

declare_state_packets!(StatusPacket,
    Clientbound => [
        status_response,
        pong_response,
    ],
    Serverbound => [
        status_request,
        ping_request,
    ]
);
