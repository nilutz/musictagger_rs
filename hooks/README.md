# Git Hooks

This directory contains Git hooks for the project.

## Pre-commit Hook

The pre-commit hook automatically runs `cargo fmt` before each commit to ensure code is properly formatted.

### Installation

The hook is already installed if you're the repository owner. For other contributors:

```bash
# From the repository root
cp hooks/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

Or create a symlink (recommended):

```bash
# From the repository root
ln -sf ../../hooks/pre-commit .git/hooks/pre-commit
```

### What it does

1. Checks if code is formatted (`cargo fmt --check`)
2. If not formatted, runs `cargo fmt` to format the code
3. Re-stages the formatted files automatically
4. Proceeds with the commit

### Bypassing the hook

If you need to commit without running the hook (not recommended):

```bash
git commit --no-verify
```
