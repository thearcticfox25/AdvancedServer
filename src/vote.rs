pub const TICKS_PER_SEC: f64 = 60.0;
pub const VOTE_TIMEOUT_TICKS: f64 = 20.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteType {
    Kick,
    Map,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteState {
    Success,
    AlreadyVoted,
    Full,
}

#[derive(Debug, Clone)]
pub struct Vote {
    pub vote_type: VoteType,
    pub ongoing: bool,
    pub votes: Vec<u16>,
    pub votetotal: u8,
    pub countdown: f64,
}

impl Default for Vote {
    fn default() -> Self {
        Self {
            vote_type: VoteType::Kick,
            ongoing: false,
            votes: Vec::new(),
            votetotal: 0,
            countdown: 0.0,
        }
    }
}

impl Vote {
    pub fn init(&mut self, participants: &[u16], excluded_id: u16, vote_type: VoteType) -> bool {
        self.votes.clear();
        self.votetotal = 0;
        self.vote_type = vote_type;

        for &id in participants {
            if id != excluded_id {
                self.votetotal += 1;
            }
        }

        if self.votetotal <= 1 {
            return false;
        }

        self.ongoing = true;
        self.countdown = VOTE_TIMEOUT_TICKS;
        true
    }

    pub fn add(&mut self, id: u16) -> VoteState {
        if self.votes.contains(&id) {
            return VoteState::AlreadyVoted;
        }
        self.votes.push(id);
        if self.votes.len() as u8 >= self.votetotal {
            VoteState::Full
        } else {
            VoteState::Success
        }
    }

    pub fn tick(&mut self, delta: f64) -> bool {
        self.countdown -= delta;
        self.countdown > 0.0
    }

    pub fn succeeded(&self) -> bool {
        self.votes.len() as f64 > self.votetotal as f64 / 2.0
    }
}
