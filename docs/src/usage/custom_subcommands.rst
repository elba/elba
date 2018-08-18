Custom Subcommands
==================

To support extensibility in the future, elba supports running custom
subcommands if it is passed a subcommand which doesn’t exist. All
arguments which were passed to elba will be instead passed to the
subcommand:

.. code-block:: console

   $ elba installnt # executes `elba-installnt`
   $ elba installnt awesome one two three # executes `elba-installnt awesome one two three`
   $ elba installnt --cool awesome --one -f # executes `elba-installnt --cool awesome --one -f`

elba is also available as a Rust library, meaning that subcommands
written in Rust can take advantage of elba’s internal data structures
and functions. This opens a variety of possibilities: using custom
project scaffolds and templates, running special heuristics on elba
projects, etc.
