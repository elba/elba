## Quick Start

This section intends to be a whirlwind tour of all the functionality available with elba. For more information on each step, refer to either the Usage or Reference chapters.

### Creating a package
Creating a package is easy with elba: all you need is a package name. Note that names in elba are special in that they are *always namespaced*; every name in elba comes with a group part and a name part, separated with a slash. For more information, see the information on names in the [manifest chapter](../usage/manifest.md).

```sh
$ elba new asd # won't work: no namespace
$ elba new grp/asd # ok!
```

This command will generate a new elba project with name `grp/asd` in the folder `./asd/`.

By default, elba will create a project with a binary target, with a main file located at `src/Main.idr`. If you'd like to generate a package with a library target instead, pass the `--lib` flag, which will add a library target to the manifest and generate the file `src/{group}/{name}.idr`. This file structure of having a group followed by a name is just convention, and isn't required.

Regardless of which target is chosen, an `elba.toml` manifest file will also be generated.

#### Initializing a pre-existing package

If you already have an Idris project and want to turn it into an elba project, use the `elba init` command instead; it follows the exact same syntax as `elba new` and is functionally identical, but uses the current directory instead of making a new one.

### Adding dependencies

Now that a new package has been created, you can start to add packages as part of your dependencies. A package can originate from one of three places: a git repository, a file directory, or a package index. Ordinary dependencies are placed under the `[dependencies]` section, while dependencies that are only needed for tests and the like are placed under `[dev_dependencies]`. Examples are shown below:

```toml
[dependencies]
"index/version" = "0.1.5" # uses the default index (i.e. the first specified one in configuration)
"index/explicit" = { version = "0.1.5", index = "index+dir+../index" } # uses the index specified
"directory/only" = { path = "../awesome" } # uses the package in the path specified
"git/master" = { git = "https://github.com/doesnt/exist" } # uses the master branch
"git/explicit" = { git = "https://github.com/doesnt/exist", branch = "beta" } # "branch" can be an arbitrary git ref: a tag, commit, etc.
```

For more information on the syntax regarding specifying and adding custom indices, see the chapters on [Resolutions](../reference/resolutions.md) and [Configuration](../usage/configuration.md). More information about dependency specification syntax is available at [its relevant chapter](../reference/specifying_dependencies.md).

Note that only packages with library targets can be depended on.

At this point, you can add whatever files you want and import anything from your dependencies.

### Targets

The manifest also allows you to specify which targets you want to have built for your package. There are three types of targets:

- A **library target** allows this package to be depended on by other packages. A package can only have one library, and the syntax follows the following:

  ```toml
  [targets.lib]
  # the path which contains all of the lib files (*cannot* be a parent directory)
  path = "src/"
  # a list of files to export
  mods = [
      "Awesome.A", # the file src/Awesome/A.idr
      "Control.Zygohistomorphic.Prepromorphisms", # the file src/Control/Zygohistomorphic/Prepromorphisms.idr
  ]
  ```

- A **bin target** specifies a binary to be built. Multiple binaries can correspond to one package.

  ```toml
  [[targets.bin]]
  # the name of the binary to create
  name = "awes"
  # the path to the Main module of the binary
  main = "src/Awesome/B.idr"
  ```

- A **test target** specifies a test binary to build. It uses the same syntax as a bin target, with the difference that we use `[[targets.test]]` to specify them and the test binary can depend on the dev-dependencies as well as the root package's library (at the moment, tests require a library target to be present).

### Building a package

...can be accomplished with the command:

```sh
# assuming the current directory is an elba package
$ elba build
```

For all elba build-related commands, the `IDRIS_OPTS` environment variable will dictate additional arguments to pass to the Idris compiler (the flags passed by elba get higher priority). This can be helpful for packages which depend on base installed Idris packages (e.g. if you want to pass `-p effects` to the compiler).

When building a local package, the output binaries are located at `target/bin`, while the output library is placed at `target/lib`.

Interactive development with the REPL can also be accomplished with the command:

```sh
# assuming the current directory is an elba package
$ elba repl
```

Instead of placing the build outputs in a `target/` folder, the `elba repl` command directly loads the files on-disk, then cleans up any build files after execution.

elba uses an `elba.lock` lockfile to ensure that these builds are reproducible.