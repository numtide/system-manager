mod hashset;
mod ordset;
mod vector;

fn code_fmt(code: &str) -> String {
    // use syntect::easy::HighlightLines;
    // use syntect::highlighting::{Style, ThemeSet};
    // use syntect::parsing::SyntaxSet;
    // use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
    //
    // let ps = SyntaxSet::load_defaults_newlines();
    // let ts = ThemeSet::load_defaults();
    // let syntax = ps.find_syntax_by_extension("rs").unwrap();
    // let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
    // let mut out = String::from("\n\n");
    // for line in LinesWithEndings::from(&code) {
    //     let ranges: Vec<(Style, &str)> = h.highlight(line, &ps);
    //     let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
    //     out += &escaped;
    // }
    // out += "\n\x1b[0m";
    // out
    code.to_string()
}
