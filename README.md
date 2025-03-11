# mdbook-diagrams

This is an [mdbook](https://github.com/rust-lang/mdBook) preprocessor that
allows you to include diagrams-as-code in your book using
[Kroki](https://kroki.io/) to render the diagrams as png or svg files, and works
for html and other renderers.

## Usage

Add the following to your `book.toml`:

```toml
[preprocessor.diagrams]
```

Then you can include diagrams in your markdown files using the following syntax:

````markdown
```mermaid
graph TD;
    A-->B;
    A-->C;
    B-->D;
    C-->D;
```
````

The contents of the code block will be sent to Kroki and the resulting image
will be included in the rendered book:

```mermaid
graph TD;
    A-->B;
    A-->C;
    B-->D;
    C-->D;
```

For the HTML renderer, the image will be
inlined using a data URI. For other renderers, the file will be output to a
temporary file and an image link to that temporary file will replace the code
block. In either case, the results of the graph are cached as temporary files
(and loaded from cache if the code block contents have not changed to avoid
unnecessary requests to Kroki).

## Configuration

You can configure the preprocessor in your `book.toml` like so:

```toml
[preprocessor.diagrams]
output_format = "svg" # can be "svg" or "png"
kroki_url = "https://kroki.io" # change the root URL of the Kroki service
language_prefix = "" # if set, only code blocks with this language prefix will be processed (i.e., set this to "diagram-" then use code blocks with language "diagram-mermaid" to render mermaid diagrams)
kroki_timeout_sec = 5 # timeout in seconds for requests to Kroki
filename_prefix = "diagram-" # prefix for temporary files. Files will be saved to /tmp/<filename_prefix><hash>.<output_format>
```

## Installation

You can install the preprocessor using cargo:

```sh
cargo install mdbook-diagrams
```

## Examples

See the [examples](examples) directory for examples of using this with diagrams
for various diagramming tools and renderers.
