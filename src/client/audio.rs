//! Sound effects and music (audio_play_sound, audio_stop_sound, scr_play_music).
//! Which file each sound is and how loud comes from Sounds/Sounds.toml (format::sounds).

use anyhow::{Context, Result};
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::sound::{FromFileError, PlaybackState};
use kira::effect::filter::{FilterBuilder, FilterHandle, FilterMode};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Panning, Tween};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use crate::format::sounds::{self, SoundEntry};
use crate::core::resources::names::SOUND_FILES;
use crate::core::resources::SoundId;

/// The pause effect (Battletoads): the sounds of the level fall silent and the music is
/// heard muffled, its highs cut. Much lower and small speakers play nothing at all.
const PAUSE_MUSIC_CUTOFF_HZ: f64 = 700.0;
const OPEN_CUTOFF_HZ: f64 = 20000.0;
const PAUSE_FADE_SECONDS: f64 = 0.25;
pub struct Audio {
    /// None when there is no usable audio device: the game then runs silently. Nothing
    /// reads it after the tracks are made, but dropping it would close the device.
    #[allow(dead_code)]
    manager: Option<AudioManager<DefaultBackend>>,
    /// Sounds and music play on their own tracks, so a pause can mute the one and
    /// filter the other (see `set_paused`).
    tracks: Option<Tracks>,
    /// The line of Sounds.toml of each SoundId, None when it has none.
    sounds: Vec<Option<SoundEntry>>,
    sounds_folder: PathBuf,
    /// Decoded sound effects and their file volumes, kept after their first use.
    decoded: HashMap<SoundId, (StaticSoundData, Decibels)>,
    playing: Vec<(SoundId, StaticSoundHandle)>,
    /// Looping sounds that stay at a place (audio_play_sound_on with an emitter), by the
    /// kind of thing they come from and its number.
    emitters: HashMap<(&'static str, u32), StaticSoundHandle>,
    /// global.music: the one looping track, streamed from its file, and its file volume.
    music: Option<(SoundId, StreamingSoundHandle<FromFileError>, Decibels)>,
    /// A second track looping silently beside the music (Act 9's chase), and its file volume.
    quiet_music: Option<(SoundId, StreamingSoundHandle<FromFileError>, Decibels)>,
}

struct Tracks {
    sounds: TrackHandle,
    /// The menus' own sounds, which a pause leaves as they are.
    interface: TrackHandle,
    music: TrackHandle,
    /// The low pass the paused music is heard through.
    music_filter: FilterHandle,
}

impl Audio {
    pub fn new(sounds_folder: &Path) -> Result<Audio> {
        let list_path = sounds_folder.join(sounds::FILE_NAME);
        let mut by_name = sounds::parse(&crate::core::resources::read_to_string(&list_path)?)
            .with_context(|| format!("parsing {}", list_path.display()))?;
        let mut manager = open_manager();
        let tracks = manager.as_mut().and_then(|manager| build_tracks(manager));
        Ok(Audio {
            manager,
            tracks,
            sounds: SOUND_FILES.iter().map(|name| by_name.remove(*name)).collect(),
            sounds_folder: sounds_folder.to_path_buf(),
            decoded: HashMap::new(),
            playing: Vec::new(),
            emitters: HashMap::new(),
            music: None,
            quiet_music: None,
        })
    }

    /// audio_play_sound
    pub fn play(&mut self, sound: SoundId, looping: bool) {
        let Some((data, _)) = self.decoded_sound(sound) else { return };
        let data = if looping { data.loop_region(..) } else { data };
        self.start(sound, data, false);
    }

    /// A sound of a menu or the chat: heard even while the pause menu mutes the level.
    pub fn play_interface(&mut self, sound: SoundId) {
        let Some((data, _)) = self.decoded_sound(sound) else { return };
        self.start(sound, data, true);
    }

    /// scr_audio_play_3d: a sound from a place in the level, `gain` 0..1 loud, `pan`
    /// from -1 (left) to 1 (right) of the listener.
    pub fn play_at(&mut self, sound: SoundId, gain: f64, pan: f64) {
        let Some((data, file_volume)) = self.decoded_sound(sound) else { return };
        self.start(sound, data.volume(gain_to_decibels(gain, file_volume)).panning(Panning(pan.clamp(-1.0, 1.0) as f32)), false);
    }

    /// An emitter's looping sound: started the first time, afterwards only as loud and
    /// as far to one side as the emitter now is.
    pub fn set_emitter(&mut self, key: (&'static str, u32), sound: SoundId, gain: f64, pan: f64) {
        let pan = Panning(pan.clamp(-1.0, 1.0) as f32);
        if let Some(handle) = self.emitters.get_mut(&key) {
            let file_volume = self.decoded.get(&sound).map_or(Decibels(0.0), |(_, volume)| *volume);
            handle.set_volume(gain_to_decibels(gain, file_volume), Tween::default());
            handle.set_panning(pan, Tween::default());
            return;
        }
        let Some((data, file_volume)) = self.decoded_sound(sound) else { return };
        let Some(tracks) = self.tracks.as_mut() else { return };
        match tracks.sounds.play(data.volume(gain_to_decibels(gain, file_volume)).panning(pan).loop_region(..)) {
            Ok(handle) => {
                self.emitters.insert(key, handle);
            }
            Err(error) => eprintln!("sound {} did not play: {error}", SOUND_FILES[sound.0]),
        }
    }

    /// audio_emitter_free: the emitters of one kind whose things are gone fall silent.
    pub fn keep_emitters(&mut self, kind: &str, alive: impl Fn(u32) -> bool) {
        self.emitters.retain(|&(emitter_kind, number), handle| {
            let keep = emitter_kind != kind || alive(number);
            if !keep {
                handle.stop(Tween::default());
            }
            keep
        });
    }

    fn start(&mut self, sound: SoundId, data: StaticSoundData, interface: bool) {
        self.playing.retain(|(_, handle)| handle.state() != PlaybackState::Stopped);
        let Some(tracks) = self.tracks.as_mut() else { return };
        let track = if interface { &mut tracks.interface } else { &mut tracks.sounds };
        match track.play(data) {
            Ok(handle) => self.playing.push((sound, handle)),
            Err(error) => eprintln!("sound {} did not play: {error}", SOUND_FILES[sound.0]),
        }
    }

    /// audio_stop_sound with a sound resource: every copy of it that is playing.
    pub fn stop(&mut self, sound: SoundId) {
        for (_, handle) in self.playing.iter_mut().filter(|(playing, _)| *playing == sound) {
            handle.stop(Tween::default());
        }
    }

    /// audio_is_playing with a sound resource, music included.
    pub fn is_playing(&self, sound: SoundId) -> bool {
        let is_music = self.music.as_ref().is_some_and(|(music, handle, _)| *music == sound && handle.state() != PlaybackState::Stopped);
        is_music || self.playing.iter().any(|(playing, handle)| *playing == sound && handle.state() != PlaybackState::Stopped)
    }

    /// audio_stop_all, music included.
    pub fn stop_all(&mut self) {
        for (_, handle) in &mut self.playing {
            handle.stop(Tween::default());
        }
        for (_, handle) in self.emitters.drain() {
            let mut handle = handle;
            handle.stop(Tween::default());
        }
        self.stop_music();
    }

    /// scr_play_music: replaces the current track and loops it.
    pub fn play_music(&mut self, sound: SoundId) {
        self.stop_music();
        self.music = self.stream(sound, false).map(|(handle, volume)| (sound, handle, volume));
    }

    /// Act 9's room creation code: the music, and `quiet` looping beside it at gain 0.
    pub fn play_music_with_quiet_track(&mut self, sound: SoundId, quiet: SoundId) {
        self.play_music(sound);
        self.quiet_music = self.stream(quiet, true).map(|(handle, volume)| (quiet, handle, volume));
    }

    /// audio_sound_gain over `seconds`: the music fades out as the second track comes up,
    /// or back when `to_second` is false.
    pub fn crossfade(&mut self, to_second: bool, seconds: f64) {
        let tween = Tween { duration: std::time::Duration::from_secs_f64(seconds), ..Tween::default() };
        if let Some((_, music, volume)) = self.music.as_mut() {
            music.set_volume(if to_second { Decibels::SILENCE } else { *volume }, tween);
        }
        if let Some((_, quiet, volume)) = self.quiet_music.as_mut() {
            quiet.set_volume(if to_second { *volume } else { Decibels::SILENCE }, tween);
        }
    }

    /// audio_sound_gain(global.music, ...): the music alone goes quiet or comes back,
    /// which the hiding warning does while it holds the screen.
    pub fn fade_music(&mut self, quiet: bool, seconds: f64) {
        let tween = Tween { duration: std::time::Duration::from_secs_f64(seconds), ..Tween::default() };
        if let Some((_, music, volume)) = self.music.as_mut() {
            music.set_volume(if quiet { Decibels::SILENCE } else { *volume }, tween);
        }
    }

    /// The pause effect: the sounds go silent and the music is left with its low end,
    /// as if only the subwoofer were still playing.
    pub fn set_paused(&mut self, paused: bool) {
        let Some(tracks) = self.tracks.as_mut() else { return };
        let tween = Tween { duration: std::time::Duration::from_secs_f64(PAUSE_FADE_SECONDS), ..Tween::default() };
        tracks.sounds.set_volume(if paused { Decibels::SILENCE } else { Decibels::IDENTITY }, tween);
        tracks.music_filter.set_cutoff(if paused { PAUSE_MUSIC_CUTOFF_HZ } else { OPEN_CUTOFF_HZ }, tween);
    }

    /// scr_stop_music
    pub fn stop_music(&mut self) {
        if let Some((_, mut music, _)) = self.music.take() {
            music.stop(Tween::default());
        }
        if let Some((_, mut quiet, _)) = self.quiet_music.take() {
            quiet.stop(Tween::default());
        }
    }

    /// A looping track streamed from its file, silent to begin with when `quiet`.
    fn stream(&mut self, sound: SoundId, quiet: bool) -> Option<(StreamingSoundHandle<FromFileError>, Decibels)> {
        let (path, volume) = self.file(sound)?;
        let tracks = self.tracks.as_mut()?;
        let data = match read_sound(&path).and_then(|bytes| Ok(StreamingSoundData::from_cursor(bytes)?)) {
            Ok(data) => data.volume(if quiet { Decibels::SILENCE } else { volume }).loop_region(..),
            Err(error) => {
                eprintln!("music {} did not load: {error}", path.display());
                return None;
            }
        };
        match tracks.music.play(data) {
            Ok(handle) => Some((handle, volume)),
            Err(error) => {
                eprintln!("music {} did not play: {error}", path.display());
                None
            }
        }
    }

    fn decoded_sound(&mut self, sound: SoundId) -> Option<(StaticSoundData, Decibels)> {
        if let Some(decoded) = self.decoded.get(&sound) {
            return Some(decoded.clone());
        }
        let (path, volume) = self.file(sound)?;
        let data = match read_sound(&path).and_then(|bytes| Ok(StaticSoundData::from_cursor(bytes)?)) {
            Ok(data) => data.volume(volume),
            Err(error) => {
                eprintln!("sound {} did not load: {error}", path.display());
                return None;
            }
        };
        self.decoded.insert(sound, (data.clone(), volume));
        Some((data, volume))
    }

    /// The sound's file and its volume from Sounds.toml.
    fn file(&self, sound: SoundId) -> Option<(PathBuf, Decibels)> {
        let Some(entry) = &self.sounds[sound.0] else {
            eprintln!("sound {} is missing from {}", SOUND_FILES[sound.0], sounds::FILE_NAME);
            return None;
        };
        Some((self.sounds_folder.join(&entry.file), volume_to_decibels(entry.volume)))
    }
}

/// The default output of PipeWire, or of ALSA where there is no PipeWire; None plays nothing.
fn open_manager() -> Option<AudioManager<DefaultBackend>> {
    match AudioManager::new(AudioManagerSettings::default()) {
        Ok(manager) => Some(manager),
        Err(error) => {
            eprintln!("no sound: the audio device did not open ({error})");
            None
        }
    }
}

/// The name the mixer shows for the game. The audio library names its PipeWire stream
/// "cpal-playback-<pid>" and has no setting for it; libpipewire lays PIPEWIRE_PROPS over
/// the stream's own properties. Called before any thread starts, as setting a variable
/// while another thread reads the environment is not safe.
pub fn name_the_stream(title: &str) {
    std::env::set_var("PIPEWIRE_PROPS", format!("{{ application.name = \"{title}\" media.name = \"{title}\" }}"));
}

/// The sound and music tracks, with the low pass the pause effect uses. Without them
/// the game still runs, only silently.
fn build_tracks(manager: &mut AudioManager<DefaultBackend>) -> Option<Tracks> {
    let mut music = TrackBuilder::new();
    // The filter is always in the chain and opened wide, so a pause only moves its cutoff.
    let music_filter = music.add_effect(FilterBuilder::new().mode(FilterMode::LowPass).cutoff(OPEN_CUTOFF_HZ));
    let sounds = manager.add_sub_track(TrackBuilder::new()).ok()?;
    let interface = manager.add_sub_track(TrackBuilder::new()).ok()?;
    let music = manager.add_sub_track(music).ok()?;
    Some(Tracks { sounds, interface, music, music_filter })
}

/// A 0..1 gain on top of the file's own volume.
fn gain_to_decibels(gain: f64, file_volume: Decibels) -> Decibels {
    if gain <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(file_volume.0 + 20.0 * gain.log10() as f32)
    }
}

/// A sound file's bytes, from Resources.tar in a release (see core/resources).
fn read_sound(path: &Path) -> Result<Cursor<Vec<u8>>> {
    crate::core::resources::read(path).map(Cursor::new)
}

/// A volume of Sounds.toml, 0..255, in decibels.
fn volume_to_decibels(volume: u8) -> Decibels {
    if volume == 0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * (volume as f32 / 255.0).log10())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_has_a_line_and_a_file() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("Resources/Sounds");
        let list = sounds::parse(&std::fs::read_to_string(folder.join(sounds::FILE_NAME)).unwrap()).unwrap();
        for name in SOUND_FILES {
            let entry = list.get(name).unwrap_or_else(|| panic!("{name} is not in {}", sounds::FILE_NAME));
            assert!(folder.join(&entry.file).is_file(), "{name}: {} does not exist", entry.file);
        }
    }
}
