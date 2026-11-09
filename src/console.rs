use std::fmt::Arguments;
use std::io::Write as _;

pub(crate) fn say(line: Arguments<'_>) {
    let _ = writeln!(std::io::stdout().lock(), "{line}");
}

pub(crate) fn complain(line: Arguments<'_>) {
    let _ = writeln!(std::io::stderr().lock(), "{line}");
}
