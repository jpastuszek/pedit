use crate::editor::*;

use cotton::prelude::*;
use regex::Regex;
use std::error::Error;

const NEW_LINE: &str = "\n";

#[derive(Debug)]
pub struct LinesEditor {
    lines: Vec<String>,
}

#[derive(Debug)]
pub enum LinesEditorError {
    InvalidPairOrSeparator,
    MultipleMatch,
    NotApplicable(String),
}

impl fmt::Display for LinesEditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinesEditorError::InvalidPairOrSeparator => write!(f, "Failed to split given value as key and value pair with given separator pattern"),
            LinesEditorError::MultipleMatch => write!(f, "Multiple matches found"),
            LinesEditorError::NotApplicable(_) => write!(f, "Edit was not applicable"),
        }
    }
}

impl Error for LinesEditorError {}

impl LinesEditor {
    pub fn load<R: Read>(data: R) -> Result<LinesEditor, std::io::Error> {
        Ok(LinesEditor {
            lines: BufReader::new(data).lines().collect::<Result<_, _>>()?
        })
    }

    fn replaced(&mut self, pair_pattern: &Regex, key_pattern: &Regex, value: String) -> Result<ReplaceStatus, LinesEditorError> {
        let mut iter = self.lines.iter_mut();
        if let Some(line) = (&mut iter).find(|line| key_pattern.is_match(line)) {
            if iter.any(|line| key_pattern.is_match(line)) {
                return Err(LinesEditorError::MultipleMatch)
            }

            if pair_pattern.is_match(line) {
                return Ok(ReplaceStatus::AlreadyPresent)
            }

            *line = value;
        } else {
            return Err(LinesEditorError::NotApplicable(value))
        }

        Ok(ReplaceStatus::Replaced)
    }

    fn present(&mut self, value_pattern: &Regex, value: String, placement: &Placement) -> Result<PresentStatus, LinesEditorError> {
        if self.lines.iter().any(|line| value_pattern.is_match(line)) {
            return Ok(PresentStatus::AlreadyPresent)
        }

        match placement {
            Placement::AtTop => {
                self.lines.insert(0, value);
            }
            Placement::AtEnd => {
                self.lines.push(value);
            }
            Placement::RelativeTo { anchor, relation } => {
                let mut iter = self.lines.iter();
                if let Some(position) = (&mut iter).position(|line| anchor.is_match(line)) {
                    if iter.any(|line| anchor.is_match(line)) {
                        return Err(LinesEditorError::MultipleMatch)
                    }

                    match relation {
                        AnchorRelation::Before => self.lines.insert(position, value),
                        AnchorRelation::After =>  self.lines.insert(position + 1, value),
                    }
                } else {
                    return Err(LinesEditorError::NotApplicable(value))
                }
            }
        }

        Ok(PresentStatus::InsertedPlacement)
    }

    fn absent(&mut self, pattern: &Regex) -> Result<AbsentStatus, LinesEditorError> {
        let mut iter = self.lines.iter();
        if let Some(position) = (&mut iter).position(|line| pattern.is_match(line)) {
            if iter.any(|line| pattern.is_match(line)) {
                return Err(LinesEditorError::MultipleMatch)
            }

            self.lines.remove(position);
        } else {
            return Ok(AbsentStatus::AlreadyAbsent)
        }

        Ok(AbsentStatus::Removed)
    }

    pub fn edit_line(&mut self, value: String, ignore_whitespace: bool, ensure: Ensure) -> Result<EditStatus, LinesEditorError> {
        let value_pattern = Regex::new(&if ignore_whitespace {
            format!(r#"^\s*{}\s*$"#, &regex::escape(&value))
        } else {
            format!(r#"^{}$"#, &regex::escape(&value))
        }).expect("failed to construct absent regex");

        let status = match ensure {
            Ensure::Present { placement } => {
                info!("Ensuring line {:?} is preset", value);
                self.present(&value_pattern, value, &placement)?.into()
            }
            Ensure::Absent => {
                info!("Ensuring line {:?} is absent", value);
                self.absent(&value_pattern)?.into()
            }
        };

        debug!("Edit line:\n{:?}:\n{:#?}", status, self);
        Ok(status)
    }

    pub fn edit_pair(&mut self, pair: String, multikey: bool, ignore_whitespace: bool, separator: &Regex, ensure: Ensure) -> Result<EditStatus, LinesEditorError> {
        let (key, value) = separator.splitn(&pair, 2).collect_tuple().ok_or(LinesEditorError::InvalidPairOrSeparator)?;

        let pair_pattern = Regex::new(&if ignore_whitespace {
            format!(r#"^\s*{}{}{}\s*$"#, regex::escape(key), separator, regex::escape(value))
        } else {
            format!(r#"^{}{}{}$"#, regex::escape(key), separator, regex::escape(value))
        }).expect("failed to construct pair_pattern regex");

        let replace_pattern = if multikey {
            // Replace only for full key-value match
            pair_pattern.clone()
        } else {
            Regex::new(&if ignore_whitespace {
                format!(r#"^\s*{}{}"#, regex::escape(key), separator)
            } else {
                format!(r#"^{}{}"#, regex::escape(key), separator)
            }).expect("failed to construct replace_pattern regex")
        };

        let status = match ensure {
            Ensure::Present { placement } => {
                info!("Ensuring key and value pair {:?} is preset", pair);
                match self.replaced(&pair_pattern, &replace_pattern, pair) {
                    Err(LinesEditorError::NotApplicable(pair)) => self.present(&pair_pattern, pair, &placement)?.into(),
                    Err(err) => return Err(err),
                    Ok(status) => status.into()
                }
            }
            Ensure::Absent => {
                info!("Ensuring key and value pair {:?} is absent", pair);
                self.absent(&pair_pattern)?.into()
            }
        };

        debug!("Edit pair:\n{:?}:\n{:#?}", status, self);
        Ok(status)
    }
}

impl fmt::Display for LinesEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            f.write_str(line)?;
            f.write_str(NEW_LINE)?;
        }
        Ok(())
    }
}
