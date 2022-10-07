use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::errors::Error;
use mdbook::book::Book;

/// A no-op preprocessor.
#[derive(Default)]
pub struct BookKeeper;

impl BookKeeper {
    pub fn new() -> BookKeeper {
        BookKeeper
    }
}

impl Preprocessor for BookKeeper {
    fn name(&self) -> &str {
        "mdbook-keeper-preprocessor"
    }

    fn run(&self, ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        // In testing we want to tell the preprocessor to blow up by setting a
        // particular config value
        if let Some(nop_cfg) = ctx.config.get_preprocessor(self.name()) {
            if nop_cfg.contains_key("blow-up") {
                anyhow::bail!("Boom!!1!");
            }
        }

        eprintln!("{:#?}", book);

        // we *are* a no-op preprocessor after all
        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer != "not-supported"
    }
}
