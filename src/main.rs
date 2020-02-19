use cotton::prelude::*;
use cotton::prelude::result::Result as PResult;

use regex::Regex;
use diff::Result::*;
use std::io::Cursor;

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

    /// Edit this file in place.
    #[structopt(long, short)]
    in_place: Option<PathBuf>,

    /// Create in-place file is it does not exist
    #[structopt(long, short = "C")]
    create: bool,

    #[structopt(subcommand)]
    edit: Edit,
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
// * preserve no line eding on last line
fn main() -> FinalResult {
    let args = Cli::from_args();
    init_logger(&args.logging, vec![module_path!()]);

    let mut diff_input = None;

    let mut input = args.in_place
        .as_ref()
        .map(|file| {
            match (File::open(file).map(|f| Box::new(f) as Box<dyn Read>), args.create) {
                (Err(_), true) => Ok(Box::new(Cursor::new(String::new())) as Box<dyn Read>),
                (result, _) => result,
            }
        }).transpose().problem_while("opening file for reading")?
        .unwrap_or_else(|| Box::new(stdin()) as Box<dyn Read>);

    if args.diff {
        let mut input_data = String::new();
        input.read_to_string(&mut input_data).problem_while("reading input data")?;

        diff_input = Some(input_data);
        input = Box::new(Cursor::new(diff_input.as_ref().unwrap()));
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
    fn test_ssh_edit_key_multi_candind() {
        let err = stable_pedit(SSH_TEST, &[
              "line-pair",
              "-s", " ",
              r#"IdentityFile ~/.ssh/quix"#,
              "present",
              "at-top",
        ]).unwrap_err();
        assert_eq!(&err.to_string(), "Multiple candidates found");
    }

    #[test]
    fn test_edit_pair_value() -> FinalResult {
        let (output, status) = stable_pedit("foo = 1\nbar = 2\nbaz = 3", &[
              "line-pair",
              "bar = 4",
              "present",
              "at-top",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo = 1\nbar = 4\nbaz = 3\n");

        Ok(())
    }

    #[test]
    fn test_edit_pair_multikey_value() -> FinalResult {
        let (output, status) = stable_pedit("foo = 1\nbar = 2\nbaz = 3", &[
              "line-pair",
              "-m",
              "bar = 4",
              "present",
              "at-top",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "bar = 4\nfoo = 1\nbar = 2\nbaz = 3\n");

        Ok(())
    }

    #[test]
    fn test_edit_pair_multiple_candidates() {
        let err = stable_pedit("foo = 1\nbar = 2\nbar = 3\nbaz = 3", &[
              "line-pair",
              "bar = 4",
              "present",
              "at-top",
        ]).unwrap_err();

        assert_eq!(&err.to_string(), "Multiple candidates found");
    }

    #[test]
    fn test_edit_line_pair_relative_to_before_middle() -> FinalResult {
        let (output, status) = stable_pedit("foo = 1\nbar = 2\nbaz = 3", &[
              "line",
              "quix = 4",
              "present",
              "relative-to",
              "bar",
              "before",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo = 1\nquix = 4\nbar = 2\nbaz = 3\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_relative_to_before_top() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "quix",
              "present",
              "relative-to",
              "foo",
              "before",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "quix\nfoo\nbar\nbaz\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_relative_to_before_middle() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "quix",
              "present",
              "relative-to",
              "bar",
              "before",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo\nquix\nbar\nbaz\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_relative_to_before_end() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "quix",
              "present",
              "relative-to",
              "baz",
              "before",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo\nbar\nquix\nbaz\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_relative_to_after_top() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "quix",
              "present",
              "relative-to",
              "foo",
              "after",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo\nquix\nbar\nbaz\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_relative_to_after_middle() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "quix",
              "present",
              "relative-to",
              "bar",
              "after",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo\nbar\nquix\nbaz\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_relative_to_after_end() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "quix",
              "present",
              "relative-to",
              "baz",
              "after",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo\nbar\nbaz\nquix\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_relative_to_multiple_candidates() {
        let err = stable_pedit("foo\nfoo", &[
              "line",
              r#"bar"#,
              "present",
              "relative-to",
              "foo",
              "before",
        ]).unwrap_err();

        assert_eq!(&err.to_string(), "Multiple candidates found");
    }

    #[test]
    fn test_edit_line_absent_middle() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "bar",
              "absent",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo\nbaz\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_absent_top() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "foo",
              "absent",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "bar\nbaz\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_absent_end() -> FinalResult {
        let (output, status) = stable_pedit("foo\nbar\nbaz", &[
              "line",
              "baz",
              "absent",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo\nbar\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_absent_multiple_candidates() {
        let err = stable_pedit("foo\nbaz\nbaz", &[
              "line",
              "baz",
              "absent",
        ]).unwrap_err();

        assert_eq!(&err.to_string(), "Multiple candidates found");
    }

    #[test]
    fn test_edit_line_pair_absent_middle() -> FinalResult {
        let (output, status) = stable_pedit("foo = 1\nbar = 2\nbaz = 3", &[
              "line-pair",
              "bar = 2",
              "absent",
        ])?;

        assert!(status.has_changed());
        assert_eq!(&output, "foo = 1\nbaz = 3\n");

        Ok(())
    }

    #[test]
    fn test_edit_line_pair_absent_multiple_candidates() {
        let err = stable_pedit("foo = 1\nbaz = 2\nbaz = 2", &[
              "line-pair",
              "baz = 2",
              "absent",
        ]).unwrap_err();

        assert_eq!(&err.to_string(), "Multiple candidates found");
    }
}
