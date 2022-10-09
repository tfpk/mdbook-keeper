mod run_tests;
mod skeptic;

#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::prelude::*;

use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use serde::{Deserialize, Serialize};
use slug::slugify;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::value::Table;
use atty::Stream;

use run_tests::{handle_test, CompileType, TestResult};
use skeptic::{create_test_input, extract_tests_from_string, Test};

type PreprocessorConfig<'a> = Option<&'a Table>;

fn get_tests_from_book(book: &Book) -> Vec<Test> {
    let chapters = book.sections.iter().filter_map(|b| match *b {
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
                .unwrap_or(slugify(c.name.clone()).replace('-', "_"));
            extract_tests_from_string(&c.content, &file_name).0
        })
        .collect::<Vec<_>>()
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct KeeperConfigParser {
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
    terminal_colors: bool
}

impl KeeperConfig {
    fn new(preprocessor_config: PreprocessorConfig, root: &PathBuf) -> KeeperConfig {
        let keeper_config: KeeperConfigParser = match preprocessor_config {
            Some(config) => toml::de::from_str(
                &toml::ser::to_string(&config).expect("this must succeed, it was just toml"),
            )
            .unwrap(),
            None => KeeperConfigParser::default(),
        };

        eprintln!("{preprocessor_config:?}");

        let base_dir = root.clone();
        let test_dir = keeper_config
            .test_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut build_dir = base_dir.clone();
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

        let terminal_colors = keeper_config.terminal_colors.unwrap_or_else(|| atty::is(Stream::Stdout));

        KeeperConfig {
            test_dir,
            target_dir,
            manifest_dir,
            terminal_colors,
        }
    }

    fn setup_environment(&self) {
        if !self.test_dir.is_dir() {
            std::fs::create_dir(&self.test_dir).unwrap();
        }

        if let Some(manifest_dir) = &self.manifest_dir {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
            let mut command = Command::new(cargo);
            command.arg("build")
                   .current_dir(&manifest_dir)
                   .env("CARGO_TARGET_DIR", &self.target_dir)
                   .env("CARGO_MANIFEST_DIR", &manifest_dir);

            println!("{command:#?}");
            let mut join_handle = command
                .spawn()
                .expect("failed to execute process");

            let build_was_ok = join_handle.wait().expect("Could not join on thread");

            if !build_was_ok.success() {
                panic!("cargo build failed!");
            }
        }
    }
}

fn write_test_to_file(test: &Test, test_dir: &Path) -> PathBuf {
    let mut file_name: PathBuf = test_dir.to_path_buf();
    file_name.push(format!("{}.rs", test.name));

    let mut output = File::create(&file_name).unwrap();
    let test_text = create_test_input(&test.text);
    write!(output, "{}", test_text).unwrap();

    file_name
}

fn run_tests_with_config(tests: Vec<Test>, config: &KeeperConfig) -> HashMap<Test, TestResult> {
    let mut results = HashMap::new();
    for test in tests {
        if test.ignore {
            continue;
        }
        let testcase_path = write_test_to_file(&test, &config.test_dir);

        let result: TestResult = handle_test(
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
        );

        results.insert(test, result);
    }

    results
}

fn print_results(results: &HashMap<Test, TestResult>) {
    for (test, output) in results {
        eprint!(" - Test: {} ", test.name);
        let mut show_output = true;
        let output = match output {
            TestResult::CompileFailed(output) => {
                eprintln!("(Failed to compile)");
                output
            }
            TestResult::RunFailed(output) if test.should_panic => {
                eprintln!("(Passed with panic)");
                show_output = false;
                output
            }
            TestResult::RunFailed(output) => {
                eprintln!("(Panicked)");
                output
            }
            TestResult::Successful(output) if test.should_panic => {
                eprintln!("(Failed without panic)");
                output
            }
            TestResult::Successful(output) => {
                eprintln!("(Passed with panic)");
                show_output = false;
                output
            }
        };
        if show_output {
            eprintln!("--------------- Start Of Test Log {} ---------------", test.name);
            if !output.stdout.is_empty() {
                eprintln!(
                    "----- Stdout -----\n{}",
                    String::from_utf8(output.stdout.to_vec()).unwrap()
                );
            } else {
                eprintln!(
                    "No stdout was captured.",
                );
            }
            if !output.stdout.is_empty() {
                eprintln!(
                    "----- Stderr -----\n{}",
                    String::from_utf8(output.stderr.to_vec()).unwrap()
                );
            } else {
                eprintln!(
                    "No stderr was captured.",
                );
            }
            eprintln!("--------------- End Of Test ---------------");
        }
    }
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

        eprintln!("{tests:#?}");

        let test_results = run_tests_with_config(tests, &config);

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
