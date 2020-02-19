use cotton::prelude::*;
use cotton::prelude::result::Result as PResult;

use regex::Regex;
use diff::Result::*;

const NEW_LINE: &str = "\n";

#[derive(Debug, StructOpt)]
enum Edit {
    /// Edit line in text file
    Line {
        /// Line of text
        value: String,
        /// Ignore any white space at the beginning and end of each file line
        #[structopt(long, short = "w")]
        ignore_whitespace: bool,
        #[structopt(flatten)]
        ensure: Ensure,
    },
    /// Edit line in text file containing key and value pairs
    LinePair {
        /// Key and value pair
        pair: String,
        /// Allow multiple keys with different values
        #[structopt(long, short)]
        multikey: bool,
        /// Ignore any white space at the beginning and end of each file line
        #[structopt(long, short = "w")]
        ignore_whitespace: bool,
        /// Regular expression pattern matching separator of key and value pairs
        #[structopt(long, short, default_value = r#"(\s*=\s*)"#)]
        separator: Regex,
        #[structopt(flatten)]
        ensure: Ensure,
    },
}

#[derive(Debug, StructOpt)]
enum Ensure {
    /// Ensure value is present in file
    Present {
        #[structopt(flatten)]
        placement: Placement,
    },
    /// Ensure value is absent from file
    Absent,
}

#[derive(Debug, StructOpt)]
enum Placement {
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
enum AnchorRelation {
    /// Before matching anchor entry or at the end of the file
    Before,
    /// After matching anchor entry or at the end of the file
    After,
}

/// Declaratively applies edits to files of various formats
#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(flatten)]
    logging: LoggingOpt,

    /// Check if file would have changed and return with status 2 if it would
    #[structopt(long, short)]
    check: bool,

    /// Print difference from before and after edit
    #[structopt(long, short)]
    diff: bool,

    #[structopt(subcommand)]
    edit: Edit,

    /// Edit this file in place.
    #[structopt(long, short)]
    in_place: Option<PathBuf>,
}

#[derive(Debug)]
struct LinesEditor {
    lines: Vec<String>,
}

#[derive(Debug)]
enum ReplaceStatus {
    AlreadyPresent,
    Replaced,
}

#[derive(Debug)]
enum PresentStatus {
    AlreadyPresent,
    InsertedPlacement,
}

#[derive(Debug)]
enum AbsentStatus {
    AlreadyAbsent,
    Removed,
}

impl LinesEditor {
    fn load<R: Read>(data: R) -> PResult<LinesEditor> {
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

#[derive(Debug)]
enum EditStatus {
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
    fn has_changed(&self) -> bool {
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

fn edit(edit: Edit, input: impl Read) -> PResult<(Box<dyn Display>, EditStatus)> {
    let mut editor = LinesEditor::load(input).problem_while("reading input text file")?;

    Ok(match edit {
        Edit::Line { value, ignore_whitespace, ensure } => {
            let value_pattern = Regex::new(&if ignore_whitespace {
                format!(r#"^\s*{}\s*$"#, &regex::escape(&value))
            } else {
                format!(r#"^{}$"#, &regex::escape(&value))
            }).expect("failed to construct absent regex");

            let status = match ensure {
                Ensure::Present { placement } => {
                    info!("Ensuring line {:?} is preset", value);
                    editor.present(&value_pattern, value, &placement).map_err(|_| "Failed to find placement for the value")?.into()
                }
                Ensure::Absent => {
                    info!("Ensuring line {:?} is absent", value);
                    editor.absent(&value_pattern).into()
                }
            };

            debug!("{:?}:\n{:#?}", status, editor);
            (Box::new(editor) as Box<dyn Display>, status)
        }
        Edit::LinePair { pair, multikey, ignore_whitespace, separator, ensure } => {
            let (key, value) = separator.splitn(&pair, 2).collect_tuple().ok_or_problem("Failed to split given value as key and value pair with given separator pattern")?;

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
                    match editor.replaced(&pair_pattern, &replace_pattern, pair) {
                        Err(pair) => editor.present(&pair_pattern, pair, &placement).map_err(|_| "Key not found and failed to find placement for the pair")?.into(),
                        Ok(status) => status.into()
                    }
                }
                Ensure::Absent => {
                    info!("Ensuring key and value pair {:?} is absent", pair);
                    editor.absent(&pair_pattern).into()
                }
            };

            debug!("{:?}:\n{:#?}", status, editor);
            (Box::new(editor) as Box<dyn Display>, status)
        }
    })
}

//TODO:
// * tests
// * stream input to output with no buffering when possible
// * replaced -> substituted?
// * option to create a file if it does not exists (for present edits)
fn main() -> FinalResult {
    let args = Cli::from_args();
    init_logger(&args.logging, vec![module_path!()]);

    let mut diff_input = None;

    let mut input = args.in_place
        .as_ref()
        .map(|file| File::open(file).map(|f| Box::new(f) as Box<dyn Read>)).transpose().problem_while("opening file for reading")?
        .unwrap_or_else(|| Box::new(stdin()) as Box<dyn Read>);

    if args.diff {
        let mut input_data = String::new();
        input.read_to_string(&mut input_data).problem_while("reading input data")?;

        diff_input = Some(input_data);
        input = Box::new(std::io::Cursor::new(diff_input.as_ref().unwrap()));
    }

    let (edited, status) = edit(args.edit, input)?;

    info!("Edit result: {}", status);

    if let Some(input_data) = diff_input.as_ref() {
        if status.has_changed() {
            let output_data = edited.to_string();

            for diff in diff::lines(input_data, &output_data){
                match diff {
                    Left(line) => eprintln!("- {}", line),
                    Both(line, _) => eprintln!("  {}", line),
                    Right(line) => eprintln!("+ {}", line),
                }
            }
        }
    }

    if args.check {
        if status.has_changed() {
            Err(Problem::from_error("File would have changed (check)")).fatal_with_status(2)?;
        }
    } else {
        let mut output = args.in_place
            .as_ref()
            .map(|file| File::create(file).map(|f| Box::new(f) as Box<dyn Write>)).transpose().problem_while("opening file for writing")?
            .unwrap_or_else(|| Box::new(stdout()) as Box<dyn Write>);

        write!(output, "{}", edited)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
