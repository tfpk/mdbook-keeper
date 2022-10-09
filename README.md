# mdBook-Keeper

> "Book-keeping for your testcases."

mdBook-keeper adds more testing features to mdBook, specifically
support for third-party crates, and testing while compiling.
My hope is that it will be merged into mdBook at some point in the future. 

Current goals of this project include: 

 - Specifying third-party crates to include when testing a mdbook. (DONE)
 - Running tests as part of the build, rather than as a seperate command. (DONE)
 - Only running tests on code that has changed. (IN PROGRESS)
 - The ability to see compilation or build output as part of the book
   (both to aid in debugging, and possibly also as a permanent output). (TODO)
   
# Installation

To install this tool, run:
 
``` sh
$ cargo install mdbook-keeper
```

Then add it as a pre-processor. Add this snippet to the bottom
of your `book.toml`

``` toml
[preprocessor.keeper]
command = "mdbook-keeper"
```

Finally, build your book as normal:

```sh
$ mdbook build path/to/book
```
 
## Using Crates From An Existing Project

Many use-cases of `mdbook` involve the documentation being a sub-folder of
a project. To make it easy to support this use-case, you can simply tell
`mdbook-keeper` which directory the `Cargo.toml` (and `Cargo.lock`, if present)
exists in. To do this, add the following line after the `[preprocessor.keeper]`
line in your `book.toml`:

``` toml
# NOTE: the file Cargo.toml should exist at this path.
manifest_dir = "../../path/to/project/"
```

If you have built the existing project already, you may find it useful to get `mdbook-keeper`
to use the same `target` directory as the project. This means that packages don't need
to get re-built in two different locations when building the book and the project.
Add the following line after the `[preprocessor.keeper]` line in your `book.toml`:

``` toml
# NOTE: the `debug` folder should exist at this path.
target_dir = "../../path/to/target/"
```

## Using Extra Crates When In A Stand-Alone Book

If you don't have an existing project, the current solution is to create a minimal
project inside the book repo. The project need only contain `Cargo.toml`, `Cargo.lock`,
and `src/main.rs` (or `src/lib.rs`). Place all required dependencies in the `Cargo.toml`.

Then, point `manifest_dir` to that directory, as shown above. 

## Other Configuration Options

All of these options can be placed in the `book.toml` file, after `[preprocessor.keeper]`.

 - `test_dir` this directory is where all intermediate work is stored, including a `target/`
 folder if one is not specified. If you don't like the default location (`./doctest_cache/`),
 you can change it here.
 - `terminal_colors` sets whether to show ANSI terminal colours in rustc output. It defaults
 to `true` only if you are on a TTY, and `false` otherwise.

## Note on differences to DocTest

`mdbook-keeper` is not a perfect replacement to `doctest`. This is for a few reasons:
 - Because much of the code is based on `rust-skeptic`, we use their rules for parsing
   markdown files. See their project for a detailed list of rules, but in short; we don't
   automatically insert a `main()` function for you, and you must tag codeblocks as `rust`
   to have them run.
 - The output format is different, mainly because replicating `doctest` seemed unnecessary,
   complex, and brittle.
 - This runs on `mdbook build`, rather than as a seperate command.
 

## Thanks to Skeptic

Some of the code in this project was taken from the
[`rust-skeptic`](https://github.com/budziq/rust-skeptic) project.
Indeed, this project was briefly named `mdbook-skeptic` to acknowledge the code they wrote
which was incredibly useful in writing this crate.

## License

All the code in this repository is released under the APACHE-2.0 or MIT licenses.

