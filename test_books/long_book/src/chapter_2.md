# Chapter 2: Tags and Character Classes


 **Warning**: `nom` has multiple different definitions of `tag`, make sure you use this one for the
 moment!

```rust,ignore
pub use nom::bytes::complete::tag;
```

For example, code to parse the string `"abc"` could be represented as `tag("abc")`.


```rust,ignore
pub fn tag<T, Input, Error: ParseError<Input>>(
    tag: T
) -> impl Fn(Input) -> IResult<Input, Input, Error> where
    Input: InputTake + Compare<T>,
    T: InputLength + Clone, 
```

Or, for the case where `Input` and `T` are both `&str`, and simplifying slightly:

```rust,ignore
fn tag(tag: &str) -> (impl Fn(&str) -> IResult<&str, Error>)
```

Below, we have implemented a function that uses `tag`.

```rust
# extern crate nom;
# pub use nom::bytes::complete::tag;
# pub use nom::IResult;
# use std::error::Error;

fn parse_input(input: &str) -> IResult<&str, &str> {
    //  note that this is really creating a function, the parser for abc
    //  vvvvv 
    //         which is then called here, returning an IResult<&str, &str>
    //         vvvvv
    tag("abc")(input)
}

fn main() -> Result<(), Box<dyn Error>> {
    let (leftover_input, output) = parse_input("abcWorld")?;
    assert_eq!(leftover_input, "World");
    assert_eq!(output, "abc");

    assert!(parse_input("defWorld").is_err());
#   Ok(())
}
```

If you'd like to, you can also check tags without case-sensitivity
with the [`tag_no_case`](https://docs.rs/nom/latest/nom/bytes/complete/fn.tag_no_case.html) function.


We can use them:

```rust
# extern crate nom;
# pub use nom::IResult;
# use std::error::Error;
pub use nom::character::complete::alpha0;
fn parser(input: &str) -> IResult<&str, &str> {
    alpha0(input)
}

fn main() -> Result<(), Box<dyn Error>> {
    let (remaining, letters) = parser("abc123")?;
    assert_eq!(remaining, "123");
    assert_eq!(letters, "abc");
    
#   Ok(())
}
```

One important note is that, due to the type signature of these functions,
it is generally best to use them within a function that returns an `IResult`.

