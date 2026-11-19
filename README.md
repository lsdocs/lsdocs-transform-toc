# lsdocs-transform-toc

Part of [lsdocs](https://github.com/lsdocs), a static-site generator whose
parsers, transformers and renderers are plugins.

`crates/transform-toc` (`lsdocs-transform-toc`, builtin `transform-toc`):
writes each page's table of contents to its metadata (`toc`), which
themes show, and, with `inline = true`, also inserts it into the page.
The Python and Node.js ports in lsdocs-plugin-sdk must render the same
site byte for byte; keep them in step.

Its settings and behavior are described in the lsdocs documentation
(https://lsdocs.github.io, "transform-toc").

## Known limitations

- When a page's first section is not level 1, the `toc` metadata lists
  it while the inline list (kept as before) treats it as the page title.

## TODO

- [ ] One rule for the page title in both the metadata and the inline
      list.
