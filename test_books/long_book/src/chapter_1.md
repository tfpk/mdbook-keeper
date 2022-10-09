# Chapter 1: The Nom Way

First of all, we need to understand the way that nom thinks about parsing.
As discussed in the introduction, nom lets us build simple parsers, and
then combine them (using "combinators").

A simple diagram.

```text
                                   ┌─► Ok(
                                   │      what the parser didn't touch,
                                   │      what matched the regex
                                   │   )
             ┌─────────┐           │
 my input───►│my parser├──►either──┤
             └─────────┘           └─► Err(...)
```


Code Block

```rust,ignore
use nom::IResult;
```

Larger Code Block

```rust
# extern crate nom;
# use nom::IResult;
# use std::error::Error;

pub fn do_nothing_parser(input: &str) -> IResult<&str, &str> {
    Ok((input, ""))
}

fn main() -> Result<(), Box<dyn Error>> {
    let (remaining_input, output) = do_nothing_parser("my_input")?;
    assert_eq!(remaining_input, "my_input");
    assert_eq!(output, "");
#   Ok(())
}
```

It's that easy!
