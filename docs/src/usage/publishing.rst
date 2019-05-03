Publishing Packages
===================

elba dependencies can be hosted on
:doc:`registries <../reference/registries>`, which are online package
repositories which host packages (think crates.io or npm) that are
associated with :doc:`indices <../reference/indices>`. The process
of publishing a package involves multiple steps (some of which are
handled for you).

Package registries are specified in the configuration of an index; for
users, this means a registry can be added by just adding the
corresponding package index. See the relevant reference docs for more
details.

Logging in
----------

Before you can upload a package to a registry, you must be authenticated
with that registry. Each package registry might do authentication in a
slightly different way, but all authentication systems revolve around
you being given an authentication token which can be used in the elba
command-line interface.

For the `official registry <https://elba.pub>`_, logging in can be done
by clicking the "Log in" button at the top of the page and
authenticating with Github (other authentication methods aren't
currently being implemented, but we're open to contributions!). After
this is done, you can add an auth token which can be used to log into
the elba cli.

For any other potential registries, the process might differ, but the
end result should be that you get an auth token.

In order to log into a registry, use the ``elba login`` subcommand. If you
have multiple indices specified in your configuration, you can specify
the index to use with the ``--index`` flag. For example:

.. code-block:: console

   $ elba login a67fc893bccfea2141 --index index+git+https://github.com/elba/index

Otherwise, elba will use the default index (the first index specified in
configuration).

Login information is saved to the ``logins.toml`` file in the platform-
specific data directory:

-  On Linux, this is either ``$XDG_DATA_HOME/elba`` or
   ``~/.local/share/elba``.

-  On Windows, this is at ``%ROAMINGAPPDATA\elba``.

-  On macOS, this is at ``~/Library/Application/Support/elba``.

Packaging/archiving
-------------------

Before a package is published, it must be compressed into a tarball with
the ``tar.gz`` extension. The ``elba publish`` command will do this
automatically for you; however, you can also do this step yourself with
the ``elba package`` subcommand.

When run in an elba project directory or subdirectory, this command will
build all targets of a project to make sure it builds successfully, then
package the source code of the project into a tarball.

If you'd like to skip the verification process, you can pass the
``--no-verify`` flag to the command.

Ignoring files
~~~~~~~~~~~~~~

In some cases, you might not want to include every file in the current
directory in the tarball. For one thing, elba will automatically ignore
any files specified in a ``.gitignore`` file in the current project.
Additionally, you can specify files to ignore in the manifest file of
your project, under the ``package.ignore`` key.

``package.ignore`` is a list which accepts individual "lines" of a
.gitignore files as list elements. An example is provided below:

.. code-block:: toml

   [package]
   # snip: other package metadata
   # ignoring files that end with .out or .dev
   ignore = ["*.out", "*.dev"]

Uploading a package
-------------------

Uploading a package can be accomplished with the corresponding command:
``elba publish``, which verifies that a package builds, packages it into
a tarball, and uploads it to a registry. Similar to ``elba login``, you
can pass the ``--index`` flag to specify which index this command should
apply to. Unlike ``elba package``, you can't disable package
verification: all targets **must** build in order to upload your package
to an index.

Yanking: for when things go wrong
---------------------------------

After a package has been published to a package registry, there is no
API endpoint or elba feature which allows for deleting a package (this
circumvents the "left-pad" problem). However, there is a **yanking**
feature (similar to crates.io), which lets you disable a version of
a package from being dependended on or retrieved by any future package
consumers.

The relevant subcommand is ``elba yank``, and it takes one positional
argument: the package and package version to yank, specified in the form
``group/name|version``. It also takes the optional ``--index`` flag,
like ``elba login`` and ``elba publish``.

You can also provide the ``--unyank`` flag, which does exactly what it
says on the tin.
