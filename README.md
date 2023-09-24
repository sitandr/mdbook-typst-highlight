# MDBOOK-TYPST-HIGHLIGHT

This is a preprocessor for [mdbook](https://github.com/rust-lang/mdBook) that uses [syntect](https://github.com/trishume/syntect) and [Typst syntax for Sublime Text](https://github.com/hyrious/typst-syntax-highlight/tree/main) to produce highlighted Typst code.

Here is an example of output:

![Example of highlighting](img/image.png)

## Usage

Install using

```bash
cargo install --git https://github.com/sitandr/mdbook-typst-highlight
```

To add preprocessor to `mdbook`, add this to your `book.toml`:

```toml
[preprocessor.typst-highlight]
```

After it, run `mdbook build` or `serve`. That's it. All inline code and blocks with `typ`

## Settings

Currently there are only two setting available: 
- Whether to highlight inline blocks (default is yes):

```
[preprocessor.typst-highlight]
disable_inline = true
```

- Whether to highlight blocks without language specifiedЖ

```
[preprocessor.typst-highlight]
highlight_without_kind = true
```