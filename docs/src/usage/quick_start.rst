Quick Start
===========

This section is for getting up-to-speed with elba as fast as possible,
covering getting elba installed on your machine in the first place and
making a new project.

By the end of this chapter, you should have a basic elba installation
up-and-running, as well as a general overview of how to use elba for
day-to-day Idris development.

We will assume that you already have the Idris toolchain installed. If
you don't, there are instructions available on the
`Idris <https://www.idris-lang.org/download/>`__ and
`Blodwen <https://github.com/edwinb/Blodwen>`__ websites.

Installation
------------

The easiest and most convenient way of installing elba is to use the
pre-built binaries for elba, which can be downloaded from `GitHub
Releases <https://github.com/elba/elba/releases>`__. To install this
way, just download the corresponding archive for your platform, extract
the executable somewhere in your PATH, add ``~/.elba/bin`` to your PATH
in order to execute elba-installed packages, and you’re done!

.. note::

   For Linux platforms, there are two varieties of binaries available:
   one suffixed with ``-gnu`` and the other suffixed with ``-musl``. The
   ``-gnu`` binary is dynamically linked to the system libc, while the
   ``-musl`` binary is statically linked using musl.

   For most users, the ``-gnu`` binary should work fine, but if it
   doesn’t, try using the ``-musl`` binary.

Installing with Cargo
~~~~~~~~~~~~~~~~~~~~~

Because elba is written in Rust, it is available as an installable crate
from `crates.io <https://crates.io>`__. In order to install elba this
way, you should have a copy of the Rust toolchain installed on your
computer first. The process for this is explained on `the Rust
website <https://www.rust-lang.org/en-US/install.html>`__. The version of
of Rust elba has successfully been built on is **nightly-2020-02-21**.

Once you have Rust installed, installing elba is self-explanatory:

.. code-block:: console

   $ cargo install elba
   $ elba # should work

Remember to add ``~/.elba/bin`` to your PATH to be able to run
elba-installed packages.

Building elba
~~~~~~~~~~~~~

Building elba from source is much the same process as installing it
using cargo; the only difference is that instead of using a stable,
versioned-crate available from crates.io, elba’s source code is used
directly. You’ll still need to have Rust **1.31** or later installed.
After that’s done, download elba’s source code and install it:

.. code-block:: console

   $ git clone https://github.com/elba/elba
   $ cd elba
   $ cargo install --path .
   $ elba # should work!

Remember to add ``~/.elba/bin`` to your PATH to be able to run
elba-installed packages.

Creating a package
------------------

Creating a package is easy with elba: all you need is a package name.
Note that names in elba are special in that they are *always
namespaced*; every name in elba comes with a group part and a name part,
separated with a slash. For more information, see the information on
names in the :doc:`manifest chapter <../reference/manifest>`.

.. code-block:: console

   $ elba new asd # won't work: no namespace
   $ elba new grp/asd # ok!

This command will generate a new elba project with name ``grp/asd`` in
the folder ``./asd/``, along with an associated git project. If you want
to omit the git project, pass the option ``--vcs none``.

By default, elba will create a project with a binary target, with a main
file located at ``src/Main.idr``. If you’d like to generate a package
with a library target instead, pass the ``--lib`` flag, which will add a
library target to the manifest and generate the file
``src/{group}/{name}.idr``. This file structure of having a group
followed by a name is just convention, and isn’t required.

Regardless of which target is chosen, an ``elba.toml`` manifest file
will also be generated.

Initializing a pre-existing package
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

If you already have an Idris project and want to turn it into an elba
project, use the ``elba init`` command instead; it follows the exact
same syntax as ``elba new`` and is functionally identical, but uses the
current directory instead of making a new one.

Adding dependencies
-------------------

Now that a new package has been created, you can start to add packages
as part of your dependencies. A package can originate from one of three
places: a git repository, a file directory, or a package index. Ordinary
dependencies are placed under the ``[dependencies]`` section, while
dependencies that are only needed for tests and the like are placed
under ``[dev_dependencies]``. Examples are shown below:

.. code-block:: toml

   [dependencies]
   "index/version" = "0.1.5" # uses the default index (i.e. the first specified one in configuration)
   "index/explicit" = { version = "0.1.5", index = "index+dir+../index" } # uses the index specified
   "directory/only" = { path = "../awesome" } # uses the package in the path specified
   "git/master" = { git = "https://github.com/doesnt/exist" } # uses the master branch
   "git/explicit" = { git = "https://github.com/doesnt/exist", tag = "beta" } # "tag" can be an arbitrary git ref: a branch, a tag, commit, etc.

For more information on the syntax regarding specifying and adding
custom indices, see the chapters on :doc:`../reference/resolutions`
and :doc:`../usage/configuration`. More information about
dependency specification syntax is available at :doc:`its relevant
chapter <../reference/dependencies>`.

Note that only packages with library targets can be depended on.

At this point, you can add whatever files you want and import anything
from your dependencies.

Targets
-------

The manifest also allows you to specify which targets you want to have
built for your package. There are three types of targets:

-  A **library target** allows this package to be depended on by other
   packages. A package can only have one library, and the syntax follows
   the following:

   .. code-block:: toml

      [targets.lib]
      # the path which contains all of the lib files (*cannot* be a parent directory)
      # this is set to "src" by default
      path = "src/"
      # a list of files to export
      mods = [
          "Awesome.A", # the file src/Awesome/A.idr
          "Control.Zygohistomorphic.Prepromorphisms", # the file src/Control/Zygohistomorphic/Prepromorphisms.idr
      ]

-  A **bin target** specifies a binary to be built. Multiple binaries
   can correspond to one package.

   .. code-block:: toml

      [[targets.bin]]
      # the name of the binary to create
      name = "awes"
      # the path which contains all of the bin files (*cannot* be a parent directory)
      # this is set to "src" by default
      path = "src/"
      # the path to the Main module of the binary
      main = "Awesome.B"

   Note: the format of the binary target has some nuance to it, so for
   more information, see the docs on :doc:`the manifest format
   <../reference/manifest>`.

-  A **test target** specifies a test binary to build. It uses the same
   syntax as a bin target, with the difference that we use
   ``[[targets.test]]`` to specify them and the test binary can depend
   on the dev-dependencies as well as the root package’s library. A test
   binary succeeds upon execution if it returns exit code 0.

Building a package
------------------

…can be accomplished with the command:

.. code-block:: console

   $ # assuming the current directory is an elba package
   $ elba build

For all elba build-related commands, the ``IDRIS_OPTS`` environment
variable will dictate additional arguments to pass to the Idris compiler
(the flags passed by elba get higher priority). Additionally, any args
passed after a double-dash will be interpreted as arguments to the
Idris compiler:

.. code-block:: console
                
   $ # adds both the contrib and effects built-in packages
   $ IDRIS_OPTS="-p contrib" elba build -- -p effects

When building a local package, the output binaries are located at
``target/bin``, while the output library is placed at ``target/lib``.

Interactive development with the REPL can also be accomplished with the
command:

.. code-block:: console

   $ # assuming the current directory is an elba package
   $ elba repl

Instead of placing the build outputs in a ``target/`` folder, the
``elba repl`` command directly loads the files on-disk, then cleans up
any build files after execution.

elba uses an ``elba.lock`` lockfile to ensure that these builds are
reproducible. This should be committed to repositories for libraries,
but not for binaries.
