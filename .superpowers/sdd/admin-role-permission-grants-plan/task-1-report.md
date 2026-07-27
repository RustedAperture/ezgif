# Task 1 Report: Atomic promotion grants

## Outcome

Implemented server-side User → Admin promotion so that a successful promotion now:

- updates the stored role,
- grants all three known permissions:
  - `upload_local_images`
  - `view_admin_stats`
  - `manage_permissions`
- does the work in one transaction,
- remains idempotent on repeated promotion,
- preserves the existing Admin → User behavior of only changing the role and leaving permission rows intact,
- preserves the existing API security boundaries already enforced by the admin route.

## Files changed

- `apps/server/src/repositories/admin.rs`
- `apps/server/tests/admin_api.rs`

## TDD evidence

### RED

Focused test command:

```bash
cargo test promoting_a_user_to_admin_grants_all_permissions_idempotently --test admin_api
```

Observed failure:

- the test failed exactly where it checked the promoted user’s permissions
- the API returned `false` for `upload_local_images` instead of `true`

That confirmed the repository was updating the role but not granting the permission rows yet.

### GREEN

After the repository change, the same focused test passed:

```bash
cargo test promoting_a_user_to_admin_grants_all_permissions_idempotently --test admin_api
```

Then I ran the full admin API test file:

```bash
cargo test --test admin_api
```

Result: all 13 admin API tests passed.

## Implementation notes

The repository now handles role promotion like this:

- if the requested role is `admin`, it opens a transaction,
- updates the user’s role,
- inserts the three known permission rows with `INSERT OR IGNORE`,
- commits only after all inserts succeed,
- returns without granting permissions if the target user row does not exist.

For `user` demotion, it keeps the old behavior and only updates the role.

## Self-review

Checked the diff after the implementation and confirmed:

- the change is confined to the repository and admin API tests,
- the new promotion path is transactional,
- repeated promotion does not create duplicate permission rows,
- existing root-admin-only controls are untouched,
- existing non-root restrictions on `manage_permissions` are untouched,
- the existing admin API tests still pass.

## Concerns

None beyond the usual note that the repo still emits the pre-existing Rust future-incompatibility warning for `proc-macro-error2`, which did not affect this change.

## Fix-round appendix

### RED

Command:

```bash
cargo test promoting_a_user_to_admin_grants_all_permissions_idempotently --test admin_api
```

Relevant failure output:

```text
test promoting_a_user_to_admin_grants_all_permissions_idempotently ... FAILED

thread 'promoting_a_user_to_admin_grants_all_permissions_idempotently' panicked at apps/server/tests/admin_api.rs:352:5:
assertion `left == right` failed
  left: Bool(false)
 right: true
```

### GREEN

Command:

```bash
cargo test promoting_a_user_to_admin_grants_all_permissions_idempotently --test admin_api
```

Relevant passing output:

```text
test promoting_a_user_to_admin_grants_all_permissions_idempotently ... ok

test result: ok. 1 passed; 0 failed
```
