## Configuration

elba's behavior can be configured through the use of TOML configuration files and environment variables. elba checks the current directory and all of its ancestors for a `.elba/config` file, unifying them in the following order (from highest to lowest priority):

```sh
# assuming current directory is /foo/bar/baz/quux
/foo/bar/baz/quux/.elba/config
/foo/bar/baz/.elba/config
/foo/bar/.elba/config
/foo/.elba/config
/.elba/config
$HOME/.elba/config
```

Any specified environment variables have the highest priority. This behavior heavily borrows from [Cargo's configuration format](https://doc.rust-lang.org/cargo/reference/config.html).

Additionally, whenever elba executes an Idris invocation, elba will pass all of the arguments in the environment variable `IDRIS_OPTS` to the compiler. In any case where the `IDRIS_OPTS` args conflict with elba's own flags (i.e. if the user specifies the flag `--ide-mode` but elba specifies `--check`), elba will override the user-specified flag.

### Config Format

A complete default elba configuration file is listed below. Any options which are not assigned to will carry the default value instead.

```toml
indices = []

[default_codegen]
name = "c"
portable = "false"

[term]
verbosity = "verbose"
color = "true"

[alias]
i = "install"
b = "build"
t = "test"

[directories]
cache = "$HOME/.elba"
```

> #### Using environment variables
>
> In order to specify an option as an environment variable, simply replace the "dots" of the option with underscores, and prefix with `ELBA_`. So the option `term.verbosity` becomes `ELBA_TERM_VERBOSITY`.

#### `indices`

This key specifies all of the indices that should be made available to packages being built. Any dependent indices of these indices will also be retrieved. The first index specified in this list will be used as the default index for packages which don't specify an index. This key should be a list of index urls; for more information on those, see the chapter on [Indices](../reference/indices.md).

At the moment, the default indices list is empty, but in the future, if we make an official elba package index, the first item in this list will become that official index by default.

#### `[default_codegen]`

This section defines options for the default codegen backend, including its name and whether it's portable or not.

#### `[profile]`

This section specifies the default author information that should be provided upon creating or initializing a new elba project. By default, this section has no value, so new projects are made without an author.

```toml
[profile]
name = "John Smith"
email = "jsmith@example.com"
```

#### `[term]`

This section specifies options for terminal output, and has two fields:

- `verbosity`: specifies how verbose elba should be. Can be one of `verbose`, `normal`, or `quiet`.
- `color`: specifies if elba should try to print color output. Either `true` or `false`.

At the moment, neither of these options actually do anything.

#### `[alias]`

This section is for providing aliases for commands. The key represents the alias and the value represents the the command that it should be aliased to. Note that aliases can alias to other aliases, which can cause *infinite recursion of aliases*. Be careful.

```sh
$ elba b # builds the local package with the default alias settings
```

#### `[directories]`

This section only contains one key: `cache`, for the location where the global cache should be placed. This controls not only the location of elba's temporary build directories but also the location of the global bin directory.

#### `[codegen]`

This section also contains no values by default, and can be used to specify extra information about a codegen backend, like what command to use when running binaries generated with that backend, and any extra options to pass to the backend.

```toml
# an example:
[codegen.node]
# we want to run node binaries with "node __":
runner = "node"
# and we want to pass the option "--potatoes" to the code generator:
opts = ["--potatoes"]
```
