use cotton::prelude::*;
use cotton::prelude::result::Result as PResult;

use regex::Regex;

const NEW_LINE: &str = "\n";

/// Interpret file as give file format
#[derive(Debug, StructOpt)]
enum Format {
    /// Text file
    Lines {
        #[structopt(flatten)]
        edit: Edit,
    }
}

#[derive(Debug, StructOpt)]
enum Edit {
    /// Ensure value is present in file at given placement
    Present {
        value: String,
        #[structopt(flatten)]
        placement: Placement,
    }
}

#[derive(Debug, StructOpt)]
enum Placement {
    /// Insert value before matching entry or at the end
    Before {
        pattern: Regex,
    },
    /// Insert value after matching entry or at the end
    After {
        pattern: Regex,
    },
    /// At the top of the fule
    Top,
    /// At the end of the file
    End,
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
    format: Format,

    /// Edit this file in place.
    #[structopt(long, short)]
    in_place: Option<PathBuf>,
}

#[derive(Debug)]
struct LinesEditor {
    lines: Vec<String>,
}

impl LinesEditor {
    fn load<R: Read>(data: R) -> PResult<LinesEditor> {
        Ok(LinesEditor {
            lines: BufReader::new(data).lines().collect::<Result<_, _>>()?
        })
    }

    fn present(&mut self, value: String, placement: &Placement) -> PResult<()> {
        info!("Present: {:?} {:?}", value, placement);


        //TODO: don't insert twice if already present!

        match placement {
            Placement::Top => {
                self.lines.insert(0, value);
            }
            Placement::End => {
                self.lines.push(value);
            }
            _ => {
                let mut value = Some(value);

                let mut lines = self.lines.drain(..).into_iter().fold(Vec::new(), |mut out, line| {
                    match placement {
                        Placement::Before { pattern } => {
                            if value.is_some() && pattern.is_match(&line) {
                                out.push(value.take().unwrap());
                            }
                            out.push(line);
                        }
                        Placement::After { pattern } => {
                            let push = value.is_some() && pattern.is_match(&line);
                            out.push(line);
                            if push {
                                out.push(value.take().unwrap());
                            }
                        }
                        Placement::Top | Placement::End => panic!("top|end"),
                    }
                    out
                });

                if let Some(value) = value {
                    lines.push(value);
                }

                self.lines = lines;
            }
        }

        dbg![self];

        Ok(())
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

    match args.format {
        Format::Lines { edit } => {
            let mut editor = LinesEditor::load(input)?;

            dbg![&editor];

            match edit {
                Edit::Present { value, placement } => {
                    editor.present(value, &placement)?;
                }
            }

            let mut output = args.in_place
                .as_ref()
                .map(|file| File::create(file).map(|f| Box::new(f) as Box<dyn Write>)).transpose()?
                .unwrap_or_else(|| Box::new(stdout()) as Box<dyn Write>);

            write!(output, "{}", editor);
        }
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
