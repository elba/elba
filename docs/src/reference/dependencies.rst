Dependencies
============

The most important job of a package manager is building dependencies of
a package. Packages in elba can depend on other packages in external
:doc:`indices <./indices>`, a local file directory, or a git repository.

Versions
--------

Versions in elba follow a slightly modified version of `Semantic
Versioning <https://semver.org/>`__ in order to ensure that packages
stay compatible with each other. Most of the core concepts of Semantic
Versioning are carried over:

-  Differences in the major version indicate backwards incompatibility.
-  Differences in the minor version indicate feature additions.
-  Differences in the patch version indicate bug fixes or other
   non-feature additions.
-  Pre-release versions can be indicated with suffixes:
   ``1.0.0-pre.2-beta.5``

In version constraints, the second and third components of a version can
be omitted, in which case they are assumed to be ``0``. A pre-release
cannot be specified without also specifying the second and third
components.

Version constraints
~~~~~~~~~~~~~~~~~~~

We say that a constraint **satisfies** a particular version if that
particular version falls within the version constraint.

elba’s version constraints offer all the same standard operators (``<``,
``>``, ``^``, ``~``, etc.), but they have some idiosyncrasies which
distinguish them from how other package managers work.

Inequality constraints
~~~~~~~~~~~~~~~~~~~~~~

The “lowest-level” constraints elba offers are **inequality
constraints**, which are fairly simple: ``< 1.0.0``, ``>= 1.0.0``, etc.

By default, **``<`` constraints will ignore pre-release versions**. for
ergonomic reasons. If a package specifies that they depend on
``< 1.0.0``, they likely don’t want to have any of the pre-release
versions of 1.0.0 selected, even if those technically satisfy the
constraint. If a package wants to include the pre-release versions as
well it can opt in to pre-releases by adding a bang after the constraint
symbol like so: ``<! 1.0.0``.

The bang trick also works for ``>=`` constraints as well: while
``>= 1.0.0`` doesn’t match pre-releases of ``1.0.0``, ``>=! 1.0.0``
does.

The constraint parser will allow you to add bangs to all types of
less-than or greater-than constraints, but some of them won’t do
anything: ``<= 1.0.0`` and ``<=! 1.0.0`` mean the exact same thing, as
do ``> 1.0.0`` and ``>! 1.0.0``.

Additionally, if the constraint specifies a pre-release, it will satisfy
other pre-releases.

Two inequality constraints can be intersected to produce a new compound
constraint. Note that at the moment, this is the only case in which the
parser will accept multiple constraints. Additionally, the greater-than
bound must be written before the less-than bound.

The new constraint must allow at least one version for it to be valid:

::

   >= 1.0.0 < 1.4.2 # valid
   >= 1.0.0 <= 1.0.0 # valid
   < 1 > 0 # invalid: less-than specified before greater-than
   > 1 < 0 # invalid: impossible constraint (satisfies no versions)

Caret constraints
~~~~~~~~~~~~~~~~~

**Caret constraints** in elba function the same as in other package
managers. To quote `Cargo’s
documentation <https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#caret-requirements>`__:

   Caret requirements allow SemVer compatible updates to a specified
   version. An update is allowed if the new version number does not
   modify the left-most non-zero digit in the major, minor, patch
   grouping.

Here are some examples of caret constraints (also taken from Cargo’s
documentation):

::

   ^1.2.3 := >= 1.2.3 < 2.0.0
   ^1.2   := >= 1.2.0 < 2.0.0
   ^1     := >= 1.0.0 < 2.0.0
   ^0.2.3 := >= 0.2.3 < 0.3.0
   ^0.2   := >= 0.2.0 < 0.3.0
   ^0.0.3 := >= 0.0.3 < 0.0.4
   ^0.0   := >= 0.0.0 < 0.1.0
   ^0     := >= 0.0.0 < 1.0.0

A version without a sigil or inequality is assumed to be a caret
constraint.

Tilde constraints
~~~~~~~~~~~~~~~~~

**Tilde constraints** are slightly stricted than caret constraints. If a
tilde constraint specifies a major and minor version, only changes in
the patch version are allowed. If only a major version is specified,
changes in the minor and patch versions are allowed.

::

   ~1.2.3 := >= 1.2.3 < 1.3.0
   ~1.2   := >= 1.2.0 < 1.3.0
   ~1     := >= 1.0.0 < 2.0.0
   ~0.2.3 := >= 0.2.3 < 0.3.0
   ~0.2   := >= 0.2.0 < 0.3.0
   ~0.0.3 := >= 0.0.3 < 0.1.0
   ~0.0   := >= 0.0.0 < 0.1.0
   ~0     := >= 0.0.0 < 1.0.0

The ``any`` constraint
~~~~~~~~~~~~~~~~~~~~~~

If a package doesn’t care about what version of a package it uses (which
it really should; it’s impossible to guarantee infinite perpetual
forwards compatibility with a package), the ``any`` constraint can be
used, which satisfies every version.

Combining constraints with unions
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Multiple constraints can be combined to form a larger constraint by
placing a comma in between each constraint, like so:
``1.0.0, 2.0.0, >= 3.1.3 <= 3.1.3``. This constraint represents the
**union** between its three component constraints, and it requires that
the version has either a major version ``1`` or ``2``, or that it’s
equal to ``3.1.3``.

Dependency Resolution
---------------------

Dependency resolution for packages is an extremely hard problem
(possibly/probably NP-complete). In order to figure out which versions
of a package should be used, elba uses the `Pubgrub
algorithm <https://github.com/dart-lang/pub/blob/master/doc/solver.md>`__
to do its dependency resolution.

While all of the gory details of how the algorithm works are available
both at that design document and the `Pub
documentation <https://www.dartlang.org/tools/pub/versioning>`__ (where
Pubgrub was first implemented), the main consequence of this decision is
that **only one version of a package can be used at a time.** If
separate packages depend on different incompatible versions of the same
package, elba will return an error during dependency resolution and will
refuse to continue until the conflict is solved.

On the one hand, this aspect of the dependency resolution system has its
fair share of drawbacks:

-  “Dependency hell” becomes much harder to avoid, since every dependent
   package is limited to one and only one version
-  Getting an ecosystem to upgrade major versions of a package can be
   much more challenging, as the entire ecosystem is locked to the
   “stragglers” stuck on previous versions

However, it does have its advantages:

-  Because there will be only one version of a package present at all
   times, any data structures or functions provided by that package can
   be used freely across between dependencies without fear of
   incompatibile data structures due to version differences
-  Restricting users to one version of a package simplifies module name
   conflicts

Additionally, one benefit that elba gains from using the Pubgrub
algorithm is that elba can provide extremely clear error reporting to
help pinpoint and fix the conflict in question. For example, given a
dependency tree that looks like this:

-  ``conflict_simple/root|1.0.0`` depends on
   ``conflict_simple/foo ^1.0.0`` and ``conflict_simple/baz ^1.0.0``.
-  ``conflict_simple/foo|1.0.0`` depends on
   ``conflict_simple/bar ^2.0.0``.
-  ``conflict_simple/bar|2.0.0`` depends on
   ``conflict_simple/baz ^3.0.0``.
-  ``conflict_simple/baz|1.0.0`` and ``3.0.0`` have no dependencies.
-  All these packages are located at the index ``index+dir+/index/``.

elba will print the following output when trying to build it:

.. code-block:: console

   $ elba build
   snip...
   [error] version solving has failed

   Because conflict_simple/bar@index+dir+/index/ any depends on
   conflict_simple/baz@index+dir+/index/ >=3.0.0 <4.0.0,
   conflict_simple/baz@index+dir+/index/ <!3.0.0, >=!4.0.0 is impossible.
   And because conflict_simple/root@index+dir+/index/ >=1.0.0 <=1.0.0 depends
   on conflict_simple/baz@index+dir+/index/ >=1.0.0 <2.0.0,
   conflict_simple/root@index+dir+/index/ >=1.0.0 <=1.0.0 is impossible.

Nice!
