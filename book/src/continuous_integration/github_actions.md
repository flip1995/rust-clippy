# GitHub Actions

## actions-rs

An easy way to get started with adding Clippy to GitHub Actions is
the [actions-rs](https://github.com/actions-rs) [clippy-check](https://github.com/actions-rs/clippy-check):

```yml
on: push
name: Clippy check
jobs:
    clippy_check:
        runs-on: ubuntu-latest
        steps:
            -   uses: actions/checkout@v1
            -   run: rustup component add clippy
            -   uses: actions-rs/clippy-check@v1
                with:
                    token: ${{ secrets.GITHUB_TOKEN }}
                    args: --all-features

```