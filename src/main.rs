use cotton::prelude::*;
use cotton::prelude::result::Result as PResult;

use regex::Regex;

const NEW_LINE: &str = "\n";

#[derive(Debug, StructOpt)]
enum Edit {
    /// Edit line in text file
    Line {
        value: String,
        #[structopt(flatten)]
        ensure: Ensure,
    },
    /// Edit line in text file containing key and value pairs
    LineKeyValue {
        pair: String,
        /// Allow multiple keys with different values
        #[structopt(long, short)]
        multikey: bool,
        /// Pattern matching separator of key and value
        #[structopt(long, short, default_value = r#"([ \t]*=[ \t]*)"#)]
        separator: Regex,
        #[structopt(flatten)]
        ensure: Ensure,
    },
}

#[derive(Debug, StructOpt)]
enum Ensure {
    /// Ensure value is present in file at given placement
    Present {
        #[structopt(flatten)]
        placement: Placement,
    },
}

#[derive(Debug, StructOpt)]
enum Placement {
    /// Relative to another entry
    RelativeTo {
        #[structopt(flatten)]
        insert: Insert,
        pattern: Regex,
    },
    /// At the top of the file
    AtTop,
    /// At the end of the file
    AtEnd,
}

#[derive(Debug, StructOpt)]
enum Insert {
    /// Before matching entry or at the end
    Before,
    /// After matching entry or at the end
    After,
}

// https://docs.rs/structopt/0.3.2/structopt/index.html#how-to-derivestructopt
/// Does stuff
#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(flatten)]
    logging: LoggingOpt,

    #[structopt(flatten)]
    dry_run: DryRunOpt,

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

    fn present(&mut self, value: String, placement: &Placement) -> PresentStatus {
        if self.lines.contains(&value) {
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
            Placement::RelativeTo { pattern, insert } => {
                self.lines = self.lines.drain(..).into_iter().fold(Vec::new(), |mut out, line| {
                    let matched = value.is_some() && pattern.is_match(&line);

                    match insert {
                        Insert::Before => {
                            if matched {
                                out.push(value.take().unwrap());
                            }
                            out.push(line);
                        }
                        Insert::After => {
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

fn main() -> FinalResult {
    let args = Cli::from_args();
    init_logger(&args.logging, vec![module_path!()]);

    let input = args.in_place
        .as_ref()
        .map(|file| File::open(file).map(|f| Box::new(f) as Box<dyn Read>)).transpose()?
        .unwrap_or_else(|| Box::new(stdin()) as Box<dyn Read>);

    let edited = match args.edit {
        Edit::Line { value, ensure } => {
            let mut editor = LinesEditor::load(input)?;

            match ensure {
                Ensure::Present { placement } => {
                    info!("Ensuring line {:?} is preset at {:?}", value, placement);
                    let status = editor.present(value, &placement);
                    debug!("Present: {:?}", status);
                    debug!("{:#?}", editor);
                }
            }

            Box::new(editor) as Box<dyn Display>
        }
        Edit::LineKeyValue { pair, multikey, separator, ensure } => {
            let (key, value) = separator.splitn(&pair, 2).collect_tuple().or_failed_to("split given value as key and value pair with given separator pattern");

            let key_separator = if multikey {
                // Replace only for full key-value match
                Regex::new(&format!("^{}{}{}$", regex::escape(key), separator, regex::escape(value)))
            } else {
                Regex::new(&format!("^{}{}", regex::escape(key), separator))
            }.expect("failed to construct key_separator regex");

            let mut editor = LinesEditor::load(input)?;

            match editor.replaced(&key_separator, pair) {
                ReplaceStatus::NotReplaced(pair) => match ensure {
                    Ensure::Present { placement } => {
                        info!("Ensuring key and value pair {:?} is preset at {:?}", pair, placement);
                        let status = editor.present(pair, &placement);
                        debug!("Present: {:?}", status);
                    }
                }
                status @ ReplaceStatus::Replaced => debug!("Replace: {:?}", status),
            }

            debug!("{:#?}", editor);
            Box::new(editor) as Box<dyn Display>
        }
    };

    let mut output = args.in_place
        .as_ref()
        .map(|file| File::create(file).map(|f| Box::new(f) as Box<dyn Write>)).transpose()?
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
