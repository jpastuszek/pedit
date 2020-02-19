use cotton::prelude::*;
use regex::Regex;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum Ensure {
    /// Ensure value is present in file
    Present {
        #[structopt(flatten)]
        placement: Placement,
    },
    /// Ensure value is absent from file
    Absent,
}

#[derive(Debug, StructOpt)]
pub enum Placement {
    /// Relative to existing anchor entry
    RelativeTo {
        #[structopt(flatten)]
        relation: AnchorRelation,
        /// Regular expression pattern matching anchor value
        anchor: Regex,
    },
    /// At the top of the file
    AtTop,
    /// At the end of the file
    AtEnd,
}

#[derive(Debug, StructOpt)]
pub enum AnchorRelation {
    /// Before matching anchor entry or at the end of the file
    Before,
    /// After matching anchor entry or at the end of the file
    After,
}

#[derive(Debug)]
pub enum ReplaceStatus {
    AlreadyPresent,
    Replaced,
}

#[derive(Debug)]
pub enum PresentStatus {
    AlreadyPresent,
    InsertedPlacement,
}

#[derive(Debug)]
pub enum AbsentStatus {
    AlreadyAbsent,
    Removed,
}

#[derive(Debug)]
pub enum EditStatus {
    Replaced(ReplaceStatus),
    Present(PresentStatus),
    Absent(AbsentStatus),
}

impl From<ReplaceStatus> for EditStatus {
    fn from(s: ReplaceStatus) -> EditStatus {
        EditStatus::Replaced(s)
    }
}

impl From<PresentStatus> for EditStatus {
    fn from(s: PresentStatus) -> EditStatus {
        EditStatus::Present(s)
    }
}

impl From<AbsentStatus> for EditStatus {
    fn from(s: AbsentStatus) -> EditStatus {
        EditStatus::Absent(s)
    }
}

impl EditStatus {
    pub fn has_changed(&self) -> bool {
        match self {
            EditStatus::Replaced(ReplaceStatus::AlreadyPresent)  |
            EditStatus::Present(PresentStatus::AlreadyPresent) |
            EditStatus::Absent(AbsentStatus::AlreadyAbsent) => false,
            _ => true,
        }
    }
}

impl fmt::Display for EditStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.has_changed() {
            write!(f, "no change made")
        } else {
            match self {
                EditStatus::Replaced(_) => write!(f, "value was replaced"),
                EditStatus::Present(_) => write!(f, "value was inserted"),
                EditStatus::Absent(_) => write!(f, "value was removed"),
            }
        }
    }
}
