mod skeptic;
mod run_tests;

use std::fs::File;
use std::io::prelude::*;

use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use serde::{Deserialize, Serialize};
use slug::slugify;
use std::path::{Path, PathBuf};

use skeptic::{extract_tests_from_string, create_test_input, Test};
use run_tests::{TestResult, handle_test, CompileType};

/// A no-op preprocessor.
#[derive(Default)]
pub struct BookKeeper;

impl BookKeeper {
    pub fn new() -> BookKeeper {
        BookKeeper
    }
}

fn get_tests_from_book(book: &Book) -> Vec<Test> {
    let chapters = book.sections.iter().filter_map(|b| match *b {
        BookItem::Chapter(ref ch) => Some(ch),
        _ => None,
    });

    chapters
        .map(|c| {
            let file_name = c
                .path
                .as_ref()
                .and_then(|x| x.file_stem())
                .map(|x| x.to_string_lossy().into_owned())
                .unwrap_or(slugify(c.name.clone()).replace("-", "_"));
            extract_tests_from_string(&c.content, &file_name).0
        })
        .flatten()
        .collect::<Vec<_>>()
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct KeeperConfigParser {
    /// This is where we keep all the intermediate work
    /// of testing. If it's not specified, it's a folder
    /// inside build. If it doesn't exist, we create it.
    #[serde(default)]
    test_dir: Option<PathBuf>,

    /// If you're building this book in the repo for a
    /// real binary/library; this should point to the target
    /// dir for that binary/library.
    #[serde(default)]
    target_dir: Option<PathBuf>,

    /// This is the path of a folder that should contain
    /// a `Cargo.toml`. If there is one there, you should
    /// assume a `Cargo.lock` will be created in the same
    /// place if it doesn't already exist.
    #[serde(default)]
    manifest_dir: Option<PathBuf>,
}

#[derive(Debug)]
struct KeeperConfig {
    test_dir: PathBuf,
    target_dir: PathBuf,
    manifest_dir: Option<PathBuf>,
}

fn create_config_from_ctx(ctx: &PreprocessorContext, preprocessor_name: &str) -> KeeperConfig {
    let preprocessor_config = ctx.config.get_preprocessor(preprocessor_name);
    let keeper_config: KeeperConfigParser = match preprocessor_config {
        Some(config) => toml::de::from_str(
            &toml::ser::to_string(&config)
                .expect("this must succeed, it was just toml"),
        )
        .unwrap(),
        None => KeeperConfigParser::default(),
    };

    eprintln!("{keeper_config:?}");

    let base_dir = ctx.root.clone();
    let test_dir = keeper_config.test_dir.unwrap_or_else(|| {
        let mut build_dir = base_dir.clone();
        build_dir.push("doctest_cache");
        build_dir
    });

    let target_dir = keeper_config.target_dir.unwrap_or_else(|| {
        let mut target_dir = test_dir.clone();
        target_dir.push("target");
        target_dir
    });

    let manifest_dir = keeper_config.manifest_dir;

    KeeperConfig {
        test_dir,
        target_dir,
        manifest_dir,
    }
}

fn setup_env_from_config(config: &KeeperConfig)  {
    if !config.test_dir.is_dir() {
        std::fs::create_dir(&config.test_dir).unwrap();
    }

    // TODO: handle case where `cargo run` still needs to happen.
}

fn write_test_to_file(test: &Test, test_dir: &Path) -> PathBuf {
    let mut file_name: PathBuf = test_dir.to_path_buf();
    file_name.push(format!("{}.rs", test.name));

    let mut output = File::create(&file_name).unwrap();
    let test_text = create_test_input(&test.text);
    write!(output, "{}", test_text).unwrap();

    file_name
}


fn run_tests(tests: &[Test], config: &KeeperConfig) {
    for test in tests {
        if test.no_run {
            continue
        }
        let testcase_path = write_test_to_file(test, &config.test_dir);

        let output: TestResult = handle_test(
            config.manifest_dir.as_deref(),
            &config.target_dir,
            current_platform::CURRENT_PLATFORM,
            &testcase_path,
            if test.no_run {CompileType::Check} else {CompileType::Full}
        );

        let output = match output {
            TestResult::CompileFailed(output) => {
                eprintln!("Test {} Failed To Compile.", test.name);
                output
            },
            TestResult::RunFailed(output) if test.should_panic  => {
                eprintln!("Test {} Panicked As Expected.", test.name);
                output
            },
            TestResult::RunFailed(output)   => {
                eprintln!("Test {} Failed To Run Correctly.", test.name);
                output
            },
            TestResult::Successful(output) if test.should_panic  => {
                eprintln!("Test {} Failed To Panic As Expected.", test.name);
                output
            },
            TestResult::Successful(output)   => {
                eprintln!("Test {} Ran Successfully.", test.name);
                output
            },
        };
        eprintln!("Stdout:\n{}", String::from_utf8(output.stdout).unwrap());
        eprintln!("Stderr:\n{}", String::from_utf8(output.stderr).unwrap());

    }
}

impl Preprocessor for BookKeeper {
    fn name(&self) -> &str {
        "mdbook-keeper-preprocessor"
    }

    fn run(&self, ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        let config = create_config_from_ctx(ctx, self.name());

        setup_env_from_config(&config);

        let tests = get_tests_from_book(&book);

        // Step 1: Run cargo build, if cargo build is required.

        run_tests(&tests, &config);

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer != "not-supported"
    }
}
