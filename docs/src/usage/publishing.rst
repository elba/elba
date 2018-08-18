Publishing to the Default Index
===============================

At the moment, elba has neither a fancy online portal for managing
packages nor the proper CLI commands for publishing to a package
repository. This will come in time, but for now, you’ll have to directly
interact with `the GitHub repo of the official
index <https://github.com/elba/index>`__.

The repo’s README has more information on what to do if you want to
publish a package to the default index and what’s allowed and not
allowed, but the summary of how to publish a package is this:

1. Read The elba Guide, especially the parts about :doc:`names in
   elba <./manifest>`, :doc:`resolutions in elba
   <../reference/resolutions>`, and :doc:`package indices
   <../reference/indices>`.

2. Fork the index and modify it as needed.

3. Submit a pull request back in the original repo (``elba/index``).

Hopefully in the future we’ll have a better story for package
publishing, but for now, it is what it is.
