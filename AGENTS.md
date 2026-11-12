# repoke

## Rules

- Commits use Conventional Commits, in English, with the personal git email (paveldelpozo@gmail.com). `scripts/release.sh` reads the prefixes to pick the semver bump: `feat:` is minor, `fix:` and other types are patch, `!` or `BREAKING CHANGE` is major.
- Releases follow semantic versioning and are made locally with `scripts/release.sh` (try `--dry-run` first; `--crate` also publishes to crates.io).
- Every new feature updates the command docs in both languages (`docs/commands.md` and `docs/es/commands.md`) and the landing in `site/`, which is available in English and Spanish. `scripts/check-docs.py` fails if a command or option is missing from either reference.
- When updating the landing, record the latest published version in the repository. The landing reads it from `Cargo.toml` at build time, so a release keeps it current.
- After each change that passes verification, reinstall the local binary with `cargo install --path . --force`.
- Before merging or integrating any branch or worktree, read its status and QA in its task note under `~/.hivefold/brain/Proyectos/repoke/tareas/` (the `hvf:` block), which you manage from the project manager in the hvf web app. State it explicitly, and never merge if the last QA is FAIL or pending.

## How to verify

- `scripts/ci.sh` runs everything CI checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (host, Linux and Windows targets), `cargo test`, the docs check and the landing build. `--quick` skips the Linux tests in Docker.
- `cargo test` alone for a fast loop; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` for style.
- The landing in `site/` builds with `pnpm install --frozen-lockfile && pnpm exec astro build`.
- `.githooks/pre-push` runs `scripts/ci.sh --quick`; enable it with `git config core.hooksPath .githooks`.
