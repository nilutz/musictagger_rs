# Release Scripts

This directory contains scripts to help with the release process.

## release.sh

Automates the version release process by:
1. Updating the version in `Cargo.toml`
2. Updating `Cargo.lock`
3. Creating a git commit
4. Creating a git tag
5. Optionally pushing to GitHub to trigger the release workflow

### Usage

```bash
# Increment patch version (0.1.13 -> 0.1.14)
./scripts/release.sh patch

# Increment minor version (0.1.13 -> 0.2.0)
./scripts/release.sh minor

# Increment major version (0.1.13 -> 1.0.0)
./scripts/release.sh major

# Set a specific version
./scripts/release.sh 1.0.0
```

### What it does

1. **Shows current version** and suggests next versions
2. **Validates** that you're on a clean working tree
3. **Updates** Cargo.toml with the new version
4. **Updates** Cargo.lock by running a build
5. **Shows diff** and asks for confirmation
6. **Commits** the version change
7. **Creates** a git tag `v<version>`
8. **Asks** if you want to push immediately

### Example

```bash
$ ./scripts/release.sh patch
Current version: 0.1.13

New version: 0.1.14

Updating Cargo.toml...
Updating Cargo.lock...

Changes:
diff --git a/Cargo.toml b/Cargo.toml
index 1234567..abcdefg 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -1,7 +1,7 @@
 [package]
 name = "musictagger_rs"
-version = "0.1.13"
+version = "0.1.14"

Proceed with release v0.1.14? (y/N) y
Creating release commit...
Creating git tag v0.1.14...

✓ Release prepared successfully!

Next steps:
  1. Push the commit:  git push
  2. Push the tag:     git push origin v0.1.14

Push now? (y/N) y
Pushing to GitHub...
✓ Release v0.1.14 pushed!
```

### GitHub Actions

Pushing the tag will trigger the `.github/workflows/release.yml` workflow, which will:
- Build binaries for all platforms (Linux x86_64/aarch64/armv7, macOS x86_64/arm64)
- Create a GitHub release
- Upload the binaries as release assets

### Safety Features

- ✅ Validates version format (semantic versioning)
- ✅ Warns if you have uncommitted changes
- ✅ Warns if you're not on the main branch
- ✅ Shows diff before committing
- ✅ Asks for confirmation before pushing
- ✅ Can abort at any step
