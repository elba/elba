Installing a Package
====================

elba can build and install the binary targets of packages into a global
directory (this directory is the ``bin`` subfolder under the folder of
the global cache; under normal circumstances, this should be located at
``~/.elba/bin``). In order for these executables to be run from
anywhere, you should this global bin folder to your ``PATH``.

Installing a local package
--------------------------

To install a package which is located on-disk, simply navigate to the
directory of the package and whack:

.. code-block:: console

   $ elba install

Doing that should rebuild the package if needed and install its binaries
into the global bin folder.

Note that if a binary with the same name as one of the binaries being
installed already exists, the above command will fail. If you’re
absolutely sure that you want to replace the old binary, run the command
again but with the ``--force`` flag. Additionally, if you only want to
install certain binaries, you can use the ``--bin`` flag:

.. code-block:: console

   $ elba install --bin yeet # only install the binary named "yeet"

Installing a package from an index
----------------------------------

If one or more package indices is :doc:`specified in elba’s
configuration <./configuration>`, you also have the option of
installing a package from one of those indices. ``elba install``
optionally takes a **package spec** as an argument, which consists of
three parts:

-  The name of the package to install (required)
-  The **resolution** of the package; for the time being, this must be
   the resolution of an index (see
   :doc:`../reference/resolutions`)
-  The version of the package

The following are examples of valid ``elba install`` invocations:

.. code-block:: console

   $ # installs the latest version of `jsmith/one` from any index it can:
   $ elba install "jsmith/one"
   $ # installs version 1.0.0 of `jsmith/one` from any index it can:
   $ elba install "jsmith/one|1.0.0"
   $ # installs the latest version of `jsmith/one` from the index specified:
   $ elba install "jsmith/one@index+tar+https://example.com/index.tar.gz"
   $ # installs version 1.0.0 of `jsmith/one` from the index specified:
   $ elba install "jsmith/one@index+tar+https://example.com/index.tar.gz|1.0.0"

As with installing a local package, if you want to replace any old
binaries in the global bin directory, use the ``--force`` flag, and if
you want to choose which binaries to install, use the ``--bin`` flag.

Note that if a spec can apply to multiple packages at the same time (i.e.
a package index wasn't specified and multiple package indices offer a
package with the same name), elba will require you to provide more info
to disambiguate between the packages.

Uninstalling a package
----------------------

Uninstalling a package is much the same process as installing: just pass
a spec to the ``elba uninstall`` invocation. Just like with
``elba install``, if you specify an ambiguous spec, elba will require
you to qualify it further.

