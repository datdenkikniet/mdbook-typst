# Typst files in nested markdown files

Nested files are also supported. Relative links are resolved from the path of the current markdown
file, and absolute paths are resolved from the book's source.

Some examples:

`<typst://nested.typ>` renders `src/nested/nested.typ`

<typst://nested.typ>

`<typst:///included.typ>` renders `src/included.typ`

<typst:///included.typ>

and `<typst://../included.typ>` also renders `src/included.typ`

<typst://../included.typ>