# Usage

This chapter describes how to use Clippy to get the most out of it. Clippy can
be used as a `cargo` subcommand or, like `rustc`, directly with the
`clippy-driver` binary.

> _Note:_ This chapter assumes that you have Clippy installed already. If you're
> not sure, take a look at the [Installation] chapter.

## Cargo subcommand

The easiest and most common way to run Clippy is through `cargo`. To do that,
just run

```bash
cargo clippy
```

### Lint configuration

The above command will run the default set of lints, which are included in the
lint group `clippy::all`. You might want to use even more lints or you might not
agree with every Clippy lint, and for that there are ways to configure lint
levels.

> _Note:_ Clippy is meant to be used with a generous sprinkling of
> `#[allow(..)]`s through your code. So if you disagree with a lint, don't feel
> bad disabling them for parts of your code or the whole project.

#### Error on lints

Instead of only emitting warnings you may want Clippy to emit errors and abort
the compilation instead. This is especially useful if you run Clippy in [CI].
You can do that with

```
cargo clippy -- -Dwarnings
```

> _Note:_ that adding `-D warnings` will cause your build to fail if **any**
> warnings are found in your code. That includes warnings found by rustc (e.g.
> `dead_code`, etc.). If you want to avoid this and only cause an error for
> Clippy warnings, use `-D clippy::all` on the command line. (You can swap
> `clippy::all` with the specific lint category you are targeting.)

#### Even more lints

Clippy has two lint groups which are allow-by-default. This means, that you will
have to enable the lints in those groups manually.

##### `clippy::pedantic`

The first group is the `pedantic` group. This group contains really opinionated
lints, that may have some intentional false positives in order to prevent false
negatives. So while this group is ready to be used in production, you can expect
to sprinkle multiple `#[allow(..)]`s in your code.

> FYI: Clippy uses the whole group to lint itself.

##### `clippy::restriction`

The second group is the `restriction` group. This group contains lints that
"restrict" the language in some way. For example the `clippy::unwrap` lint from
this group won't allow you to use `.unwrap()` in your code. You may want to look
through the lints in this group and enable the ones that fit your need.

> _Note:_ You shouldn't enable the whole lint group, but cherry-pick lints from
> this group. Some lints in this group will even contradict other Clippy lints!

#### Too many lints

The most opinionated warn-by-default group of Clippy is the `clippy::style`
group. Some people prefer to disable this group completely and then cherry-pick
some lints they like from this group. The same is of course possible with every
other of Clippy's lint groups.

> _Note:_ We try to keep the warn-by-default groups free from false positives
> (FP). If you find that a lint wrongly triggers, please report it in an issue
> (if there isn't an issue for that FP already)

#### Command line

You can configure lint levels on the command line by adding
`-A/W/Dclippy::lint_name` like this:

```bash
cargo clippy -- -Aclippy::style -Wclippy::double_neg -Dclippy::perf
```

#### Source Code

You can configure lint levels in source code the same way you can configure
`rustc` lints:

```rust
#![allow(clippy::style)]

#[warn(clippy::double_neg)]
fn main() {
    let x = 1;
    let y = --x;
    //      ^^ warning: double negation
}
```

### Automatically applying Clippy suggestions

Clippy can automatically apply some lint suggestions, just like the compiler.

```terminal
cargo clippy --fix
```

### Workspaces

All the usual workspace options should work with Clippy. For example the
following command will run Clippy on the `example` crate in your workspace:

```terminal
cargo clippy -p example
```

As with `cargo check`, this includes dependencies that are members of the
workspace, like path dependencies. If you want to run Clippy **only** on the
given crate, use the `--no-deps` option like this:

```terminal
cargo clippy -p example -- --no-deps
```

## Using Clippy without `cargo`: `clippy-driver`

Clippy can also be used in projects that do not use cargo. To do so, run
`clippy-driver` with the same arguments you use for `rustc`. For example:

```terminal
clippy-driver --edition 2018 -Cpanic=abort foo.rs
```

> _Note:_ `clippy-driver` is designed for running Clippy and should not be used
> as a general replacement for `rustc`. `clippy-driver` may produce artifacts
> that are not optimized as expected, for example.

[Installation]: installation.md
[CI]: continuous_integration
