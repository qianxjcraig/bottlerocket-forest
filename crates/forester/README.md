# Forester

Generic forest management for multi-repo projects.

A forest is a collection of related git repositories that are developed together but maintained as separate repos (not submodules). Forester provides:

- Forest configuration via `forester.toml`
- Coordinated grove management across all member repos
- Integration with crumbly for semantic search

## Installation

```bash
cargo install forester
```

## Usage

### Seed a Forest

Clone all member repositories and set up the forest:

```bash
forester seed
forester seed --verbose
```

### Sync Seed

Fetch latest changes from upstream remotes into local bare repositories:

```bash
forester sync-seed
forester sync-seed --verbose
```

This updates the bare repos without affecting existing groves.
New groves will use the updated refs.

### Update a Grove

`sync-seed` deliberately stops at the bare repos, so an existing grove stays where
it was. `grove update` is the other half — it advances a grove's checkouts onto
what was fetched:

```bash
forester grove update develop              # advance one grove
forester update develop                    # sync-seed, then advance
forester grove update                      # defaults to the current grove
```

Updating is offline: a grove member's `origin` is the forest's bare repo, so the
network hop happens once, in `sync-seed`.

Each member is classified before anything is touched, and the update refuses
rather than guesses:

| Checkout state | Result |
|---|---|
| clean, behind | fast-forwarded |
| clean, current | left alone |
| uncommitted changes | left alone, files reported |
| local commits and behind | refused under `--ff-only`; replayed under `--strategy rebase` |
| detached HEAD | left alone |
| member missing from the grove | cloned, then advanced |

Every refusal prints the command that resolves it. Nothing is stashed implicitly,
`git pull` is never used, and a conflicted rebase is rolled back rather than left
in progress.

```bash
forester grove update develop --dry-run              # classify, write nothing
forester grove update develop --member twoliter      # restrict to one member
forester grove update develop --strategy rebase      # replay local commits
```

To re-index after an update, add the `post-grove-update` trigger to a hook:

```toml
[[hook]]
name = "crumbly"
command = "update-context"
triggers = ["post-grove-create", "post-grove-update"]
```

### Manage Groves

Create a new forest grove (clones every member repo into it):

```bash
forester grove create feature-x
forester grove create feature-x --branch my-branch
```

List existing groves:

```bash
forester grove list
```

Remove a grove:

```bash
forester grove remove feature-x
forester grove remove feature-x --force
```

## Configuration

### forester.toml

Defines the forest members:

```toml
[forest]
name = "my-project"

[[member]]
name = "main-repo"
remote = "git@github.com:org/main-repo.git"
path = "main-repo"
default_branch = "main"

[[member]]
name = "lib-repo"
remote = "git@github.com:org/lib-repo.git"
path = "libs/lib-repo"
default_branch = "develop"
```

### crumbly.toml

Defines what to index for semantic search. See [crumbly documentation](../crumbly-cli/README.md).

## Directory Structure

After seeding:

```
my-forest/
  forester.toml
  crumbly.toml
  .forest/
    bare/                     # Bare clones of all member repos
      main-repo.git/
      lib-repo.git/
  
  # Main grove
  main-repo/
  libs/lib-repo/
  .crumbly/                    # Sembly index

  groves/
    feature-x/
      main-repo/
      libs/lib-repo/
      .crumbly/
```
