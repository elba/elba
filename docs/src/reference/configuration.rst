Configuration
=============

elba’s behavior can be configured through the use of TOML configuration
files and environment variables. elba checks the current directory and
all of its ancestors for a ``.elba/config`` file, unifying them in the
following order (from highest to lowest priority):

.. code-block:: console

   # assuming current directory is /foo/bar/baz/quux
   /foo/bar/baz/quux/.elba/config
   /foo/bar/baz/.elba/config
   /foo/bar/.elba/config
   /foo/.elba/config
   /.elba/config
   # Your platform-specific config file would go here
   # - Linux: ~/.config/elba/config
   # - macOS: /Users/<user>/Library/Preferences/elba/config
   # - Windows: %LOCALAPPDATA%\elba\config\config
   $HOME/.elba/config

Any specified environment variables have the highest priority. This
behavior heavily borrows from `Cargo’s configuration
format <https://doc.rust-lang.org/cargo/reference/config.html>`__.

Additionally, whenever elba executes an Idris invocation, elba will pass
all of the arguments in the environment variable ``IDRIS_OPTS`` to the
compiler. In any case where the ``IDRIS_OPTS`` args conflict with elba’s
own flags (i.e. if the user specifies the flag ``--ide-mode`` but elba
specifies ``--check``), elba will override the user-specified flag.

Config Format
-------------

A complete default elba configuration file is listed below. Any options
which are not assigned to will carry the default value instead.

.. code-block:: toml

   compiler = "idris"
                
   [indices]
   "official" = "index+git+https://github.com/elba/elba"

   [term]
   verbosity = "normal"
   color = "true"

   [alias]
   i = "install"
   b = "build"
   t = "test"

   [directories]
   cache = "$HOME/.elba"

   [[backend]]
   name = "c"
   default = true
   portable = false
   opts = []

..

   .. rubric:: Using environment variables
      :name: using-environment-variables

   In order to specify an option as an environment variable, simply
   replace the “dots” of the option with underscores, and prefix with
   ``ELBA_``. So the option ``term.verbosity`` becomes
   ``ELBA_TERM_VERBOSITY``.
   
``compiler``
~~~~~~~~~~~~

The compiler key specifies the name of the executable of the Idris
compiler. By default it is set to "idris". You should **not** pass
any command line options in this string, as elba will search the
path for an executable with the name of this string.

elba is smart enough to detect the version of the compiler - whether
it's Idris 1 or 2 (Blodwen). If it can't tell what version the compiler
is, it'll default to the behavior for Idris 1.

``indices``
~~~~~~~~~~~

This key plays a few different roles based on the context of the elba
operation:

-  When building a local package or running a command which takes a
   ``--index`` command-line flag, this key defines aliases for indices;
   this way, you don't have to completely write out the resolution of
   an index to refer to it (but you can if you want).

   For both the command-line flag and when building a package, if an
   index is specified, elba will first see if it's an alias for another
   index. If not, it will try to parse the index as an index resolution.

   The first index specified is set as the default index when building
   a package. For commands with an ``--index`` flag, elba will require
   that you specify what index you're referring to if the config lists
   multiple indices.

-  When building a package which originates from an index, this key
   defines all the indices that will be searched for the package.

By default, the first and only index available to elba is the `official
package index <https://github.com/elba/index>`__.

``[profile]``
~~~~~~~~~~~~~

This section specifies the default author information that should be
provided upon creating or initializing a new elba project. By default,
this section has no value, so new projects are made without an author.

.. code-block:: toml

   [profile]
   name = "John Smith"
   email = "jsmith@example.com"

``[term]``
~~~~~~~~~~

This section specifies options for terminal output, and has two fields:

-  ``verbosity``: specifies how verbose elba should be. Can be one of
   ``verbose``, ``normal``, ``quiet``, or ``none``.
-  ``color``: specifies if elba should try to print color output. Either
   ``true`` or ``false``.

At the moment, neither of these options actually do anything.

``[alias]``
~~~~~~~~~~~

This section is for providing aliases for commands. The key represents
the alias and the value represents the the command that it should be
aliased to. Note that aliases can alias to other aliases, which can
cause *infinite recursion of aliases*. Be careful.

.. code-block:: console

   $ elba b # builds the local package with the default alias settings

``[directories]``
~~~~~~~~~~~~~~~~~

This section only contains one key: ``cache``, for the location where
the global cache should be placed. This controls not only the location
of elba’s temporary build directories but also the location of the
global bin directory.

``[[backend]]``
~~~~~~~~~~~~~~~

This section specifies information about codegen backends. By default,
information about one default codegen is provided: the C backend. These
settings are used whenever a codegen backend is unspecified or a codegen
backend is specified but doesn’t have any information on it available in
the configuration. A example full ``[[backend]]`` section is provided
below:

.. code-block:: toml

   [[backend]]
   # The name of the backend, passed to the --codegen or --portable-codegen
   # compiler option
   name = "awesome"
   # Whether this should be treated as a new default codegen backend, instead of
   # the c one provided by default. Note that if multiple backends have default set
   # to true, the backend mentioned first will be used as the default
   default = true
   # Whether or not this backend is portable
   portable = false
   # The command to use to run executables generated by this codegen backend
   # If omitted, the executable will just be run by itself
   runner = "awesomec"
   # The extension to use for executables generated by this codegen backend
   # elba will pass the name of the binary/test target with this extension set to
   # the -o flag of the Idris compiler
   # If unset, no extension-setting will happen
   extension = "awe"
   # Options to be passed to the codegen backend
   opts = []
