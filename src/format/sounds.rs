//! Sounds/Sounds.toml: every sound of the game, one line each, by the name the
//! code knows it by:
//!
//!     menu_press = { file = "Menu/menu_press.wav", volume = 255, audio-track = "ui-sfx" }
//!
//! `file` is relative to Sounds/, so a sound pack replaces a file or edits its line.

use serde::Deserialize;
use std::collections::BTreeMap;

pub const FILE_NAME: &str = "Sounds.toml";

/// What kind of sound it is. Nothing reads it yet: the code still picks the track a
/// sound plays on. It is written down for effects that treat the kinds differently.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AudioTrack {
    #[default]
    PlayerSfx,
    UiSfx,
    Music,
    Ambient,
}

impl AudioTrack {
    /// The kind a sound gets from the top folder GameMaker kept it in.
    pub fn for_folder(top_folder: &str) -> AudioTrack {
        match top_folder {
            "Music" => AudioTrack::Music,
            "Menu" => AudioTrack::UiSfx,
            "Ambient" => AudioTrack::Ambient,
            _ => AudioTrack::PlayerSfx,
        }
    }

    fn name(self) -> &'static str {
        match self {
            AudioTrack::PlayerSfx => "player-sfx",
            AudioTrack::UiSfx => "ui-sfx",
            AudioTrack::Music => "music",
            AudioTrack::Ambient => "ambient",
        }
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct SoundEntry {
    pub file: String,
    /// Base volume of every playback: 0 is silent, 255 is the file as recorded.
    #[serde(default = "full_volume")]
    pub volume: u8,
    #[serde(rename = "audio-track", default)]
    pub audio_track: AudioTrack,
}

fn full_volume() -> u8 {
    255
}

/// GameMaker's 0..1 sound volume on the 0..255 scale.
pub fn volume_from_gain(gain: f32) -> u8 {
    (gain.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub type SoundList = BTreeMap<String, SoundEntry>;

pub fn parse(text: &str) -> Result<SoundList, toml::de::Error> {
    toml::from_str(text)
}

/// One line a sound, sorted by name. The toml crate would give every sound its own
/// [table] of three lines instead.
pub fn to_toml(list: &SoundList) -> String {
    let mut text = String::new();
    for (name, entry) in list {
        let file = toml::Value::String(entry.file.clone());
        text += &format!("{name} = {{ file = {file}, volume = {}, audio-track = \"{}\" }}\n", entry.volume, entry.audio_track.name());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_list_reads_back_the_same() {
        let mut list = SoundList::new();
        list.insert("menu_press".into(), SoundEntry { file: "Menu/menu_press.wav".into(), volume: 77, audio_track: AudioTrack::UiSfx });
        list.insert("rain".into(), SoundEntry { file: "Ambient/rain.ogg".into(), volume: 255, audio_track: AudioTrack::Ambient });
        let text = to_toml(&list);
        assert_eq!(text.lines().count(), 2);
        assert_eq!(parse(&text).unwrap(), list);
        assert_eq!(volume_from_gain(0.3), 77);
    }
}
