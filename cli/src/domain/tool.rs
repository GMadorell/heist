use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Git,
    Gh,
    Crit,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Git, Tool::Gh, Tool::Crit];

    pub fn binary_name(&self) -> &'static str {
        match self {
            Tool::Git => "git",
            Tool::Gh => "gh",
            Tool::Crit => "crit",
        }
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.binary_name())
    }
}
