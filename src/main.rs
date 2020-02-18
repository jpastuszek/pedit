use cotton::prelude::*;
use cotton::prelude::result::Result as PResult;

use regex::Regex;

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
    NotReplaced(String),
    Replaced,
}

#[derive(Debug)]
enum PresentStatus {
    AlreadyPresent,
    InsertedPlacement,
    InsertedFallback,
}

impl LinesEditor {
    fn load<R: Read>(data: R) -> PResult<LinesEditor> {
        Ok(LinesEditor {
            lines: BufReader::new(data).lines().collect::<Result<_, _>>()?
        })
    }

    fn replaced(&mut self, pattern: &Regex, value: String) -> ReplaceStatus {
        let mut value = Some(value);
        self.lines = self.lines.drain(..).into_iter().fold(Vec::new(), |mut out, line| {
            if pattern.is_match(&line) {
                if let Some(value) = value.take() {
                    out.push(value);
                }
                // else delete matching key
            } else {
                out.push(line);
            }
            out
        });

        if let Some(value) = value {
            ReplaceStatus::NotReplaced(value)
        } else {
            ReplaceStatus::Replaced
        }
    }

    fn present(&mut self, value_pattern: &Regex, value: String, placement: &Placement) -> PresentStatus {
        if self.lines.iter().any(|line| value_pattern.is_match(line)) {
            return PresentStatus::AlreadyPresent
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
            self.lines.push(value);
            return PresentStatus::InsertedFallback
        }

        PresentStatus::InsertedPlacement
    }

    fn absent(&mut self, pattern: &Regex) {
        self.lines = self.lines.drain(..).into_iter().fold(Vec::new(), |mut out, line| {
            if pattern.is_match(&line) {
                // delete matching key
            } else {
                out.push(line);
            }
            out
        });
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

fn edit(edit: Edit, input: impl Read) -> PResult<Box<dyn Display>> {
    Ok(match edit {
        Edit::Line { value, ignore_whitespace, ensure } => {
            let mut editor = LinesEditor::load(input)?;

            let value_pattern = Regex::new(&if ignore_whitespace {
                format!(r#"^\s*{}\s*$"#, &regex::escape(&value))
            } else {
                format!(r#"^{}$"#, &regex::escape(&value))
            }).expect("failed to construct absent regex");

            match ensure {
                Ensure::Present { placement } => {
                    info!("Ensuring line {:?} is preset", value);
                    let status = editor.present(&value_pattern, value, &placement);
                    debug!("Present: {:?}", status); }
                Ensure::Absent => {
                    info!("Ensuring line {:?} is absent", value);
                    editor.absent(&value_pattern);
                }
            }

            debug!("{:#?}", editor);
            Box::new(editor) as Box<dyn Display>
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

            let mut editor = LinesEditor::load(input)?;

            match ensure {
                Ensure::Present { placement } => {
                    info!("Ensuring key and value pair {:?} is preset", pair);
                    match editor.replaced(&replace_pattern, pair) {
                        ReplaceStatus::NotReplaced(pair) => {
                            let status = editor.present(&pair_pattern, pair, &placement);
                            debug!("Present: {:?}", status);
                        }
                        status @ ReplaceStatus::Replaced => debug!("Replace: {:?}", status),
                    }
                }
                Ensure::Absent => {
                    info!("Ensuring key and value pair {:?} is absent", pair);
                    editor.absent(&pair_pattern);
                }
            }
            debug!("{:#?}", editor);
            Box::new(editor) as Box<dyn Display>
        }
    })
}

fn main() -> FinalResult {
    let args = Cli::from_args();
    init_logger(&args.logging, vec![module_path!()]);

    let input = args.in_place
        .as_ref()
        .map(|file| File::open(file).map(|f| Box::new(f) as Box<dyn Read>)).transpose().problem_while("opening file for reading")?
        .unwrap_or_else(|| Box::new(stdin()) as Box<dyn Read>);

    let edited = edit(args.edit, input)?;

    let mut output = args.in_place
        .as_ref()
        .map(|file| File::create(file).map(|f| Box::new(f) as Box<dyn Write>)).transpose().problem_while("opening file for writing")?
        .unwrap_or_else(|| Box::new(stdout()) as Box<dyn Write>);

    write!(output, "{}", edited)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
