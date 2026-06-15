# Contributing

## Local hooks

Install [pre-commit](https://pre-commit.com/) and enable the repository hooks:

```sh
pip install pre-commit
pre-commit install
```

Hooks run on each commit and enforce:

- `cargo fmt --all -- --check`
- `cargo clippy -- -D warnings`

Run all hooks manually:

```sh
pre-commit run --all-files
```

## ABI snapshots

After changing contract interfaces, regenerate and verify ABI metadata from the `COMEBACKHERE/` repo:

```sh
# In COMEBACKHERE/
make update-abi-snapshots
# or
just snapshot
```
