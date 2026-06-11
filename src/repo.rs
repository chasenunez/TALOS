use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Push,
    Diverged,
    Pull,
    NoUpstream,
    Synced,
    NotRepo,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            State::Push => "push",
            State::Diverged => "diverged",
            State::Pull => "pull",
            State::NoUpstream => "no-upstream",
            State::Synced => "synced",
            State::NotRepo => "not-repo",
        }
    }

    pub fn color(self) -> Color {
        match self {
            State::Push => Color::Red,
            State::Diverged | State::Pull => Color::Yellow,
            State::NoUpstream => Color::Cyan,
            State::Synced => Color::Green,
            State::NotRepo => Color::Reset,
        }
    }
}

pub struct Repo {
    pub name: String,
    pub state: State,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
    pub branch: String,
    pub last_commit: String,
    pub commits: u64,
}

impl Repo {
    pub fn placeholder(name: String, state: State) -> Self {
        Self {
            name,
            state,
            dirty: false,
            ahead: 0,
            behind: 0,
            branch: "(n/a)".into(),
            last_commit: "N/A".into(),
            commits: 0,
        }
    }
}
