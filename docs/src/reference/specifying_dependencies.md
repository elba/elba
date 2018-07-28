# Specifying Dependencies

Packages in elba can depend on other packages in external [indices](./indices.md), a local file directory, or a git repository.

## Versions

Versions in elba follow a slightly modified version of [Semantic Versioning](https://semver.org/) in order to ensure that packages stay compatible with each other. Most of the core concepts of Semantic Versioning are carried over:

- Differences in the major version indicate backwards incompatibility.
- Differences in the minor version indicate feature additions.
- Differences in the patch version indicate bug fixes or other non-feature additions.
- Pre-release versions can be indicated with suffixes: `1.0.0-pre.2-beta.5`

In version constraints, the second and third components of a version can be omitted, in which case they are assumed to be `0`. A pre-release cannot be specified without also specifying the second and third components.

## Version constraints

We say that a constraint **satisfies** a particular version if that particular version falls within the version constraint.

elba's version constraints offer all the same standard operators (`<`, `>`, `^`, `~`, etc.), but they have some idiosyncrasies which distinguish them from how other package managers
 work.

### Inequality constraints

The "lowest-level" constraints elba offers are **inequality constraints**, which are fairly simple: `< 1.0.0`, `>= 1.0.0`, etc.

By default, **`<` constraints will ignore pre-release versions**. for ergonomic reasons. If a package specifies that they depend on `< 1.0.0`, they likely don't want to have any of the pre-release versions of 1.0.0 selected, even if those technically satisfy the constraint. If a package wants to include the pre-release versions as well it can opt in to pre-releases by adding a bang after the constraint symbol like so: `<! 1.0.0`.

The bang trick also works for `>=` constraints as well: while `>= 1.0.0` doesn't match pre-releases of `1.0.0`, `>=! 1.0.0` does.

The constraint parser will allow you to add bangs to all types of less-than or greater-than constraints, but some of them won't do anything: `<= 1.0.0` and `<=! 1.0.0` mean the exact same thing, as do `> 1.0.0` and `>! 1.0.0`.

Additionally, if the constraint specifies a pre-release, it will satisfy other pre-releases.

Two inequality constraints can be intersected to produce a new compound constraint. Note that at the moment, this is the only case in which the parser will accept multiple constraints. Additionally, the greater-than bound must be written before the less-than bound.

The new constraint must allow at least one version for it to be valid:

```
>= 1.0.0 < 1.4.2 # valid
>= 1.0.0 <= 1.0.0 # valid
< 1 > 0 # invalid: less-than specified before greater-than
> 1 < 0 # invalid: impossible constraint (satisfies no versions)
```

### Caret constraints

**Caret constraints** in elba function the same as in other package managers. To quote [Cargo's documentation](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#caret-requirements):

> Caret requirements allow SemVer compatible updates to a specified version. An update is allowed if the new version number does not modify the left-most non-zero digit in the major, minor, patch grouping.

Here are some examples of caret constraints (also taken from Cargo's documentation):

```
^1.2.3 := >= 1.2.3 < 2.0.0
^1.2   := >= 1.2.0 < 2.0.0
^1     := >= 1.0.0 < 2.0.0
^0.2.3 := >= 0.2.3 < 0.3.0
^0.2   := >= 0.2.0 < 0.3.0
^0.0.3 := >= 0.0.3 < 0.0.4
^0.0   := >= 0.0.0 < 0.1.0
^0     := >= 0.0.0 < 1.0.0
```

A version without a sigil or inequality is assumed to be a caret constraint.

### Tilde constraints

**Tilde constraints** are slightly stricted than caret constraints. If a tilde constraint specifies a major and minor version, only changes in the patch version are allowed. If only a major version is specified, changes in the minor and patch versions are allowed.

```
~1.2.3 := >= 1.2.3 < 1.3.0
~1.2   := >= 1.2.0 < 1.3.0
~1     := >= 1.0.0 < 2.0.0
~0.2.3 := >= 0.2.3 < 0.3.0
~0.2   := >= 0.2.0 < 0.3.0
~0.0.3 := >= 0.0.3 < 0.1.0
~0.0   := >= 0.0.0 < 0.1.0
~0     := >= 0.0.0 < 1.0.0
```
### The `any` constraint

If a package doesn't care about what version of a package it uses (which it really should; it's impossible to guarantee infinite perpetual forwards compatibility with a package), the `any` constraint can be used, which satisfies every version.

### Combining constraints with unions

Multiple constraints can be combined to form a larger constraint by placing a comma in between each constraint, like so: `1.0.0, 2.0.0, >= 3.1.3 <= 3.1.3`. This constraint represents the **union** between its three component constraints, and it requires that the version has either a major version `1` or `2`, or that it's equal to `3.1.3`.

## Dependency specifications

TODO: version, version + index, git, path