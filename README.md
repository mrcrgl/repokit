# repokit-cli

Apply and update repository templates safely with deterministic overlays, manifests, and merge-aware updates.

## Usage

### Initialize
Use a Git URL or a local filesystem path:

- Git URL:
  - `repokit init --template https://github.com/org/template.git --ref main --profiles base,service`
- Local path:
  - `repokit init --template /path/to/template --profiles base,service`

Notes:
- `init` is the only command that accepts `--template`, `--ref`, and `--profiles`.
- If `.kit.toml` already exists, `init` fails; use `repokit config set` instead.
- Local templates do **not** support `--ref`.
- `base` is always included first; duplicate profiles are ignored.

### Update
After init, `update` always uses `.kit.toml` (no CLI overrides):

- `repokit update`

### Status
After init, `status` always uses `.kit.toml` (no CLI overrides):

- `repokit status`

### Config management
Use `repokit config` to inspect and update `.kit.toml`:

- Show:
  - `repokit config show`
- Set:
  - `repokit config set --template https://github.com/org/template.git --ref main --profiles base,service`
- Clear ref:
  - `repokit config set --ref ""`

## Template layout

Templates are expected to follow this structure:

- `profiles/base` (required)
- `profiles/<profile>` (optional overlays)
- Optional merge rules (combined across selected profiles, later profiles win on conflicts):
  - `profiles/<profile>/.repokit/merge.toml`
- Optional GitHub labels per profile:
  - `profiles/<profile>/.repokit/github/labels.toml`

The `.repokit/` directory configures `repokit` behavior only and is not copied into the target repo.

### Example merge rules (`merge.toml`)

```toml
[[rule]]
name = "gitignore"
strategy = "gitignore"
patterns = [".gitignore"]

[[rule]]
name = "docs"
strategy = "markdown_block"
patterns = ["README.md", "docs/**/*.md"]
```

## Behavior summary

- Writes `.kit.toml` with template metadata and file hashes.
- Updates overwrite only if unchanged; otherwise writes `.new`.
- Merge-aware files (e.g. `.gitignore`, Markdown blocks) are merged without `.new`.
- GitHub labels are additive and idempotent (no deletions).
- If no GitHub auth is available, label sync is skipped with a warning.

## Requirements

- `git` for Git URL templates
- `gh` or `GITHUB_TOKEN` for GitHub label operations
