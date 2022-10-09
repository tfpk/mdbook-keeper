# Chapter 3: Alternatives and Composition


Nom gives us a similar ability through the `alt()` combinator.

```rust,ignore
use nom::branch::alt;
```


  * [ ] We can see a basic example of `alt()` below.

```rust
# extern crate nom;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::IResult;
# use std::error::Error;

fn parse_abc_or_def(input: &str) -> IResult<&str, &str> {
    alt((
        tag("abc"),
        tag("def")
    ))(input)
}

fn main() -> Result<(), Box<dyn Error>> {
    let (leftover_input, output) = parse_abc_or_def("abcWorld")?;
    assert_eq!(leftover_input, "World");
    assert_eq!(output, "abc");

    assert!(parse_abc_or_def("ghiWorld").is_err());
#   Ok(())
}
```

## Composition

Now that we can create more interesting regexes, we can compose them together.
The simplest way to do this is just to evaluate them in sequence:

```rust,ignore
# extern crate nom;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::IResult;
# use std::error::Error;

fn parse_abc(input: &str) -> IResult<&str, &str> {
    tag("abc")(input)
}
fn parse_def_or_ghi(input: &str) -> IResult<&str, &str> {
    alt((
        tag("def"),
        tag("ghi")
    ))(input)
}

fn main() -> Result<(), Box<dyn Error>> {
    let input = "abcghi";
    let (remainder, abc) = parse_abc(input)?;
    let (remainder, def_or_ghi) = parse_def_or_ghi(remainder)?;
    println!("first parsed: {abc}; then parsed: {def_or_ghi};");
    
#   Ok(())
}
```

Composing tags is such a common requirement that, in fact, Nom has a few built in
combinators to do it. The simplest of these is `tuple()`. The `tuple()` combinator takes a tuple of parsers,
and either returns `Ok` with a tuple of all of their successful parses, or it 
returns the `Err` of the first failed parser.

```rust,ignore
use nom::sequence::tuple;
```


```rust
# extern crate nom;
use nom::branch::alt;
use nom::sequence::tuple;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::{digit1};
use nom::IResult;
# use std::error::Error;

fn parse_base(input: &str) -> IResult<&str, &str> {
    alt((
        tag_no_case("a"),
        tag_no_case("t"),
        tag_no_case("c"),
        tag_no_case("g")
    ))(input)
}

fn parse_pair(input: &str) -> IResult<&str, (&str, &str)> {
    // the many_m_n combinator might also be appropriate here.
    tuple((
        parse_base,
        parse_base,
    ))(input)
}

fn main() -> Result<(), Box<dyn Error>> {
    let (remaining, parsed) = parse_pair("aTcG")?;
    assert_eq!(parsed, ("a", "T"));
    assert_eq!(remaining, "cG");
    
    assert!(parse_pair("Dct").is_err());

#   Ok(())
}
```


