# Chapter 1

```typ
= Hello there!

This is a test
```

## Render & Preamble

```typ-norender
This will not be rendered.

And that way?
```

```typ-nopreamble
This will be default doc
```

This is some `#inline` code.

```
This is code without any lang specified.
```

````typ
Typst and some Rust inside
```rust
fn main() {
    todo!();
}
```
````

## Hidelines

With the hidelines option configured as `"% "` prefix, the following code:

```none
% #let x = 10;
The hidden $x$ value is #x.
```

will produce:

```typ
% #let x = 10;
The hidden $x$ value is #x.
```

If you configure it as `"^^^"` prefix, then:

````none
```typ,hidelines=^^^
Assume that ```typ #add(x, y)``` is defined.
^^^#let add(x, y) = x + y;
Then ```typ #add(2, 3)``` will be #add(2, 3).
```
````

will result in:

```typ,hidelines=^^^
Assume that ```typ #add(x, y)``` is defined.
^^^#let add(x, y) = x + y;
Then ```typ #add(2, 3)``` will be #add(2, 3).
```
