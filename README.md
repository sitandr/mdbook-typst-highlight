# mdbook-typst-highlight

This is a preprocessor for [mdbook](https://github.com/rust-lang/mdBook) that uses [syntect](https://github.com/trishume/syntect) and [Typst syntax for Sublime Text](https://github.com/hyrious/typst-syntax-highlight/tree/main) to produce&_render_ highlighted Typst code.

Here is an example of output:

![Example of highlighting](img/image.png)

## Usage

Install using `cargo`:

```bash
cargo install --git https://github.com/sitandr/mdbook-typst-highlight --locked
```

To add preprocessor to `mdbook`, add this to your `book.toml`:

```toml
[preprocessor.typst-highlight]
```

After it, run `mdbook build` or `serve`. That's it. All inline code and blocks with `typ` will be highlighted.

## Settings

You can customize the preprocessor using the following settings:

- Whether to highlight inline blocks (default is to highlight them):

    ```toml
    [preprocessor.typst-highlight]
    disable_inline = true
    ```

- Whether to highlight and render blocks without language specified (default is `false`):

    ```toml
    [preprocessor.typst-highlight]
    typst_default = true
    ```

- What symbols to use for hiding code lines (default is none, read more in [the mdBook documentation](https://rust-lang.github.io/mdBook/format/mdbook.html?highlight=hide#hiding-code-lines)):

    ```toml
    [output.html.code.hidelines]
    typst = "% "
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

It comes with prelude that sets `width: 300pt`, `margin: 0.5cm` and `height: auto`. To disable it, add `typ-nopreamble` as codeblock language.

You can also disable certain blocks (but still highlight them) using `typ-norender`.

## Caching

To prevent recompiling large amount of files, all images are cached. To clear images, add `**/typst-src` and `**/typst-img` to `.gitignore` and then run `git clean -d -X -i` (I strictly advise using interactive mode just to make sure you don't delete something necessary).
