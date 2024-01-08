# cco

_**TL;DR:** write [HCL](https://github.com/hashicorp/hcl/blob/main/hclsyntax/spec.md). run `cco` to render configuration
files (`json`/`yaml`/...)_

**Example**

```hcl
# cco.hcl
data service app {
  port = 13371
}
data service db {
  port = 13372
}
```

```shell
# query the document...
> cco eval 'service.app.port' --input-file cco.hcl --output-format yaml

13371

# ...or evaluate arbitrary hcl expressions
> cco eval '{for k,s in service : k => s.port}' --input-file cco.hcl --output-format yaml

app: 13371
db: 13372
```

**Why?**

At `$WORK` we use multiple tools that all want some configuration values.
The are currently spread across multiple files in a git repository.
There are some values that are duplicated and some values that are just composites or subsets of others.

At times it is difficult to keep all those values in sync.

**What?**

It would be nice to have a _single source of truth_ that allows sharing/deriving final values.

We would express our values and then derive/select subsets that we need.

**How?**

`HCL` is a well known format in the "DevOps/Infrastructure/Ops/Administration" space and lends itself well to 'chaining'
values.
The idea is to write the "truth" in `HCL` and then somehow output configuration values in a format suitable for each
tool used (json/yaml/...).

For implementation details see [docs.rs/cco](https://docs.rs/cco).

**And then?**

This would, _in theory_, allow many things, such as:

- trace where values
    - are coming from
    - are used (or no longer needed)
    - are changed (and what parts we have to deploy to apply those changes)
- typing & validation allow for
    - -> shift-left of error messages
    - -> safe refactoring (you know if the "final" output changes)
- code deduplication (`cco` can parameterize code, like "per-environment")

## Installation

_No binaries yet. Please compile from sources._

## File format

While `cco` uses
the [HashiCorp Configuration Syntax](https://developer.hashicorp.com/terraform/language/syntax/configuration) the
allowed attributes and blocks are fixed. \
The following guide assumes that you already know hcl.

**Attributes in the root of the document are ignored**

```hcl
# ignored
foo = "bar"
```

**Define data with the data block**

```hcl
# at least one label is required
data example {
  # specify attributes in the body
  attribute = 42
}
```

**Consider attributes names starting with `cco__` reserved.**

```hcl
data example {
  # Not allowed, cco__* is used internally
  cco__something_something = 1
}
```

**Duplicate data blocks or attribute names are __not allowed__**.

```hcl
data example {
  key = "value1"
  # The second attribute is not allowed as it has the same name
  key = "value2"
}

# This block is not allowed, as it has the same labels as the previous one
data example {
}
```

**An attribute is not allowed to refer to itself, even if resolution would not lead to a loop**.

```hcl
# This is an error, even if technically feasible
data example {
  a = [
    "foo",
    example.a[0]
  ]
}
```

**The number of labels for any given data block type must match across all documents**.

```hcl
# Error: One example block has 1 label and another example block has 2
data example {
  i_have_one_label = 1
}
data example foo {
  i_have_two = 2
}
```

**Use labels which are valid identifiers because labels are sanitized**

The current rules for label sanitation (subject to change):

- An empty ident results in an identifier containing a single underscore.
- Invalid characters in ident will be replaced with underscores.
- If ident starts with a character that is invalid in the first position but would be valid in the rest of an HCL
  identifier it is prefixed with an underscore.

- It is recommended not to depend on this behavior.
- It is recommended not to quote labels to let tools complain if labels are not valid identifiers

```hcl
# will be example.quoted_label
data example "quoted label" {
}
```

**`self` can be used to refer to the current root block**

```hcl
data example {
  foo      = "bar"
  uses_foo = self.foo # resolves to example.foo ("bar")
}
```

**`self[N]` can be used to refer to the current root block's identifier/label**

Please note that this is not a real list. Hcl features operating on a list do not work as expected.

```hcl
data example foo bar {
  example = self[0] # "example"
  foo     = self[1] # "foo"
  bar     = self[2] # "bar"
}
```

**Additionally:**

- the load order of multiple files will never affect the value output
- the order of root blocks will never affect the value output
- currently, not all rules are enforced
- currently, there are no functions

## Command line interface

**Input**

Configuration file names for `cco` should end with `cco.hcl`. \
If there is a single file it should be named `cco.hcl`. When using multiple files in one directory the file extension
should be `.cco.hcl`. \
`cco` will load any file provided via the `-f/--input-file` option but only load files with names ending in `cco.hcl`
when loading from the working directory (`-w/--input-workdir`) or directories provided via `-d/--input-dir`.
There is an additional mode "chain" that starts at the current work directory and then walks up the tree as long as it finds files to load `-c/--input-chain`.

When no options for files or directories are provided `cco` will read `stdin` as a single file.

**Output**

- `stdout`: requested information (configuration values; help text when explicitly asked)
- `stderr`: log messages

**Exit Codes**

- `== 0`: success
- `!= 0`: failure
    - `1`: general error
    - `2`: invalid invocation / help displayed
    - `>=3`: reserved/unused

**Environment Variables**

- `CCO_LOG`: configure logging. see
  tracing_subscriber's [env_filter directive](https://docs.rs/tracing-subscriber/0.3.18/tracing_subscriber/filter/struct.EnvFilter.html#directives)
  for value format.
