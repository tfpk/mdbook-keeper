/// File entirely copied from:
/// https://raw.githubusercontent.com/budziq/rust-skeptic/master/skeptic/src/rt.rs
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

use cargo_metadata::Edition;
use error_chain::error_chain;
use walkdir::WalkDir;

use crate::skeptic::Test;

#[derive(Debug)]
pub enum TestResult {
    Successful(Output),
    CompileFailed(Output),
    RunFailed(Output),
    Cached,
}

impl TestResult {
    /// A test-result meets expectations if the result is
    /// what is "expected" from that test. This is either
    /// a successful test, or a crash if the test is supposed
    /// to panic. All other tests have not met expectations.
    ///
    /// Cached tests are assumed to have passed, since they don't
    /// stay cached unless they pass.
    pub fn met_test_expectations(&self, test: &Test) -> bool {
        match self {
            TestResult::CompileFailed(_) if test.compile_fail => true,
            TestResult::Successful(_) if !test.should_panic && !test.compile_fail => true,
            TestResult::RunFailed(_) if test.should_panic => true,
            TestResult::Cached => true,
            _ => false,
        }
    }
}

/// This function is designed to run a single test.
///
///  - `manifest_dir` should be a path to a `Cargo.toml`.
///  - `target_dir` should be the path the root of a "target" directory.
///  - `target_triple` should be the type of the target to compile.
///  - `testcase_path` should be the path to a rust file, which contains the test code.
///  - `compile_type` should be [`CompileType::Full`] if the compilation should include
///    running the code; otherwise just [`CompileType::Check`]
pub fn handle_test(
    manifest_dir: Option<&Path>,
    target_dir: &Path,
    target_triple: &str,
    testcase_path: &Path,
    compile_type: CompileType,
    terminal_colors: bool,
    externs: &Vec<String>,
) -> TestResult {
    // First, let's get the command ready, no matter
    // whether or not a Cargo.toml is specified.

    let rustc = env::var("RUSTC").unwrap_or_else(|_| String::from("rustc"));
    let mut cmd = Command::new(rustc);
    cmd.arg(testcase_path)
        .arg("--verbose")
        .arg(if terminal_colors {
            "--color=always"
        } else {
            "--color=never"
        });

    match compile_type {
        CompileType::Full => cmd.arg("--crate-type=bin"),
        CompileType::Check => cmd.arg("--crate-type=lib"),
    };

    if let Some(manifest_dir) = manifest_dir {
        // OK, here's where a bunch of magic happens using assumptions
        // about cargo internals. We are going to use rustc to compile
        // the examples, but to do that we've got to tell it where to
        // look for the rlibs with the -L flag, and what their names
        // are with the --extern flag. This is going to involve
        // parsing fingerprints out of the lockfile and looking them
        // up in the fingerprint file.

        let mut cargo_toml_path = PathBuf::from(manifest_dir);
        cargo_toml_path.push("Cargo.toml");

        let mut deps_dir = PathBuf::from(target_dir);
        deps_dir.push("debug/deps");

        // Find the edition

        // This has to come before "-L".
        let metadata = get_cargo_meta(&cargo_toml_path).expect("failed to read Cargo.toml");
        let edition = metadata
            .packages
            .iter()
            .filter_map(|package| edition_str(&package.edition))
            .max()
            .unwrap();
        if edition != "2015" {
            cmd.arg(format!("--edition={}", edition));
        }

        cmd.arg("-L")
            .arg(target_dir)
            .arg("-L")
            .arg(&deps_dir)
            .arg("--target")
            .arg(target_triple);

        for dep in externs {
            cmd.arg("--extern");
            cmd.arg(dep);
        }

        for dep in get_rlib_dependencies(manifest_dir.to_path_buf(), target_dir.to_path_buf())
            .expect("failed to read dependencies")
        {
            cmd.arg("--extern");
            cmd.arg(format!(
                "{}={}",
                dep.libname,
                dep.rlib.to_str().expect("filename not utf8"),
            ));
        }
    }

    let mut binary_path = PathBuf::from(testcase_path);
    binary_path.set_extension("exe");

    match compile_type {
        CompileType::Full => cmd.arg("-o").arg(&binary_path),
        CompileType::Check => cmd.arg(format!(
            "--emit=dep-info={0}.d,metadata={0}.m",
            binary_path.display()
        )),
    };

    let command_result = cmd.output().unwrap();
    return if !command_result.status.success() {
        TestResult::CompileFailed(command_result)
    } else if CompileType::Check == compile_type {
        TestResult::Successful(command_result)
    } else {
        let cmd_current_dir = testcase_path
            .parent()
            .expect("File must live in a directory.");

        let mut cmd = Command::new(binary_path);
        cmd.current_dir(cmd_current_dir);
        let command_output = cmd.output().unwrap();

        if command_output.status.success() {
            TestResult::Successful(command_result)
        } else {
            TestResult::RunFailed(command_result)
        }
    };
}

// Retrieve the exact dependencies for a given build by
// cross-referencing the lockfile with the fingerprint file
fn get_rlib_dependencies(manifest_dir: PathBuf, target_dir: PathBuf) -> Result<Vec<Fingerprint>> {
    let lock = LockedDeps::from_path(manifest_dir)?;

    let fingerprint_dir = target_dir.join(".fingerprint/");
    let locked_deps: HashMap<String, String> = lock.collect();
    let mut found_deps: HashMap<String, Fingerprint> = HashMap::new();

    for finger in WalkDir::new(fingerprint_dir)
        .into_iter()
        .filter_map(|v| Fingerprint::from_path(v.ok()?.path()).ok())
    {
        let locked_ver = match locked_deps.get(&finger.name()) {
            Some(ver) => ver,
            None => continue,
        };

        // TODO this should be refactored to something more readable
        match (found_deps.entry(finger.name()), finger.version()) {
            (Entry::Occupied(mut e), Some(ver)) => {
                // we find better match only if it is exact version match
                // and has fresher build time
                if *locked_ver == ver && e.get().mtime < finger.mtime {
                    e.insert(finger);
                }
            }
            (Entry::Vacant(e), ver) => {
                // we see an exact match or unversioned version
                if ver.unwrap_or_else(|| locked_ver.clone()) == *locked_ver {
                    e.insert(finger);
                }
            }
            _ => (),
        }
    }

    Ok(found_deps
        .into_iter()
        .filter_map(|(_, val)| if val.rlib.exists() { Some(val) } else { None })
        .collect())
}

// An iterator over the root dependencies in a lockfile
#[derive(Debug)]
struct LockedDeps {
    dependencies: Vec<String>,
}

fn get_cargo_meta<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
    path: P,
) -> Result<cargo_metadata::Metadata> {
    Ok(cargo_metadata::MetadataCommand::new()
        .manifest_path(&path)
        .exec()?)
}

impl LockedDeps {
    fn from_path<P: AsRef<Path>>(path: P) -> Result<LockedDeps> {
        let path = path.as_ref().join("Cargo.toml");
        let metadata = get_cargo_meta(path)?;
        let workspace_members = metadata.workspace_members;
        let deps = metadata
            .resolve
            .ok_or("Missing dependency metadata")?
            .nodes
            .into_iter()
            .filter(|node| workspace_members.contains(&node.id))
            .flat_map(|node| node.dependencies.into_iter())
            .chain(workspace_members.clone());

        Ok(LockedDeps {
            dependencies: deps.map(|node| node.repr).collect(),
        })
    }
}

impl Iterator for LockedDeps {
    type Item = (String, String);

    fn next(&mut self) -> Option<(String, String)> {
        let dep = self.dependencies.pop()?;
        let mut parts = dep.split_whitespace();
        let name = parts.next()?;
        let val = parts.next()?;
        Some((name.replace('-', "_"), val.to_owned()))
    }
}

#[derive(Debug)]
struct Fingerprint {
    libname: String,
    version: Option<String>, // version might not be present on path or vcs deps
    rlib: PathBuf,
    mtime: SystemTime,
}

fn guess_ext(mut path: PathBuf, exts: &[&str]) -> Result<PathBuf> {
    for ext in exts {
        path.set_extension(ext);
        if path.exists() {
            return Ok(path);
        }
    }
    Err(ErrorKind::Fingerprint.into())
}

impl Fingerprint {
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Fingerprint> {
        let path = path.as_ref();

        // Use the parent path to get libname and hash, replacing - with _
        let mut captures = path
            .parent()
            .and_then(Path::file_stem)
            .and_then(OsStr::to_str)
            .ok_or(ErrorKind::Fingerprint)?
            .rsplit('-');
        let hash = captures.next().ok_or(ErrorKind::Fingerprint)?;
        let mut libname_parts = captures.collect::<Vec<_>>();
        libname_parts.reverse();
        let libname = libname_parts.join("_");

        path.extension()
            .and_then(|e| if e == "json" { Some(e) } else { None })
            .ok_or(ErrorKind::Fingerprint)?;

        let mut rlib = PathBuf::from(path);
        rlib.pop();
        rlib.pop();
        rlib.pop();
        let mut dll = rlib.clone();
        rlib.push(format!("deps/lib{}-{}", libname, hash));
        dll.push(format!("deps/{}-{}", libname, hash));
        rlib = guess_ext(rlib, &["rlib", "so", "dylib"]).or_else(|_| guess_ext(dll, &["dll"]))?;

        Ok(Fingerprint {
            libname,
            version: None,
            rlib,
            mtime: fs::metadata(path)?.modified()?,
        })
    }

    fn name(&self) -> String {
        self.libname.clone()
    }

    fn version(&self) -> Option<String> {
        self.version.clone()
    }
}

error_chain! {
    errors { Fingerprint }
    foreign_links {
        Io(std::io::Error);
        Metadata(cargo_metadata::Error);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CompileType {
    Full,
    Check,
}

fn edition_str(edition: &Edition) -> Option<&'static str> {
    Some(match edition {
        Edition::E2015 => "2015",
        Edition::E2018 => "2018",
        Edition::E2021 => "2021",
        _ => return None,
    })
}
