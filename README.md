# mdbook-typst-highlight

This is a preprocessor for [mdbook](https://github.com/rust-lang/mdBook) that uses [syntect](https://github.com/trishume/syntect) and [Typst syntax for Sublime Text](https://github.com/hyrious/typst-syntax-highlight/tree/main) to produce&_render_ highlighted Typst code.

Here is an example of output:

![Example of highlighting](img/image.png)

## Usage

Install using `cargo`:

```bash
cargo install --git https://github.com/sitandr/mdbook-typst-highlight
```

To add preprocessor to `mdbook`, add this to your `book.toml`:

```toml
[preprocessor.typst-highlight]
```

After it, run `mdbook build` or `serve`. That's it. All inline code and blocks with `typ` will be highlighted.

## Settings

Currently there are only two settings available: 
- Whether to highlight inline blocks (default is yes):

```toml
[preprocessor.typst-highlight]
disable_inline = true
```

- Whether to highlight and render blocks without language specified:

```toml
[preprocessor.typst-highlight]
typst_default = true
```

# Rendering

To enable rendering, just add

```toml
[preprocessor.typst-highlight]
render = true
```

_Important:_ the binary doesn't include Typst and itself. For rendering to work, you have to get _installed Typst in `PATH`_.

Rendered looks like this:

![Example](img/image_2.png)

It comes with prelude that sets `width: 300pt`, `margin: 0.5cm` and `height: auto`. To disable it, add `typ-noprelude` as codeblock language.

You can also disable certain blocks (but still highlight them) using `typ-norender`.