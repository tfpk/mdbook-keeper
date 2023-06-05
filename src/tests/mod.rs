use anyhow::Error;
use fs_extra::copy_items;
use mdbook::book::Book;
use mdbook::config::BuildConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};
use toml::value::{Table, Value};

use crate::run_tests::TestResult;
use crate::BookKeeper;

fn make_tmpdir_like(path: &Path) -> TempDir {
    // Create a directory inside of `std::env::temp_dir()`.
    let dir = tempdir().unwrap();
    copy_items(&[path], dir.path(), &fs_extra::dir::CopyOptions::new()).unwrap();

    dir
}

fn get_starting_directories(book_name: &str) -> Result<(TempDir, Book), Error> {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("test_books");
    d.push(book_name);

    let root_tempdir_obj = make_tmpdir_like(&d);
    let root_tempdir = root_tempdir_obj.path().to_path_buf();

    let mut src_tempdir = root_tempdir.clone();
    src_tempdir.push(book_name);
    src_tempdir.push("src");

    let build_config = BuildConfig {
        // since we never build, this can be anything.
        extra_watch_dirs: vec![],
        build_dir: root_tempdir,
        create_missing: true,
        use_default_preprocessors: false,
    };
    let book = mdbook::book::load_book(src_tempdir, &build_config)?;

    Ok((root_tempdir_obj, book))
}

#[test]
fn empty_book() -> Result<(), Error> {
    let (tmp_dir, mut book) = get_starting_directories("empty_book")?;
    let root_tempdir = tmp_dir.path();

    let bookkeeper = BookKeeper::new();

    let table = Table::new();
    let result = bookkeeper.real_run(Some(&table), root_tempdir.to_path_buf(), &mut book);

    assert_eq!(result.unwrap().len(), 0);

    Ok(())
}

#[test]
fn short_book() -> Result<(), Error> {
    let (tmp_dir, mut book) = get_starting_directories("short_book")?;
    let root_tempdir = tmp_dir.path();

    let bookkeeper = BookKeeper::new();

    let table = Table::new();
    let result = bookkeeper.real_run(Some(&table), root_tempdir.to_path_buf(), &mut book)?;

    let test_list = result
        .into_iter()
        .map(|(t, res)| (t.text[0].trim().to_string(), (t, res)))
        .collect::<HashMap<_, _>>();

    assert_eq!(test_list.len(), 5);

    assert!(test_list.contains_key("// compile-error"));
    assert!(matches!(
        test_list["// compile-error"].1,
        TestResult::CompileFailed(_)
    ));

    assert!(test_list.contains_key("// no-run"));
    assert!(matches!(
        test_list["// no-run"].1,
        TestResult::CompileFailed(_)
    ));

    assert!(test_list.contains_key("// ok"));
    assert!(matches!(test_list["// ok"].1, TestResult::Successful(_)));

    assert!(test_list.contains_key("// panic"));
    assert!(matches!(test_list["// panic"].1, TestResult::RunFailed(_)));

    assert!(test_list.contains_key("// panic-ok"));
    assert!(matches!(
        test_list["// panic-ok"].1,
        TestResult::RunFailed(_)
    ));

    Ok(())
}

#[test]
fn long_book() -> Result<(), Error> {
    let (tmp_dir, mut book) = get_starting_directories("long_book")?;
    let root_tempdir = tmp_dir.path();

    let mut cargo_dir = root_tempdir.to_path_buf();
    cargo_dir.push("long_book");
    cargo_dir.push("cargo");

    let bookkeeper = BookKeeper::new();

    let mut table = Table::new();
    table.insert(
        String::from("manifest_dir"),
        Value::String(cargo_dir.display().to_string()),
    );
    table.insert(
        String::from("externs"),
        Value::Array(vec![Value::String("nom".to_string())]),
    );
    let result = bookkeeper.real_run(Some(&table), root_tempdir.to_path_buf(), &mut book)?;
    crate::print_results(&result);

    assert_eq!(result.len(), 5);

    let mut passed = 0;

    for (_test, result) in result {
        if let TestResult::Successful(_) = result {
            passed += 1;
        }
    }

    assert_eq!(passed, 5);

    Ok(())
}
