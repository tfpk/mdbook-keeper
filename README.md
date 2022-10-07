# mdBook-Keeper

> "Keep all your mdBook code working!"

mdBook-keeper adds more testing features to mdBook. My hope is that it will be merged
into mdBook at some point in the future. 

Current goals include: 

 - Specifying third-party crates to include when testing a mdbook.
 - Running tests as part of the build, rather than as a seperate command.
 - Only running tests on code that has changed.
 - The ability to see compilation or build output as part of the book
   (both to aid in debugging, and possibly also as a permanent output).

## Thanks to Skeptic

Some of the code in this project was taken from the [`rust-skeptic`] project.
Indeed, this project was briefly named `mdbook-skeptic` to acknowledge the code they wrote
which was incredibly useful in writing this crate.

## License

All the code in this repository is released under the APACHE-2.0 or MIT licenses.

[`rust-skeptic`](https://github.com/budziq/rust-skeptic)
