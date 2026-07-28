# Task 4 Final Audit Report

Date: July 28, 2026
Worktree: `/Users/cameronvarley/projects/ezgif/.worktrees/security-dependency-updates`
Branch: `codex/security-dependency-updates`

## Scope

Validated the reviewed dependency-update commits already present on the branch:

- `c16e8c9` — web dependency updates
- `3c6aacb` — Rust dependency updates

No tracked files were modified during this audit. This report file was created as requested.

## Commands Run

### Final audits

```bash
cd apps/web && npm audit --json
cargo audit --json
```

### Repository hygiene

```bash
git status --short --branch
git diff --check
git log --oneline -3
```

### Dependency-path follow-up

```bash
cd apps/web && npm ls eslint @eslint/config-array @eslint/eslintrc minimatch brace-expansion eslint-config-next eslint-plugin-import eslint-plugin-jsx-a11y eslint-plugin-react
cargo tree -i anyhow --workspace
cargo tree -i proc-macro-error2 --workspace
cargo tree -i spin@0.9.8 --workspace
rg -n "downcast_mut" apps/server Cargo.toml Cargo.lock
```

## Results

### 1) Web audit (`apps/web`)

`npm audit --json` still reports 9 high-severity findings.

Summary:

- Total vulnerabilities: 9 high
- Production-app packages implicated: none
- Remaining findings are all in the ESLint / Next lint toolchain
- Fixes offered by npm require semver-major or clearly incorrect downgrade suggestions, so there is no clean in-branch auto-fix from this audit output

Affected dependency chain confirmed by `npm ls`:

- direct dev dependencies:
  - `eslint@9.39.5`
  - `eslint-config-next@16.2.12`
- transitive path:
  - `eslint` -> `@eslint/config-array@0.21.2` -> `minimatch@3.1.5` -> `brace-expansion@1.1.16`
  - `eslint` -> `@eslint/eslintrc@3.3.6` -> `minimatch@3.1.5` -> `brace-expansion@1.1.16`
  - `eslint-config-next` -> `eslint-plugin-import@2.32.0` -> `minimatch@3.1.5` -> `brace-expansion@1.1.16`
  - `eslint-config-next` -> `eslint-plugin-jsx-a11y@6.10.2` -> `minimatch@3.1.5` -> `brace-expansion@1.1.16`
  - `eslint-config-next` -> `eslint-plugin-react@7.37.5` -> `minimatch@3.1.5` -> `brace-expansion@1.1.16`

Important nuance:

- A patched `minimatch@10.2.5` / `brace-expansion@5.0.8` chain is already present elsewhere in the tree, including under `@typescript-eslint/typescript-estree` and `shadcn` dependencies, so the remaining audit findings are specifically the older lint-stack branches above.
- The vulnerable packages are development-time linting dependencies, not runtime web-serving dependencies.

Upstream remediation status:

- `npm audit` only offers a major move to `eslint@10.8.0` for several findings.
- For `eslint-config-next`, npm reports an obviously incompatible `0.2.4` suggestion, which should not be treated as a valid remediation plan.
- Practical remediation depends on upstream ecosystem compatibility between Next 16.x, `eslint-config-next`, and an ESLint release line that no longer resolves to the vulnerable `minimatch@3` chain.

Assessment:

- The web dependency-update commit improved the dependency state but did **not** clear all audit findings.
- Remaining findings are real audit findings, but they are limited to dev tooling and appear upstream-constrained rather than fixable by a safe lockfile-only bump on this branch.

### 2) Rust audit (`apps/server` workspace)

`cargo audit --json` reports:

- Vulnerabilities found: 0
- Unmaintained warnings: 1
- Unsound warnings: 1
- Yanked crates: 1

#### 2a) Unsound warning

- Advisory: `RUSTSEC-2026-0190`
- Package: `anyhow v1.0.102`
- Patched version range: `>=1.0.103`

Dependency path:

- `memebucket-server` -> `anyhow v1.0.102`

Repository usage check:

- `rg -n "downcast_mut" apps/server Cargo.toml Cargo.lock` returned no matches.

Interpretation:

- The advisory is tied to `Error::downcast_mut()`.
- I did not find in-repo usage of `downcast_mut`, which lowers immediate practical risk, but the crate version is still below the patched release and remains audit-visible until updated.

#### 2b) Unmaintained warning

- Advisory: `RUSTSEC-2026-0173`
- Package: `proc-macro-error2 v2.0.1`

Dependency path:

- `memebucket-server` -> `validator v0.19.0` -> `validator_derive v0.19.0` -> `proc-macro-error2 v2.0.1`

Interpretation:

- This is a maintenance warning, not a vulnerability finding.
- Remediation depends on upstream movement in the `validator` toolchain or a local migration away from that derive stack.

#### 2c) Yanked crate

- Package: `spin v0.9.8`

Dependency paths:

- `memebucket-server` -> `sqlx v0.9.0` -> `sqlx-sqlite v0.9.0` -> `flume v0.12.0` -> `spin v0.9.8`
- `memebucket-server` -> `axum v0.8.9` -> `multer v3.1.0` -> `spin v0.9.8`
- `memebucket-server` -> `tower_governor v0.8.0` -> `axum v0.8.9` -> `multer v3.1.0` -> `spin v0.9.8`

Interpretation:

- This is a yanked-package warning, not a currently listed vulnerability in the audit output.
- Clearing it requires upstream dependency movement rather than a report-only change.

Assessment:

- The Rust dependency-update commit cleared vulnerability findings.
- The remaining Rust issues are warnings/advisories that should be tracked, but they are not recorded as active vulnerability hits in this audit run.

## Repository hygiene

`git status --short --branch`

- Output showed only `## codex/security-dependency-updates` before this report file was created.

`git diff --check`

- Clean; no whitespace or patch-format issues.

`git log --oneline -3`

- `3c6aacb fix: update vulnerable rust dependencies`
- `c16e8c9 fix: update vulnerable web dependencies`
- `b233fba fix: search admin users by raw discord id`

Interpretation:

- The two reviewed dependency-update commits are the latest intended commits on the branch.
- No `.env`, changelog, or unrelated tracked-file edits were present during the audit.

## Compatibility edits observed

- Web package versions currently resolve to:
  - `next 16.2.12`
  - `react 19.2.4`
  - `eslint 9.39.5`
  - `eslint-config-next 16.2.12`
- Rust workspace currently resolves to:
  - `sqlx 0.9.0`
  - `axum 0.8.9`
  - `anyhow 1.0.102`

No additional compatibility edits were made during Task 4.

## Merge choice recommendation

Recommended choice: **keep branch / do not merge yet**

Reason:

- Rust is in materially better shape and has zero vulnerability findings in `cargo audit`.
- Web still has 9 high `npm audit` findings in the lint stack, so the branch does not yet satisfy an “all fixable findings are gone” standard.
- The remaining web findings look dev-only and upstream-constrained, but they are still present and should be explicitly accepted, deferred, or remediated before merge.

If you want to move forward anyway, the safest framing would be:

1. keep the branch open,
2. decide whether dev-only ESLint findings are acceptable risk for this release window,
3. if not acceptable, perform a focused follow-up on the Next/Eslint toolchain rather than merging this branch as final.

## Handoff summary

- Web dependency commit improved the dependency set but left 9 high dev-tooling findings.
- Rust dependency commit cleared vulnerability findings, leaving one unsound warning (`anyhow`), one unmaintained warning (`proc-macro-error2` via `validator`), and one yanked crate (`spin v0.9.8`) through upstream paths.
- Repository hygiene checks passed.

## Final review fix — July 28, 2026

Addressed the final review finding without widening scope:

- removed the stale RustSec ignore comments and `--ignore` flags from `.github/workflows/ci.yml`, so CI now runs plain `cargo audit`
- updated `anyhow` in `Cargo.lock` from `1.0.102` to `1.0.103`
- left the 9 acknowledged npm dev-tool advisories untouched
- did not modify changelog files, version strings, `.env`, or application code

Verification run after the fix:

- `cargo audit` — passed with 0 vulnerability findings; remaining warnings are `proc-macro-error2` unmaintained (`RUSTSEC-2026-0173`) and yanked `spin v0.9.8`
- `cargo fmt --all --check` — passed
- `cargo check -p memebucket-server` — passed
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — passed
- `cargo test -p memebucket-server --test admin_api` — passed (17/17)
- `cargo test -p memebucket-server` — passed
- `cargo build -p memebucket-server` — passed

CI YAML diff verified:

- the audit step changed only from the ignored form to plain `cargo audit`
