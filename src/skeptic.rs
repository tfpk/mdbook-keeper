use std::collections::HashMap;
use std::fs::File;
use std::io::{Error as IoError, Read};
use std::mem;
use std::path::Path;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag};

#[derive(Debug)]
pub enum Buffer {
    None,
    Code(Vec<String>),
    Heading(String),
}

pub fn extract_tests_from_string(s: &str, file_stem: &str) -> (Vec<Test>, Option<String>) {
    let mut tests = Vec::new();
    let mut buffer = Buffer::None;
    let parser = Parser::new(s);
    let mut section = None;
    let mut code_block_start = 0;
    // Oh this isn't actually a test but a legacy template
    let mut old_template = None;

    for (event, range) in parser.into_offset_iter() {
        let line_number = bytecount::count(&s.as_bytes()[0..range.start], b'\n');
        match event {
            Event::Start(Tag::Heading(level, ..)) if level < HeadingLevel::H3 => {
                buffer = Buffer::Heading(String::new());
            }
            Event::End(Tag::Heading(level, ..)) if level < HeadingLevel::H3 => {
                let cur_buffer = mem::replace(&mut buffer, Buffer::None);
                if let Buffer::Heading(sect) = cur_buffer {
                    section = Some(sanitize_test_name(&sect));
                }
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info))) => {
                let code_block_info = parse_code_block_info(info);
                if code_block_info.is_rust {
                    buffer = Buffer::Code(Vec::new());
                }
            }
            Event::Text(text) => {
                if let Buffer::Code(ref mut buf) = buffer {
                    if buf.is_empty() {
                        code_block_start = line_number;
                    }
                    buf.extend(text.lines().map(|s| format!("{}\n", s)));
                } else if let Buffer::Heading(ref mut buf) = buffer {
                    buf.push_str(&*text);
                }
            }
            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(ref info))) => {
                let code_block_info = parse_code_block_info(info);
                if let Buffer::Code(buf) = mem::replace(&mut buffer, Buffer::None) {
                    if code_block_info.is_old_template {
                        old_template = Some(buf.into_iter().collect())
                    } else {
                        let name = if let Some(ref section) = section {
                            format!("{}_sect_{}_line_{}", file_stem, section, code_block_start)
                        } else {
                            format!("{}_line_{}", file_stem, code_block_start)
                        };
                        tests.push(Test {
                            name,
                            text: buf,
                            ignore: code_block_info.ignore,
                            no_run: code_block_info.no_run,
                            should_panic: code_block_info.should_panic,
                            template: code_block_info.template,
                        });
                    }
                }
            }
            _ => (),
        }
    }
    (tests, old_template)
}

pub fn load_templates(path: &Path) -> Result<HashMap<String, String>, IoError> {
    let file_name = format!(
        "{}.skt.md",
        path.file_name().expect("no file name").to_string_lossy()
    );
    let path = path.with_file_name(&file_name);
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::new();

    let mut file = File::open(path)?;
    let s = &mut String::new();
    file.read_to_string(s)?;
    let parser = Parser::new(s);

    let mut code_buffer = None;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info))) => {
                let code_block_info = parse_code_block_info(info);
                if code_block_info.is_rust {
                    code_buffer = Some(Vec::new());
                }
            }
            Event::Text(text) => {
                if let Some(ref mut buf) = code_buffer {
                    buf.push(text.to_string());
                }
            }
            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(ref info))) => {
                let code_block_info = parse_code_block_info(info);
                if let Some(buf) = code_buffer.take() {
                    if let Some(t) = code_block_info.template {
                        map.insert(t, buf.into_iter().collect());
                    }
                }
            }
            _ => (),
        }
    }

    Ok(map)
}

pub fn sanitize_test_name(s: &str) -> String {
    s.to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii() && ch.is_alphanumeric() {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

pub fn parse_code_block_info(info: &str) -> CodeBlockInfo {
    // Same as rustdoc
    let tokens = info.split(|c: char| !(c == '_' || c == '-' || c.is_alphanumeric()));

    let mut seen_rust_tags = false;
    let mut seen_other_tags = false;
    let mut info = CodeBlockInfo {
        is_rust: false,
        should_panic: false,
        ignore: false,
        no_run: false,
        is_old_template: false,
        template: None,
    };

    for token in tokens {
        match token {
            "" => {}
            "rust" => {
                info.is_rust = true;
                seen_rust_tags = true
            }
            "should_panic" => {
                info.should_panic = true;
                seen_rust_tags = true
            }
            "ignore" => {
                info.ignore = true;
                seen_rust_tags = true
            }
            "no_run" => {
                info.no_run = true;
                seen_rust_tags = true;
            }
            "skeptic-template" => {
                info.is_old_template = true;
                seen_rust_tags = true
            }
            _ if token.starts_with("skt-") => {
                info.template = Some(token[4..].to_string());
                seen_rust_tags = true;
            }
            _ => seen_other_tags = true,
        }
    }

    info.is_rust &= !seen_other_tags || seen_rust_tags;

    info
}

#[derive(Debug)]
pub struct CodeBlockInfo {
    is_rust: bool,
    should_panic: bool,
    ignore: bool,
    no_run: bool,
    is_old_template: bool,
    template: Option<String>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Test {
    pub(crate) name: String,
    pub(crate) text: Vec<String>,
    pub(crate) ignore: bool,
    pub(crate) no_run: bool,
    pub(crate) should_panic: bool,
    pub(crate) template: Option<String>,
}

/// Just like Rustdoc, ignore a "#" sign at the beginning of a line of code.
/// These are commonly an indication to omit the line from user-facing
/// documentation but include it for the purpose of playground links or skeptic
/// testing.
#[allow(clippy::manual_strip)] // Relies on str::strip_prefix(), MSRV 1.45
fn clean_omitted_line(line: &str) -> &str {
    // XXX To silence depreciation warning of trim_left and not bump rustc
    // requirement upto 1.30 (for trim_start) we roll our own trim_left :(
    let trimmed = if let Some(pos) = line.find(|c: char| !c.is_whitespace()) {
        &line[pos..]
    } else {
        line
    };

    if trimmed.starts_with("# ") {
        &trimmed[2..]
    } else if line.trim() == "#" {
        // line consists of single "#" which might not be followed by newline on windows
        &trimmed[1..]
    } else {
        line
    }
}

/// Creates the Rust code that this test will be operating on.
pub fn create_test_input(lines: &[String]) -> String {
    lines
        .iter()
        .map(|s| clean_omitted_line(s).to_owned())
        .collect()
}
