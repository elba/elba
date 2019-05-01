Registries
==========

Where :doc:`indices <./indices>` can be thought of as the "read-only"
part of a package repository, providing information about packages and
nothing more, a **registry** is a package server which serves the actual
package files and allows users to upload and yank packages from them.

All registries are tied to indices - a registry must have a
corresponding package index (though the opposite isn't necessarily
true). Package registries can be specified as URLs in the configuration
of an index with the following syntax:

.. code-block:: toml
   [index]
   secure = false
   registry = "https://api.elba.pub"

API v1 Endpoints
----------------

It's assumed that all package registries have some sort of auth system
centered around a user token which allows the registry to authenticate
and authorize different users. elba users can log in to a registry
using the ``elba login <token>`` command, where token is the auth token
provided to them by the registry.

In order to function with elba, package registries must support two
basic operations:

-  **Package publishing**: package registries should be able to have
   packages uploaded to them at the PUT endpoint ``/api/v1/publish``.
   The body of this request will be the package in archived (tar.gz)
   form, and the auth token will be provided as a query parameter
   ``token``.

-  **Package yanking**: in order to prevent a left-pad-esque scenario,
   the public interface of a package registry prohibits package
   deletion; instead, packages can be yanked, which means that future
   packages will be unable to depend on said package or package version.
   A package ``group/name|version`` can be yanked at the PATCH endpoint
   ``/api/v1/group/name/version/yank``. This endpoint should accept a
   boolean query parameter ``yanked`` (usually set to ``true``) and a
   query parameter ``token``.

Currently, these are the two endpoints which elba needs to function.
However, the full list of endpoints is much longer than this, and can
be found in the `source code of the reference elba registry
<https://github.com/elba/website/blob/f41ff1dacc741f2d23650932a0e4daacf00e34b8/src/router.rs>`__.
