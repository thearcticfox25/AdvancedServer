pub mod ids;

use ids::RoomId;

/// The playable levels by the number the server and the protocol use (global.levels).
pub const LEVEL_ROOMS: [RoomId; 21] = [
    RoomId::Hideandseek2,
    RoomId::Ravinemist,
    RoomId::Dotdotdot,
    RoomId::Deserttown,
    RoomId::Youcantrun,
    RoomId::Limpcity,
    RoomId::Notperfect,
    RoomId::Kindandfair,
    RoomId::Act9,
    RoomId::Nastyparadise,
    RoomId::Pricelessfreedom,
    RoomId::Volcanovalley,
    RoomId::Greenhill,
    RoomId::Majongforest,
    RoomId::Angelisland,
    RoomId::Torturecave,
    RoomId::Dartower,
    RoomId::Haundream,
    RoomId::Weedzone,
    RoomId::Marijuna,
    RoomId::Fartzone,
];
