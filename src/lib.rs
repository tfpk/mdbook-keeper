mod run_tests;
mod skeptic;

#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::prelude::*;

use atty::Stream;
use colored::{control::set_override, Colorize};
use glob::glob;
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use serde::{Deserialize, Serialize};
use slug::slugify;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::value::Table;

use run_tests::{handle_test, CompileType, TestResult};
use skeptic::{create_test_input, extract_tests_from_string, Test};

type PreprocessorConfig<'a> = Option<&'a Table>;

fn get_tests_from_book(book: &Book) -> Vec<Test> {
    get_tests_from_items(&book.sections)
}

fn get_tests_from_items(items: &[BookItem]) -> Vec<Test> {
    let chapters = items.iter().filter_map(|b| match *b {
        BookItem::Chapter(ref ch) => Some(ch),
        _ => None,
    });

    chapters
        .flat_map(|c| {
            let file_name = c
                .path
                .as_ref()
                .and_then(|x| x.file_stem())
                .map(|x| x.to_string_lossy().into_owned())
                .unwrap_or_else(|| slugify(c.name.clone()).replace('-', "_"));
            let (mut tests, _) = extract_tests_from_string(&c.content, &file_name);
            tests.append(&mut get_tests_from_items(&c.sub_items));
            tests
        })
        .collect::<Vec<_>>()
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct KeeperConfigParser {
    /// This is unfortunately necessary thanks to how
    /// rust-skeptic parses code examples, and how rustc
    /// works. Any libraries named here will be passed as
    /// `--extern <lib>` options to rustc. This has
    /// the equivalent effect of putting an `extern crate <lib>;`
    /// line at the start of every example.
    #[serde(default)]
    externs: Vec<String>,

    /// This is where we keep all the intermediate work
    /// of testing. If it's not specified, it's a folder
    /// inside build. If it doesn't exist, we create it.
    #[serde(default)]
    test_dir: Option<String>,

    /// If you're building this book in the repo for a
    /// real binary/library; this should point to the target
    /// dir for that binary/library.
    #[serde(default)]
    target_dir: Option<String>,

    /// This is the path of a folder that should contain
    /// a `Cargo.toml`. If there is one there, you should
    /// assume a `Cargo.lock` will be created in the same
    /// place if it doesn't already exist.
    #[serde(default)]
    manifest_dir: Option<String>,

    /// Whether to show terminal colours.
    #[serde(default)]
    terminal_colors: Option<bool>,
}

#[derive(Debug)]
struct KeeperConfig {
    test_dir: PathBuf,
    target_dir: PathBuf,
    manifest_dir: Option<PathBuf>,
    terminal_colors: bool,
    externs: Vec<String>,
}

impl KeeperConfig {
    fn new(preprocessor_config: PreprocessorConfig, root: &Path) -> KeeperConfig {
        let keeper_config: KeeperConfigParser = match preprocessor_config {
            Some(config) => toml::de::from_str(
                &toml::ser::to_string(&config).expect("this must succeed, it was just toml"),
            )
            .unwrap(),
            None => KeeperConfigParser::default(),
        };

        let base_dir = root.to_path_buf();
        let test_dir = keeper_config
            .test_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut build_dir = base_dir;
                build_dir.push("doctest_cache");
                build_dir
            });

        let target_dir = keeper_config
            .target_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut target_dir = test_dir.clone();
                target_dir.push("target");
                target_dir
            });

        let manifest_dir = keeper_config.manifest_dir.map(PathBuf::from);

        let terminal_colors = keeper_config
            .terminal_colors
            .unwrap_or_else(|| atty::is(Stream::Stderr));

        set_override(terminal_colors);

        KeeperConfig {
            test_dir,
            target_dir,
            manifest_dir,
            terminal_colors,
            externs: keeper_config.externs,
        }
    }

    fn setup_environment(&self) {
        if !self.test_dir.is_dir() {
            std::fs::create_dir(&self.test_dir).unwrap();
        }

        if let Some(manifest_dir) = &self.manifest_dir {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
            let mut command = Command::new(cargo);
            command
                .arg("build")
                .current_dir(manifest_dir)
                .env("CARGO_TARGET_DIR", &self.target_dir)
                .env("CARGO_MANIFEST_DIR", manifest_dir);

            let mut join_handle = command.spawn().expect("failed to execute process");

            let build_was_ok = join_handle.wait().expect("Could not join on thread");

            if !build_was_ok.success() {
                panic!("cargo build failed!");
            }
        }
    }
}

fn get_test_path(test: &Test, test_dir: &Path) -> PathBuf {
    let mut file_name: PathBuf = test_dir.to_path_buf();
    file_name.push(format!("keeper_{}.rs", test.hash));

    file_name
}

fn write_test_to_path(test: &Test, path: &Path) -> Result<(), std::io::Error> {
    let mut output = File::create(path)?;
    let test_text = create_test_input(&test.text);
    write!(output, "{}", test_text)?;

    Ok(())
}

fn run_tests_with_config(tests: Vec<Test>, config: &KeeperConfig) -> HashMap<Test, TestResult> {
    let mut results = HashMap::new();
    for test in tests {
        if test.ignore {
            continue;
        }
        let testcase_path = get_test_path(&test, &config.test_dir);

        let result: TestResult = if !testcase_path.is_file() {
            write_test_to_path(&test, &testcase_path).unwrap();
            handle_test(
                config.manifest_dir.as_deref(),
                &config.target_dir,
                current_platform::CURRENT_PLATFORM,
                &testcase_path,
                if test.no_run {
                    CompileType::Check
                } else {
                    CompileType::Full
                },
                config.terminal_colors,
                &config.externs,
            )
        } else {
            TestResult::Cached
        };
        results.insert(test, result);
    }

    results
}

fn print_results(results: &HashMap<Test, TestResult>) {
    let mut cached_tests = 0;
    for (test, test_result) in results {
        if !matches!(test_result, &TestResult::Cached) {
            eprint!(" - Test: {} ", test.name);
        }
        let output = match test_result {
            TestResult::CompileFailed(output) if test.compile_fail => {
                eprintln!("{}", "(Failed to compile as expected)".green());
                output
            }
            TestResult::CompileFailed(output) => {
                eprintln!("{}", "(Failed to compile)".red());
                output
            }
            TestResult::RunFailed(output) if test.should_panic => {
                eprintln!("{}", "(Panicked as expected)".green());
                output
            }
            TestResult::RunFailed(output) => {
                eprintln!("{}", "(Panicked)".red());
                output
            }
            TestResult::Successful(output) if test.should_panic => {
                eprintln!("{}", "(Unexpectedly suceeded)".red());
                output
            }
            TestResult::Successful(output) => {
                eprintln!("{}", "(Passed)".green());
                output
            }
            TestResult::Cached => {
                cached_tests += 1;
                continue;
            }
        };
        if !test_result.met_test_expectations(test) {
            eprintln!(
                "--------------- {} {} ---------------",
                "Start of Test Log: ".bold(),
                test.name
            );
            if !output.stdout.is_empty() {
                eprintln!(
                    "----- {} -----\n{}",
                    "Stdout".bold(),
                    String::from_utf8(output.stdout.to_vec()).unwrap()
                );
            } else {
                eprintln!("{}", "No stdout was captured.".red(),);
            }
            if !output.stderr.is_empty() {
                eprintln!(
                    "----- {} -----\n\n{}",
                    "Stderr".bold(),
                    String::from_utf8(output.stderr.to_vec()).unwrap()
                );
            } else {
                eprintln!("{}", "No stderr was captured.".red(),);
            }
            eprintln!("--------------- End Of Test ---------------");
        }
    }

    if cached_tests > 0 {
        eprintln!(
            "{} {} {}",
            "Skipped".bold(),
            cached_tests.to_string().bold().blue(),
            "tests which had identical code, and previously passed.".bold()
        );
    }
}

fn clean_file(test_results: &HashMap<Test, TestResult>, path: &Path) -> Option<()> {
    // If the file doesn't contain a hash in the right format, we quit.
    let file_stem = path.file_stem()?;
    let file_str = file_stem.to_str()?;
    let hash = file_str.strip_prefix("keeper_")?;

    let matching_test = test_results.iter().find(|(t, _)| t.hash == hash);

    let should_remove = match matching_test {
        Some((t, tr)) => !tr.met_test_expectations(t),
        None => true,
    };

    if should_remove {
        std::fs::remove_file(path).expect("Should be able to delete cache-file");
    }

    Some(())
}

fn cleanup_keepercache(config: &KeeperConfig, test_results: &HashMap<Test, TestResult>) {
    // Go through every file that's like keeper_*.rs
    // If the test passed, keep the file otherwise, delete it.
    let glob_str = format!("{}/keeper_*.rs", config.test_dir.display());
    glob(&glob_str)
        .expect("Could not list keeper files.")
        .filter_map(Result::ok)
        .for_each(|p| {
            clean_file(test_results, &p);
        });
}

#[derive(Default)]
pub struct BookKeeper;

impl BookKeeper {
    pub fn new() -> BookKeeper {
        BookKeeper
    }
}

impl BookKeeper {
    pub fn real_run(
        &self,
        preprocessor_config: PreprocessorConfig,
        root: PathBuf,
        book: &mut Book,
    ) -> Result<HashMap<Test, TestResult>, Error> {
        let config = KeeperConfig::new(preprocessor_config, &root);

        config.setup_environment();

        let tests = get_tests_from_book(book);

        let test_results = run_tests_with_config(tests, &config);

        cleanup_keepercache(&config, &test_results);

        Ok(test_results)
    }
}

impl Preprocessor for BookKeeper {
    fn name(&self) -> &str {
        "keeper"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let preprocessor_config = ctx.config.get_preprocessor(self.name());
        let root = ctx.root.to_path_buf();

        let test_results = self.real_run(preprocessor_config, root, &mut book)?;
        print_results(&test_results);

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer != "not-supported"
    }
}
