Indices
-------

A **package index** is a source of metadata for available packages,
mapping package names and versions to requisite dependencies and a
location to retrieve the package. Package indices serve several purposes
in elba’s package management system:

-  Package indices group together versions of packages to make depending
   on and installing packages easier, more convenient, and less prone to
   breakage (á la RubyGems, crates.io)

-  Package indices can serve to curate sets of packages which are known
   to work together correctly (á la Stackage)

-  They provide a level of indirection for packages; consumers of
   packages don’t have to be tied to directly depending on a certain git
   repository or tarball, they can just rely on wherever the index says
   the package is located.

Packages within package indices are capable of depending on packages in
other indices (so long as the index specifies all of the indices it
depends on), and users of elba can specify multiple package indices to
pull from. Additionally, packages in package indices can have arbitrary
direct resolutions as their actual location. This makes elba’s package
indices extremely powerful as a consequence.

Users can have their packages appear in indices by uploading them to
their corresponding :doc:`registries <./registries>`.

Index Resolutions
~~~~~~~~~~~~~~~~~

An index is identified primarily by its index resolution, which
corresponds to the place where the index is made available. For more
information, see the previous chapter on :doc:`./resolutions`.

In the ``elba.toml`` file, when a package requirement is declared with a
certain version, elba goes through the following steps to decide which
package index to use:

-  If the resolution of an index is provided in the dependency
   specification, elba will use that index.

   .. code-block:: toml

      [dependencies]
      "test/one" = { version = "0.1.0", index = "index+dir+/index" }
      # for this package, elba will use the index located on-disk at `/index`.

-  If no resolution is provided, elba will default to *the first index
   listed in configuration*.

   .. code-block:: toml

      # .elba/config
      indices = [
          "index+dir+/one",
          "index+dir+/two"
      ]

      # elba.toml
      [dependencies]
      "test/two" = "0.1.0"
      # for this package, elba will use the index located on-disk at `/one`.

Note that if a declared dependency uses an index that isn’t specified in
the configuration, the package will fail to build during dependency
resolution with a “package not found” error.

``index.toml``
~~~~~~~~~~~~~~

A package index is (when extracted, for tarballs) a directory tree of
metadata files. All package indices must have a configuration file at
the root of this directory tree named ``index.toml``, and specify the
following keys:

.. code-block:: toml

   [index]
   secure = false

   [index.dependencies]

The ``secure`` key tells elba whether to treat the index like a secure
package index. At the moment, this flag does nothing, but in the future,
this flag may be used to enable compatibility with `The Update
Framework <https://theupdateframework.github.io/>`__. For forwards
compatibility, package index maintainers should set this key to
``false``.

The ``dependencies`` key is a mapping from the “name” of an index to its
index resolution. The name can be whatever you want, but that name will
be how the index will be referred to within metadata files. Every other
index which the packages of this index need to build properly must be
specified in this field, or else package building will fail during
dependency resolution.

An additional key, ``registry``, should be the url of the registry API.

Metadata structure
~~~~~~~~~~~~~~~~~~

Package indices must follow a fairly strict folder and file structure in
order for elba to interpret them correctly. The top-level folders should
be groups, and underneath the folder for each group should be a metadata
file corresponding to a package. The name of that file should be the
second portion of the package’s name:

.. code:: sh

   # an example index:
   .
   |-- group
   |   |-- name # metadata file corresponding to the package `group/name`
   |   +-- cool # metadata file corresponding to the package `group/cool`
   |-- next
   |   +-- zzz # metadata file corresponding to the package `next/zzz`
   |
   +-- index.toml

Each line of the metadata file for a package should be a complete JSON
object corresponding to a specific version of a package, and should
follow the following structure (pretty-printed for readability):

.. code:: json

   {
     "name": "no_conflict/root",
     "version": "1.0.0",
     "dependencies": [
       {
         "name": "no_conflict/foo",
         "req": "1.0.0"
       },
       {
         "name": "awesome/bar",
         "index": "best_index",
         "req": ">= 0.1.0"
       }
     ],
     "yanked": false,
     "location": "dir+test"
   }

The ``name`` and ``version`` fields should be self-explanatory. The
``dependencies`` section should be a list of objects with fields
``name``, ``index``, and ``req``. ``name`` is self-explanatory, and
``req`` is just the version constraint of that particular dependency.
The value in ``index`` should correspond to an index name specified
within the index’s config; if the index is unspecified or if the index
name can’t be found in configuration, elba will assume that the package
is available from the current index.

The ``yanked`` field allows for “yanking” of a package, which disallows
future consumers of a package from using that version (but allows
current consumers of a yanked package version to continue using it).
Finally, the ``location`` field indicates the direct resolution of the
package in question.

Index Retrieval Semantics
~~~~~~~~~~~~~~~~~~~~~~~~~

To avoid constantly updating the package index, elba will only update
its indices if it’s building a global project (i.e. ``elba install``),
or if a package cannot be found in the locally cached indices or changes
versions in such a way that is incompatible with an existing lockfile.
This means that if an index changes the resolution of a package, the
package indices might not be updated immediately.
