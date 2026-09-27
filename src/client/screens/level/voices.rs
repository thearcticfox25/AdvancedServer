//! EXE voice lines: which line an event plays, chosen at random as the original
//! did, and never over another voice line (global.net_snds from index 46 on).

use crate::client::audio::Audio;
use macroquad::rand::ChooseRandom;
use crate::core::resources::names::sound::*;
use crate::core::resources::SoundId;
use crate::core::player::ExeCharacter;

/// The original's check starts at index 46 of a list that later gained snd_none at
/// its head, so Exeller's stun sound counts as a voice line too.
const VOICE_LINES: &[SoundId] = &[
    SND_EXELLER_STUN,
    SND_EXE_KILL1, SND_EXE_KILL2, SND_EXE_KILL3, SND_EXE_KILL4, SND_EXE_LAUGH, SND_EXE_TAUNT, SND_EXE_TAUNT1, SND_EXE_TAUNT2, SND_EXE_STUN2,
    SND_EXE_APPEAR2, SND_EXE_APPEAR3, SND_EXE_INVISENTER, SND_EXE_INVISENTER2,
    SND_CHAOS_KILL, SND_CHAOS_KILL2, SND_CHAOS_KILL3, SND_CHAOS_KILL4, SND_CHAOS_KILL5, SND_CHAOS_KILL6, SND_CHAOS_KILL7, SND_CHAOS_KILL8,
    SND_CHAOS_TAUNT, SND_CHAOS_TAUNT2, SND_CHAOS_PIZZA, SND_CHAOS_VINEBOOM, SND_CHAOS_LAUGH,
    SND_EXETIOR_KILL1, SND_EXETIOR_KILL2, SND_EXETIOR_KILL3, SND_EXETIOR_KILL4, SND_EXETIOR_KILL5, SND_EXETIOR_KILL6,
    SND_EXETIOR_TAUNT1, SND_EXETIOR_TAUNT2, SND_EXETIOR_TAUNT3, SND_EXETIOR_RING1, SND_EXETIOR_RING2, SND_EXETIOR_RING3, SND_EXETIOR_RING4,
    SND_EXETIOR_LAUGH,
    SND_EXELLER_CLONE, SND_EXELLER_CLONELINE, SND_EXELLER_KILL1, SND_EXELLER_KILL2, SND_EXELLER_KILL3, SND_EXELLER_KILL4, SND_EXELLER_KILL5,
    SND_EXELLER_KILL6, SND_EXELLER_KILL7, SND_EXELLER_TAUNT1, SND_EXELLER_TAUNT2, SND_EXELLER_TAUNT3, SND_EXELLER_LAUGH,
    SND_CHAOS_LAND, SND_TAILSBALL_JUMPSCARE2, SND_ROAR, SND_BOOHOO,
];

/// A voice line is playing, so no other may start.
pub fn voice_playing(audio: &Audio) -> bool {
    VOICE_LINES.iter().any(|&line| audio.is_playing(line))
}

fn one_of(lines: &[SoundId]) -> SoundId {
    *lines.choose().expect("voice line lists are not empty")
}

/// scr_move_basic: the taunt of an emotion key (1 to 3).
pub fn taunt(exe: ExeCharacter, emotion: u8) -> SoundId {
    match (exe, emotion) {
        (ExeCharacter::Original, 1) => SND_EXE_TAUNT,
        (ExeCharacter::Original, 2) => one_of(&[SND_EXE_TAUNT, SND_EXE_TAUNT1]),
        (ExeCharacter::Original, _) => SND_EXE_LAUGH,
        (ExeCharacter::Chaos, 1) => one_of(&[SND_CHAOS_PIZZA, SND_CHAOS_VINEBOOM]),
        (ExeCharacter::Chaos, _) => one_of(&[SND_CHAOS_TAUNT, SND_CHAOS_TAUNT2]),
        (ExeCharacter::Exetior, 3) => SND_EXETIOR_LAUGH,
        (ExeCharacter::Exetior, _) => one_of(&[SND_EXETIOR_TAUNT1, SND_EXETIOR_TAUNT2, SND_EXETIOR_TAUNT3]),
        (ExeCharacter::Exeller, 1) => SND_EXELLER_TAUNT3,
        (ExeCharacter::Exeller, 2) => one_of(&[SND_EXELLER_TAUNT1, SND_EXELLER_TAUNT2]),
        (ExeCharacter::Exeller, _) => SND_EXELLER_LAUGH,
    }
}

/// scr_exe_checkwin: a kill line of the EXE that killed.
pub fn kill(exe: ExeCharacter) -> SoundId {
    match exe {
        ExeCharacter::Original => one_of(&[SND_EXE_KILL1, SND_EXE_KILL2, SND_EXE_KILL3, SND_EXE_KILL4]),
        ExeCharacter::Chaos => one_of(&[SND_CHAOS_KILL, SND_CHAOS_KILL2, SND_CHAOS_KILL3, SND_CHAOS_KILL4, SND_CHAOS_KILL5, SND_CHAOS_KILL6, SND_CHAOS_KILL7, SND_CHAOS_KILL8]),
        ExeCharacter::Exetior => one_of(&[SND_EXETIOR_KILL1, SND_EXETIOR_KILL2, SND_EXETIOR_KILL3, SND_EXETIOR_KILL4, SND_EXETIOR_KILL5, SND_EXETIOR_KILL6]),
        ExeCharacter::Exeller => one_of(&[SND_EXELLER_KILL1, SND_EXELLER_KILL2, SND_EXELLER_KILL3, SND_EXELLER_KILL4, SND_EXELLER_KILL5, SND_EXELLER_KILL6, SND_EXELLER_KILL7]),
    }
}

/// scr_exe_special: turning invisible, heard by the EXE alone.
pub fn vanish() -> SoundId {
    one_of(&[SND_EXE_INVISENTER, SND_EXE_INVISENTER2])
}

/// scr_exe_special: becoming visible again (irandom_range(0, 100) >= 50).
pub fn appear() -> SoundId {
    if macroquad::rand::gen_range(0, 101) >= 50 {
        SND_EXE_APPEAR2
    } else {
        SND_EXE_APPEAR3
    }
}

/// scr_exeller_special: placing a clone, a line one time in four (choose with three snd_none).
pub fn clone_line() -> Option<SoundId> {
    (macroquad::rand::gen_range(0, 4) == 0).then_some(SND_EXELLER_CLONELINE)
}

/// scr_exetior_special: placing a black ring.
pub fn black_ring() -> SoundId {
    one_of(&[SND_EXETIOR_RING1, SND_EXETIOR_RING2, SND_EXETIOR_RING3, SND_EXETIOR_RING4])
}
