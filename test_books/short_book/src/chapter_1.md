# Chapter 1

This fragment will break because there's a compilation error.

```rust
// compile-error
fn main() {
    println!("Example");
    asdf
}
```

This fragment would break, but won't! Thanks `no_run`

```rust,no_run
// no-run
fn main() {
    println!("Another example!");
    asdf
}
```

This fragment will compile correctly.

```rust
// ok
fn main() {
    println!("Yeet.");
}
```

This will compile but panic.

```rust
// panic
fn main() {
    println!("kalm!");
    panic!(":(")
}
```

  * [ ] This will compile but panic, which is OK!
```rust,should_panic
// panic-ok
fn main() {
    println!("kalm!");
    panic!(":(")
}
```

This should just not get picked up

```rust,ignore
// ignore-me
fn main() {
    println!("kalm!");
    panic!(":(")
}
```
