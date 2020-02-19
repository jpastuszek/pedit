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
    PlacementError(&'static str),
    InvalidPairOrSeparator,
}

impl fmt::Display for LinesEditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinesEditorError::PlacementError(context) => write!(f, "No placement found while {}", context),
            LinesEditorError::InvalidPairOrSeparator => write!(f, "Failed to split given value as key and value pair with given separator pattern"),
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

    fn replaced(&mut self, pair_pattern: &Regex, key_pattern: &Regex, value: String) -> Result<ReplaceStatus, String> {
        if self.lines.iter().any(|line| pair_pattern.is_match(line)) {
            return Ok(ReplaceStatus::AlreadyPresent)
        }

        let mut value = Some(value);

        self.lines = self.lines.drain(..).into_iter().fold(Vec::new(), |mut out, line| {
            if key_pattern.is_match(&line) && value.is_some() {
                out.push(value.take().unwrap());
            } else {
                out.push(line);
            }
            out
        });

        if let Some(value) = value {
            return Err(value)
        }

        Ok(ReplaceStatus::Replaced)
    }

    fn present(&mut self, value_pattern: &Regex, value: String, placement: &Placement) -> Result<PresentStatus, String> {
        if self.lines.iter().any(|line| value_pattern.is_match(line)) {
            return Ok(PresentStatus::AlreadyPresent)
        }

        let mut value = Some(value);

        match placement {
            Placement::AtTop => {
                self.lines.insert(0, value.take().unwrap());
            }
            Placement::AtEnd => {
                self.lines.push(value.take().unwrap());
            }
            Placement::RelativeTo { anchor, relation } => {
                self.lines = self.lines.drain(..).into_iter().fold(Vec::new(), |mut out, line| {
                    let matched = value.is_some() && anchor.is_match(&line);

                    match relation {
                        AnchorRelation::Before => {
                            if matched {
                                out.push(value.take().unwrap());
                            }
                            out.push(line);
                        }
                        AnchorRelation::After => {
                            out.push(line);
                            if matched {
                                out.push(value.take().unwrap());
                            }
                        }
                    }
                    out
                });
            }
        }

        if let Some(value) = value {
            return Err(value)
        }

        Ok(PresentStatus::InsertedPlacement)
    }

    fn absent(&mut self, pattern: &Regex) -> AbsentStatus {
        let mut removed = false;
        self.lines = self.lines.drain(..).into_iter().fold(Vec::new(), |mut out, line| {
            if pattern.is_match(&line) {
                removed = true
            } else {
                out.push(line);
            }
            out
        });

        if removed {
            AbsentStatus::Removed
        } else {
            AbsentStatus::AlreadyAbsent
        }
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
                self.present(&value_pattern, value, &placement)
                    .map_err(|_| LinesEditorError::PlacementError("ensuring line is present"))?
                    .into()
            }
            Ensure::Absent => {
                info!("Ensuring line {:?} is absent", value);
                self.absent(&value_pattern).into()
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
                    Err(pair) => self.present(&pair_pattern, pair, &placement)
                        .map_err(|_| LinesEditorError::PlacementError("ensuring key and value is present"))?
                        .into(),
                    Ok(status) => status.into()
                }
            }
            Ensure::Absent => {
                info!("Ensuring key and value pair {:?} is absent", pair);
                self.absent(&pair_pattern).into()
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
