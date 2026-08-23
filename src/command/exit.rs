use std::process::ExitCode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandExit {
    Success,
    Usage,
    Unavailable,
    Internal,
    Child(u8),
}

impl CommandExit {
    pub(crate) fn from_clap(code: i32) -> Self {
        match code {
            0 => Self::Success,
            2 => Self::Usage,
            _ => Self::Internal,
        }
    }

    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Usage => 2,
            Self::Unavailable => 69,
            Self::Internal => 70,
            Self::Child(code) => code,
        }
    }
}

impl From<CommandExit> for ExitCode {
    fn from(value: CommandExit) -> Self {
        Self::from(value.code())
    }
}
