# The `mdbook-typst` preprocessor

This preprocessor allows you to include typst in your mdbook.

Currently, two forms are supported: code blocks and autolinks.

Packages are supported, but slightly differently than how the `typst` CLI does. See [Package support](#package-support) for more
information.

## Code blocks

The contents of code blocks whose information line is equal to `typst` are interpreted as typst code and transformed
into an SVG, and the first page of the output is placed at the location of the code block.

For instance, the following code block:

````
```typst
#set page(width: auto, height: auto)

My typst document
```
````

renders like this:

```typst
#set page(width: auto, height: auto)
#import "@preview/lilaq:0.6.0"

My typst document
```

## File inclusion

Typst files can also be included directly by creating autolinks with the `typst://` scheme.

For instance, including the file `included.typ` by writing `<typst://included.typ>` displays as follows:

<typst://included.typ>

The contents of the included typst file can be seen in [this sub-section](./included.md)

## Package support { #package-support }

Packages, as included through the `#import "@preview/name:version"` directive in your typst code, are supported. However,
the preprocessor will not download packages for you automatically. Instead, you'll have to download the `tar` files, decompress
them, and place them in the `typst-pkgs` subdirectory of your book root directory manually.

If you have and `wget` and `gunzip` installed, the preprocessor can do this for you: simply call `mdbook-typst download <package-version> <target-dir>`.

If you do not have these tools available (or would prefer not to have the `mdbook-typst` preprocessor execute the command for you), an example of
doing all of this manually using the `lilaq:0.6.0` package in an mdBook in the `book/` directory can be seen below:

```bash
# Download the .tar.gz file
wget https://packages.typst.org/preview/lilaq-0.6.0.tar.gz

# Decompress it
gunzip lilaq-0.6.0.tar.gz

# Create the `typst-pkgs` directory if it didn't exist yet
mkdir book/typst-pkgs/

# Move the now-decompressed tarball to the correct directory
mv lilaq-0.6.0.tar book/typst-pkgs/
```

## Using markdown image links instead of inline `svg` data

By default, the preprocessor replaces your typst with inline SVG data in the output markdown. However, it also
supports using image links instead of inline-data if you wish.

To enable this behaviour, you must set the `preprocessor.typst.svg-dir` value in your `book.toml` to the path
to a subdirectory in your book source. Additionally, you must add that directory to the `.gitignore` in your
book's root to prevent `mdbook serve` from rebuilding the project infinitely.

For example

```
# book.toml
[preprocessor.typst]
svg-dir = "_typst_generated"
```

```
# .gitignore
book
src/_typst_generated/
```