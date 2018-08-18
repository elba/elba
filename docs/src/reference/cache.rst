The Global Cache
================

elba uses an internal global cache to store downloaded packages, build
packages in a temporary clean directory, and store built packages for
future re-use. The structure of the global cache looks like the
following:

.. code-block:: console

   this directory is platform specific:
   - Linux: ~/.cache/elba
   - Windows: %LOCALAPPDATA%\elba
   - macOS: /Users/<user>/Library/Caches/elba
   |
   |-- build
   |   |-- a78bu877c78deadbeef...
   |   +-- # snip
   |-- indices
   |   |-- d3237be53e69715112f...
   |   +-- # snip
   |-- src
   |   |-- d2e4a311d3323b784ef...
   |   +-- # snip
   +-- tmp
       |-- a78bu877c78deadbeef...
       +-- # snip

Installed binaries
------------------

Binaries are special in that they get their own folder separate from the
internal cache stuff. Ordinarily this is stored at ``~/.elba/bin`` for
all systems, but this can be controlled in the config, separate from the
cache dir. Deleting the whole folder should be safe, but deleting
individual binaries might not be; if you try to uninstall them later
down the line, you might get an error.

Folder structure
----------------

``build``
~~~~~~~~~

This folder stores the binary (i.e. ``.ibc`` file) outputs of library
builds. elba globally caches the builds of all dependencies to avoid
having to rebuild the same library over and over across different
projects. Each built version of a package gets its own hash which
encapsulates the entire environment under which the package was built
(package dependencies, etc.), ensuring reproducible builds. This
emulates the Nix package manager in some respects.

This folder and its subfolders are safe to delete, although it may cause
rebuilds of some packages.

``indices``
~~~~~~~~~~~

This folder stores the downloaded package indices as specified in elba’s
:doc:`configuration <../usage/configuration>`, with a hash corresponding
to each different package index.

This folder and its subfolders are safe to delete; elba will redownload
any needed indices on its next invocation.

``src``
~~~~~~~

This folder stores the downloaded sources of packages. elba globally
caches these to avoid having to redownload the same files over and over
again.

This folder and its subfolders are safe to delete, although it may cause
having to redownload and rebuild some packages.

``tmp``
~~~~~~~

This folder is a temporary build directory for packages, and is more of
an implementation detail than anything else. Folders correspond to build
hashes for packages, and the internal structure of these folders mirrors
the ``target/`` directory of a local package build.

This folder and its subfolders can be safely deleted.

Cleaning the cache
------------------

…can be accomplished with the following invocation:

.. code-block:: console

   $ elba clean

Doing so clears the ``artifacts``, ``build``, ``indices``, ``src``, and
``tmp`` directories.
