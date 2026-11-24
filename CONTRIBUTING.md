# Contributing to EclipticaHUD

I built EclipticaHUD for my own use first and I maintain it alone. I'm picky about how the code is structured, how it's styled and how features fit together. If you hit a bug or want a change, please open an [issue](https://github.com/RealWhyKnot/EclipticaHUD/issues/new/choose) instead of sending a surprise pull request. A description of the problem, the steps to reproduce it or what you're trying to do is more useful to me than an unannounced PR.

If you'd like to help maintain EclipticaHUD over time, especially if I'm committing to it less often, or you have a substantial change you want to build, open an issue or reach out first. I'm happy to talk through the approach and agree on code style and commit conventions before you write any code.

## If we agree on a PR

Link the issue where we settled the approach in the PR description.

### Setting up

See [Build](README.md#build) for the toolchain. Turn on the repository's hooks after cloning:

```powershell
git config core.hooksPath .githooks
```

### Checks

CI runs these on Windows, plus the style and shell checks on Linux:

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --release --locked
```

The style rules run with `python -B .github/lint/style_check.py`, the shell checks with `bash .github/lint/check-shell`, and the full gate with `bash .githooks/pre-push`.

### Commits

Subjects look like `type(scope): summary`. The commit-msg hook checks the format.
