use cotton::prelude::*;
use cotton::prelude::result::Result as PResult;

use regex::Regex;
use diff::Result::*;

mod editor;
mod lines_editor;

use editor::{Ensure, EditStatus};
use lines_editor::LinesEditor;

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

fn edit(edit: Edit, input: impl Read) -> PResult<(Box<dyn Display>, EditStatus)> {
    let mut editor = LinesEditor::load(input).problem_while("reading input text file")?;

    let status = match edit {
        Edit::Line { value, ignore_whitespace, ensure } => {
            editor.edit_line(value, ignore_whitespace, ensure)?
        }
        Edit::LinePair { pair, multikey, ignore_whitespace, separator, ensure } => {
            editor.edit_pair(pair, multikey, ignore_whitespace, &separator, ensure)?
        }
    };

    Ok((Box::new(editor) as Box<dyn Display>, status))
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
