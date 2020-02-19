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

fn edit(input: impl Read, edit: Edit) -> PResult<(Box<dyn Display>, EditStatus)> {
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
// * line-pair -> line-kv?
// * top/end -> begginging/end or head/tail?
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

    let (edited, status) = edit(input, args.edit)?;

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
    use super::*;
    use std::io::Cursor;

    const XML_TEST: &str =
r#"<LayoutModificationTemplate
    xmlns="http://schemas.microsoft.com/Start/2014/LayoutModification"
    xmlns:defaultlayout="http://schemas.microsoft.com/Start/2014/FullDefaultLayout"
    xmlns:start="http://schemas.microsoft.com/Start/2014/StartLayout"
    xmlns:taskbar="http://schemas.microsoft.com/Start/2014/TaskbarLayout"
    Version="1">
  <CustomTaskbarLayoutCollection PinListPlacement="Replace">
    <defaultlayout:TaskbarLayout>
      <taskbar:TaskbarPinList>
        <taskbar:DesktopApp DesktopApplicationLinkPath="C:\Users\Administrator\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Scoop Apps\Process Explorer.lnk" />
        <taskbar:DesktopApp DesktopApplicationLinkPath="C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Administrative Tools\IIS Manager.lnk" />
      </taskbar:TaskbarPinList>
    </defaultlayout:TaskbarLayout>
  </CustomTaskbarLayoutCollection>
</LayoutModificationTemplate>"#;

    const SSH_TEST: &str =
r#"UserKnownHostsFile /dev/null
StrictHostKeyChecking no
IdentityFile ~/.ssh/foo
IdentityFile ~/.ssh/bar

Host *.foo.example.com
    User Administrator
"#;

    /// Applies edit to input
    fn pedit(input: &str, args: &[&str]) -> PResult<(String, EditStatus)> {
        let cli = Cli::from_iter_safe(Some("pedit").iter().chain(args.iter())).or_failed_to("bad args");
        let args = dbg![cli.edit];
        let (disp, status) = edit(Cursor::new(input), args)?;
        let out = disp.to_string();
        dbg![&status];
        eprintln!("{}", out);
        Ok((out, status))
    }

    /// Applies edit to input also verifying that subsequent application on result won't change anything
    fn stable_pedit(input: &str, args: &[&str]) -> PResult<(String, EditStatus)> {
        let (output, status) = pedit(input, args)?;

        // Second run on result should have not changes
        let (output2, status2) = pedit(&output, args)?;
        assert!(!status2.has_changed());
        assert_eq!(output, output2);

        Ok((output, status))
    }

    #[test]
    fn test_xml_edit() -> FinalResult {
        let (output, status) = stable_pedit(XML_TEST, &[
              "line",
              r#"        <taskbar:DesktopApp DesktopApplicationLinkPath="C:\ProgramData\foo.exe" />"#,
              "present",
              "relative-to",
              "</taskbar:TaskbarPinList>",
              "before"
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output,
r#"<LayoutModificationTemplate
    xmlns="http://schemas.microsoft.com/Start/2014/LayoutModification"
    xmlns:defaultlayout="http://schemas.microsoft.com/Start/2014/FullDefaultLayout"
    xmlns:start="http://schemas.microsoft.com/Start/2014/StartLayout"
    xmlns:taskbar="http://schemas.microsoft.com/Start/2014/TaskbarLayout"
    Version="1">
  <CustomTaskbarLayoutCollection PinListPlacement="Replace">
    <defaultlayout:TaskbarLayout>
      <taskbar:TaskbarPinList>
        <taskbar:DesktopApp DesktopApplicationLinkPath="C:\Users\Administrator\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Scoop Apps\Process Explorer.lnk" />
        <taskbar:DesktopApp DesktopApplicationLinkPath="C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Administrative Tools\IIS Manager.lnk" />
        <taskbar:DesktopApp DesktopApplicationLinkPath="C:\ProgramData\foo.exe" />
      </taskbar:TaskbarPinList>
    </defaultlayout:TaskbarLayout>
  </CustomTaskbarLayoutCollection>
</LayoutModificationTemplate>
"#);

        Ok(())
    }

    #[test]
    fn test_ssh_edit_key_value() -> FinalResult {
        let (output, status) = stable_pedit(SSH_TEST, &[
              "line-pair",
              "-s", " ",
              r#"StrictHostKeyChecking yes"#,
              "present",
              "at-end",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output,
r#"UserKnownHostsFile /dev/null
StrictHostKeyChecking yes
IdentityFile ~/.ssh/foo
IdentityFile ~/.ssh/bar

Host *.foo.example.com
    User Administrator
"#);
        Ok(())
    }

    #[test]
    fn test_ssh_edit_multikey_value() -> FinalResult {
        let (output, status) = stable_pedit(SSH_TEST, &[
              "line-pair",
              "-s", " ",
              "-m",
              r#"IdentityFile ~/.ssh/quix"#,
              "present",
              "at-top",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output,
r#"IdentityFile ~/.ssh/quix
UserKnownHostsFile /dev/null
StrictHostKeyChecking no
IdentityFile ~/.ssh/foo
IdentityFile ~/.ssh/bar

Host *.foo.example.com
    User Administrator
"#);
        Ok(())
    }

    #[test]
    fn test_ssh_edit_key_multiple_match() {
        let err = stable_pedit(SSH_TEST, &[
              "line-pair",
              "-s", " ",
              r#"IdentityFile ~/.ssh/quix"#,
              "present",
              "at-top",
        ]).unwrap_err();
        assert_eq!(&err.to_string(), "Multiple matches found");
    }
}
