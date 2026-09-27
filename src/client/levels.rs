//! The music of the playable levels, by the number the server uses for them (global.levels).

use crate::core::resources::names::sound;
use crate::core::resources::SoundId;
use crate::core::rooms::ids::RoomId;
use crate::core::rooms::LEVEL_ROOMS;

pub struct Level {
    pub room: RoomId,
    pub music: SoundId,
    /// The music once EXE is close (chaseMusic).
    pub chase_music: SoundId,
}

pub const LEVELS: [Level; 21] = [
    Level { room: LEVEL_ROOMS[0], music: sound::MUS_HIDEANDSEEK2, chase_music: sound::MUS_HIDEANDSEEK2_CHASE },
    Level { room: LEVEL_ROOMS[1], music: sound::MUS_RAVIMEMIST, chase_music: sound::MUS_RAVIMEMIST_CHASE },
    Level { room: LEVEL_ROOMS[2], music: sound::MUS_DOTDOTDOT, chase_music: sound::MUS_DOTDOTDOT_CHASE },
    Level { room: LEVEL_ROOMS[3], music: sound::MUS_DESERTTOWN, chase_music: sound::MUS_DESERTTOWN_CHASE },
    Level { room: LEVEL_ROOMS[4], music: sound::MUS_YOUCANTRUN, chase_music: sound::MUS_YOUCANTRUN_CHASE },
    Level { room: LEVEL_ROOMS[5], music: sound::MUS_LIMPCITY, chase_music: sound::MUS_LIMPCITY_CHASE },
    Level { room: LEVEL_ROOMS[6], music: sound::MUS_NOTPERFECT, chase_music: sound::MUS_NOTPERFECT_CHASE },
    Level { room: LEVEL_ROOMS[7], music: sound::MUS_KINDANDFAIR, chase_music: sound::MUS_KINDANDFAIR_CHASE },
    Level { room: LEVEL_ROOMS[8], music: sound::MUS_ACT9, chase_music: sound::MUS_ACT9_CHASE },
    Level { room: LEVEL_ROOMS[9], music: sound::MUS_NASTYPARADISE, chase_music: sound::MUS_NASTYPARADISE_CHASE },
    Level { room: LEVEL_ROOMS[10], music: sound::MUS_PRICELESSFREEDOM, chase_music: sound::MUS_PRICELESSFREEDOM_CHASE },
    Level { room: LEVEL_ROOMS[11], music: sound::MUS_VOLCANOVALLEY, chase_music: sound::MUS_VOLCANOVALLEY_CHASE },
    Level { room: LEVEL_ROOMS[12], music: sound::MUS_HILL, chase_music: sound::MUS_HILL_CHASE },
    Level { room: LEVEL_ROOMS[13], music: sound::MUS_MAJINFOREST, chase_music: sound::MUS_MAJINFOREST_CHASE },
    Level { room: LEVEL_ROOMS[14], music: sound::MUS_ANGELISLAND, chase_music: sound::MUS_ANGELISLAND_CHASE },
    Level { room: LEVEL_ROOMS[15], music: sound::MUS_TORTURECAVE, chase_music: sound::MUS_TORTURECAVE_CHASE },
    Level { room: LEVEL_ROOMS[16], music: sound::MUS_DARKTOWER, chase_music: sound::MUS_DARKTOWER_CHASE },
    Level { room: LEVEL_ROOMS[17], music: sound::MUS_HAUNTDREAM, chase_music: sound::MUS_HAUNTDREAM_CHASE },
    Level { room: LEVEL_ROOMS[18], music: sound::MUS_WEEDZONE, chase_music: sound::MUS_WEEDZONE_CHASE },
    Level { room: LEVEL_ROOMS[19], music: sound::MUS_MARIJUNA, chase_music: sound::MUS_MARIJUNA_CHASE },
    Level { room: LEVEL_ROOMS[20], music: sound::MUS_FARTZONE, chase_music: sound::MUS_FARTZONE_CHASE },
];
