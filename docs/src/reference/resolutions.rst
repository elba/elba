Resolutions
-----------

A core tenet in elba’s functionality is the idea of **resolutions**. A
resolution is a generic location from which some resource (a package or
a :doc:`package index <./indices>`) can be retrieved. Internally, elba
distinguishes between two types of resolutions:

-  A **direct resolution** refers to a direct location from which a
   resource (either a package or a package index) can be downloaded.
   Direct resolutions themselves can include references to tarballs
   (either on a network somewhere or located on disk), local directories
   on disk, or git repositories.
-  An **index resolution** refers to an index from which information
   about a package’s location can be obtained. The location of the index
   itself must be a direct resolution.

A package can have (and is identified by) either a direct resolution or
an index resolution. A package index is identified by its index
resolution.

Syntax
~~~~~~

In order to refer to these types of direct resolutions, elba has its own
simple syntax for “resolution strings”:

-  Each of the types of direct resolutions has its own syntax:

   -  For a direct resolution which points to a tarball, the resolution
      string must start with the identifier ``tar+`` and include a
      properly-formed URL with either the ``http://``/``https://``
      (referring to a tarball on the network somewhere) or ``file://``
      (referring to a local tarball) schemas:

      ::

         These are all valid:
         tar+http://example.com/asdf.tar.gz
         tar+https://example.com/asdf
         tar+file://../asdf.tar.gz

   -  For a direct resolution which points to a directory on disk, the
      resolution string must start with the identifier ``dir+`` and
      include a properly-formed path to a directory on disk:

      ::

         These are all valid:
         dir+asdf
         dir+./asdf
         dir+../asdf/whatever/subfolder

         On Windows, these would be valid too:
         dir+C:\Users\John\etc

   -  For a direct resolution which points to a git repository, the
      resolution string must start with the identifier ``git+`` and
      provide the URL of the repository in question. Additionally, a git
      ref can be specified as part of the fragment of the URL:

      ::

         These are all valid:
         git+https://github.com/example/doesnt-exist
         git+https://github.com/example/doesnt-exist#master <- use the master branch
         git+https://github.com/example/doesnt-exist#v1.0.0 <- use the "v1.0.0" tag
         git+https://github.com/example/doesnt-exist#a4e13343 <- use the commit "a4e13343"
         git+ssh://git@github.com/example/doesnt-exist <- using ssh instead of https

-  For an index resolution, the resolution string must start with the
   identifier ``index+`` and include the direct resolution of the origin
   of the index:

   ::

      These are all valid
      index+tar+http://example.com/asdf.tar.gz
      index+dir+../asdf/whatever/subfolder
      index+git+ssh://git@github.com/example/doesnt-exist#a4e13343
