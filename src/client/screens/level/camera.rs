//! obj_camera: centred on this player (trailing a little in the air, moved by
//! looking up or down), still for a moment after dying or escaping, then watching
//! the other survivors; shaken by hits, kept inside the room.

use crate::client::canvas::{VIEW_HEIGHT, VIEW_WIDTH};
use macroquad::rand;
use crate::core::resources::sprites::Sprites;
use crate::core::player::{gm_sign, Buttons, Character, Player};
use crate::core::config::step;
use crate::core::config::ticks;

/// In the air the camera lets the player move this far vertically before following.
const AIR_SLACK: f64 = 32.0;
/// Looking up or down for longer than this moves the camera, a few pixels a step.
const LOOK_DELAY_SECONDS: f64 = 1.5;
const LOOK_DISTANCE: f64 = 100.0;
const LOOK_STEP: f64 = 2.0;
/// Pixels a 60 Hz step the camera returns towards the player when nobody looks up or down.
const RETURN_SPEED: f64 = 2.0;
/// Mode 3 waits this long before watching someone else (alarm[1] = 60 * 3).
const FADE_SECONDS: f64 = 3.0;
/// A watched player further than this from the camera is caught up with half the way a step.
const CATCH_UP_DISTANCE: f64 = 10.0;
/// Pixels a step the free camera flies with the movement keys.
const FREE_SPEED: f64 = 6.0;
const CATCH_UP_SHARE: f64 = 0.5;

/// global.cameraMode
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CameraMode {
    /// 0: this player.
    Follow,
    /// 2: someone else, changed with any key unless locked.
    Spectate,
    /// 3: standing still after dying or escaping.
    Fade,
    /// 4: on a place (Limp City's watched eye), set from outside.
    Watch,
    /// The free camera of the Singleplayer page and of a spectator: it follows
    /// nobody and flies with the movement keys.
    Free,
}

pub struct Camera {
    pub x: f64,
    pub y: f64,
    dist: f64,
    look_timer: i32,
    shake: Option<Shake>,
    pub mode: CameraMode,
    previous_mode: Option<CameraMode>,
    fade_alarm: i32,
    /// spectatingInd and spectatingObj
    spectating_index: usize,
    pub spectating: Option<u16>,
    /// After the round's end the camera stays on EXE.
    pub locked: bool,
    /// A spectator watches everyone -- survivors, demons and killers alike -- and
    /// skips whoever is away in their pause menu. A player who died watches the
    /// survivors who are still up, as the original let them.
    watch_everyone: bool,
}

/// What the camera looks at this step.
pub struct CameraView<'a> {
    pub own: Option<&'a Player>,
    /// Everyone else, by id in the order the client knows them.
    pub others: &'a [(u16, Player)],
    pub exe_ids: &'a [u16],
    pub any_key_pressed: bool,
    /// What this client holds down, which the free camera flies with.
    pub buttons: Buttons,
    pub room_width: f64,
    pub room_height: f64,
}

struct Shake {
    time: i32,
    magnitude: f64,
    fade: f64,
}

impl Camera {
    /// Create event: the view centred on the player.
    pub fn new(player: &Player) -> Camera {
        Camera::at(player.x, player.y, CameraMode::Follow)
    }

    /// The free camera, which starts where the player it stands in for would have.
    pub fn free(x: f64, y: f64) -> Camera {
        Camera::at(x, y, CameraMode::Free)
    }

    /// A spectator's camera: it has no player of its own, so it starts out looking for
    /// one of the round's to follow (states::spectate).
    pub fn watching() -> Camera {
        let mut camera = Camera::at(0.0, 0.0, CameraMode::Spectate);
        camera.watch_everyone = true;
        camera
    }

    /// Watching and the free camera, one key apart: a spectator may do both.
    pub fn toggle_free(&mut self) {
        self.mode = match self.mode {
            CameraMode::Free => CameraMode::Spectate,
            _ => CameraMode::Free,
        };
    }

    fn at(x: f64, y: f64, mode: CameraMode) -> Camera {
        Camera {
            x: x.floor() - VIEW_WIDTH / 2.0,
            y: y.floor() - VIEW_HEIGHT / 2.0,
            dist: AIR_SLACK,
            look_timer: 0,
            shake: None,
            mode,
            previous_mode: None,
            fade_alarm: super::super::ALARM_OFF,
            spectating_index: 0,
            spectating: None,
            locked: false,
            watch_everyone: false,
        }
    }

    /// obj_aiz_zipline: the camera jumps to centre the player, without look-ahead.
    pub fn snap_to(&mut self, player: &Player) {
        self.x = player.x.floor() - VIEW_WIDTH / 2.0;
        self.y = player.y.floor() - VIEW_HEIGHT / 2.0;
        self.dist = 0.0;
    }

    /// Alarm_1 and Draw_76 (Pre-Draw) of obj_camera.
    pub fn step(&mut self, view: &CameraView, sprites: &Sprites) {
        if super::super::count_down_alarm(&mut self.fade_alarm) {
            self.fade_over(view);
        }
        match self.mode {
            CameraMode::Follow => {
                if let Some(own) = view.own {
                    self.follow(own);
                }
            }
            CameraMode::Spectate => self.spectate(view, sprites),
            CameraMode::Free => self.fly(view),
            CameraMode::Watch => {}
            CameraMode::Fade => {
                if self.previous_mode != Some(CameraMode::Fade) {
                    self.fade_alarm = ticks(FADE_SECONDS);
                }
            }
        }
        self.previous_mode = Some(self.mode);
        if self.look_timer == 0 {
            let back = (RETURN_SPEED * step()).min(self.dist.abs());
            self.dist -= back * gm_sign(self.dist);
        }
        self.apply_shake();
        self.x = gm_clamp(self.x, 0.0, view.room_width - VIEW_WIDTH);
        self.y = gm_clamp(self.y, 0.0, view.room_height - VIEW_HEIGHT);
    }

    /// Alarm_1: nobody revived this player in the moment after dying, so the camera goes watching.
    fn fade_over(&mut self, view: &CameraView) {
        if view.own.is_some_and(|own| own.hp > 0) {
            return;
        }
        let watching_exe = self.spectating.and_then(|id| view.others.iter().find(|(other, _)| *other == id)).is_some_and(|(_, player)| player.character == Character::Exe);
        if watching_exe {
            return;
        }
        self.spectating = None;
        self.spectating_index = 0;
        self.mode = CameraMode::Spectate;
    }

    /// The free camera: the keys that walk a player move the view instead.
    fn fly(&mut self, view: &CameraView) {
        let axis = |less: u16, more: u16| f64::from(view.buttons.held(more)) - f64::from(view.buttons.held(less));
        self.x += axis(Buttons::LEFT, Buttons::RIGHT) * FREE_SPEED * step();
        self.y += axis(Buttons::UP, Buttons::DOWN) * FREE_SPEED * step();
    }

    /// Mode 2: the next player worth watching -- for someone who died, the next
    /// survivor who is neither EXE, a demon nor down; for a spectator, anyone at all
    /// who is not away in their pause menu.
    fn spectate(&mut self, view: &CameraView, sprites: &Sprites) {
        if self.spectating.is_none() && !view.others.is_empty() {
            let (id, player) = &view.others[self.spectating_index.min(view.others.len() - 1)];
            let watchable = match self.watch_everyone {
                true => !player.paused,
                false => !view.exe_ids.contains(id) && player.revival_times < 2 && !player.is_dead,
            };
            if watchable {
                self.spectating = Some(*id);
            } else {
                self.next_spectated(view.others.len());
            }
        }
        let target = self.spectating.and_then(|id| view.others.iter().find(|(other, _)| *other == id));
        match target {
            Some((_, player)) => {
                let (to_x, to_y) = (player.x.floor() - VIEW_WIDTH / 2.0, player.y.floor() - VIEW_HEIGHT / 2.0);
                let body = crate::core::collision::sprite_bbox(sprites.get(player.sprite_index), player.x, player.y, player.image_xscale, 1.0, 0.0);
                // distance_to_object from the camera's own position to the watched player's box.
                let dx = (body.left - self.x).max(self.x - body.right).max(0.0);
                let dy = (body.top - self.y).max(self.y - body.bottom).max(0.0);
                if dx.hypot(dy) > CATCH_UP_DISTANCE {
                    self.x += (to_x - self.x) * crate::core::config::eased_share(CATCH_UP_SHARE);
                    self.y += (to_y - self.y) * crate::core::config::eased_share(CATCH_UP_SHARE);
                } else {
                    (self.x, self.y) = (to_x, to_y);
                }
            }
            None => self.spectating = None,
        }
        if !self.locked && view.any_key_pressed {
            self.spectating = None;
            self.next_spectated(view.others.len());
        }
    }

    fn next_spectated(&mut self, player_count: usize) {
        self.spectating_index += 1;
        if self.spectating_index + 1 > player_count {
            self.spectating_index = 0;
        }
    }

    /// scr_camera_shake
    pub fn shake(&mut self, time: i32, magnitude: f64, fade: f64) {
        // The time comes in the original's 60 Hz steps.
        self.shake = Some(Shake { time: crate::core::config::original_ticks(time), magnitude, fade });
    }

    /// Draw_76 with global.cameraMode 0.
    fn follow(&mut self, player: &Player) {
        let centred_x = player.x.floor() - VIEW_WIDTH / 2.0;
        let centred_y = player.y.floor() - VIEW_HEIGHT / 2.0;
        self.x = centred_x;
        if !player.is_grounded {
            self.dist = self.y - centred_y;
            if self.dist.abs() > AIR_SLACK {
                self.dist = AIR_SLACK * gm_sign(self.dist);
                self.y = centred_y + self.dist;
            }
        } else {
            self.look(player);
            self.y = centred_y + self.dist;
        }
    }

    fn look(&mut self, player: &Player) {
        // scr_collision_objects_after: a hiding player holds the timer at its start.
        if player.is_hiding {
            self.look_timer = 0;
        }
        let direction = if player.is_looking_down {
            1.0
        } else if player.is_looking_up {
            -1.0
        } else {
            self.look_timer = 0;
            return;
        };
        self.look_timer += 1;
        if self.look_timer > ticks(LOOK_DELAY_SECONDS) && self.dist * direction < LOOK_DISTANCE {
            self.dist += LOOK_STEP * direction * step();
        }
    }

    fn apply_shake(&mut self) {
        let Some(shake) = self.shake.as_mut() else { return };
        shake.time -= 1;
        self.x += choose_sign() * shake.magnitude;
        self.y += choose_sign() * shake.magnitude;
        if shake.time <= 0 {
            shake.magnitude -= shake.fade * step();
            if shake.magnitude <= 0.0 {
                self.shake = None;
            }
        }
    }
}

/// choose(-1, 1)
fn choose_sign() -> f64 {
    if rand::gen_range(0, 2) == 0 {
        -1.0
    } else {
        1.0
    }
}

/// GameMaker clamp: the upper bound wins when the bounds cross (a room smaller than the view).
fn gm_clamp(value: f64, low: f64, high: f64) -> f64 {
    let raised = if value < low { low } else { value };
    if raised > high {
        high
    } else {
        raised
    }
}
