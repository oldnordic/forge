# Publishing ForgeKit

Guide for publishing ForgeKit to GitHub and crates.io.

## Prerequisites

- GitHub account with access to `oldnordic` organization
- crates.io account with publish permissions
- Local repository at `/home/feanor/Projects/forge`

## Step 1: Create GitHub Repository

### Option A: Web Interface

1. Go to: https://github.com/new
2. Fill in:
   - **Owner:** oldnordic
   - **Repository name:** forge
   - **Description:** Deterministic code intelligence SDK with dual backend support
   - **Visibility:** Public
   - **Initialize:** ❌ **DO NOT** check "Initialize this repository with a README"
3. Click "Create repository"

### Option B: GitHub CLI

```bash
gh repo create oldnordic/forge \
  --public \
  --description "Deterministic code intelligence SDK with dual backend support" \
  --source=/home/feanor/Projects/forge \
  --remote=origin \
  --push
```

## Step 2: Push Local Repository

If you used Option A (web), push manually:

```bash
cd /home/feanor/Projects/forge

# Add remote (if not already set)
git remote add origin git@github.com:oldnordic/forge.git

# Push to GitHub
git branch -M master
git push -u origin master
```

## Step 3: Verify GitHub Repository

Check: https://github.com/oldnordic/forge

Should contain:
- [ ] README.md
- [ ] LICENSE.md (GPL-3.0)
- [ ] CHANGELOG.md
- [ ] docs/ directory with all documentation
- [ ] forge_core/ crate
- [ ] forge_runtime/ crate
- [ ] forge_agent/ crate

## Step 4: Publish to crates.io

### Prerequisites

1. Login to crates.io:
   ```bash
   cargo login
   # Enter API token from https://crates.io/settings/tokens
   ```

2. Check you can publish:
   ```bash
   cargo owner --list -p forge-core
   ```

### Publication Order

Publish in dependency order:

#### 1. forge_core

```bash
cd /home/feanor/Projects/forge/forge_core

# Dry run first
cargo publish --dry-run

# If successful, publish
cargo publish --allow-dirty

# Wait for availability
sleep 60
```

#### 2. forge_runtime

```bash
cd /home/feanor/Projects/forge/forge_runtime

# Update dependency to use published version first:
# In Cargo.toml, change:
# forge_core = { path = "../forge_core", ... }
# to:
# forge_core = { version = "0.2.0", ... }

# Dry run
cargo publish --dry-run

# Publish
cargo publish --allow-dirty

# Wait
sleep 60
```

#### 3. forge_agent

```bash
cd /home/feanor/Projects/forge/forge_agent

# Update dependency:
# forge_core = { version = "0.2.0", ... }

# Dry run
cargo publish --dry-run

# Publish
cargo publish --allow-dirty
```

### Verify crates.io

Check each crate:

- https://crates.io/crates/forge-core
- https://crates.io/crates/forge-runtime
- https://crates.io/crates/forge-agent

## Step 5: Post-Publication

### Update Dependencies

After all crates are published, update to use versioned dependencies:

```toml
# forge_runtime/Cargo.toml
[dependencies]
forge_core = { version = "0.2.0", default-features = false }

# forge_agent/Cargo.toml
[dependencies]
forge_core = { version = "0.2.0", default-features = false }
```

### Tag Release

```bash
cd /home/feanor/Projects/forge
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

### Create GitHub Release

1. Go to: https://github.com/oldnordic/forge/releases/new
2. Choose tag: v0.2.0
3. Title: "ForgeKit v0.2.0"
4. Description: Copy from CHANGELOG.md
5. Publish release

## Troubleshooting

### "Repository not found" on push

Repository doesn't exist on GitHub yet. Create it first (Step 1).

### "crate already exists" on publish

- Check if you have ownership: `cargo owner --list -p forge-core`
- Contact existing owners if needed

### "failed to verify package"

```bash
# Check what files are included
cargo package --list -p forge_core

# Make sure all files are committed
git add -A
git commit -m "Prepare for publish"
```

### Dependency version conflicts

```bash
# Update all dependencies
cargo update

# Check tree
cargo tree -p forge_core
```

## Checklist

Before publishing:

- [ ] Version bumped in all Cargo.toml
- [ ] CHANGELOG.md updated
- [ ] All tests pass: `cargo test --workspace --all-features`
- [ ] Documentation builds: `cargo doc --no-deps`
- [ ] GitHub repository created
- [ ] Code pushed to GitHub
- [ ] crates.io login working
- [ ] LICENSE.md present
- [ ] README.md present
- [ ] All docs/ included

## Verification Commands

```bash
# Test everything
cargo test --workspace --all-features

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Build docs
cargo doc --no-deps

# Dry run publish
cd forge_core && cargo publish --dry-run
cd ../forge_runtime && cargo publish --dry-run
cd ../forge_agent && cargo publish --dry-run
```

## Support

For issues:
- GitHub: https://github.com/oldnordic/forge/issues
- crates.io: https://crates.io/crates/forge-core