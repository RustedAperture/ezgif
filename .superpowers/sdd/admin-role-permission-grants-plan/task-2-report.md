# Task 2 report: Confirmed role changes in the admin table

Date: 2026-07-27
Worktree: `/Users/cameronvarley/projects/ezgif/.worktrees/admin-role-permission-grants`
Task 1 server commit referenced by brief: `0e6be73`
Task 2 starting HEAD: `f97ad88`

## Summary

Implemented a confirmation flow for admin-table role changes in `apps/web/components/admin-users-table.tsx` and added focused UI coverage in `apps/web/components/admin-users-table.test.tsx`.

The table now:

- opens a confirmation dialog before a root admin role change request is sent,
- names the target user and old/new roles in the copy,
- explicitly warns that `User → Admin` grants all three permissions,
- calls only the existing role endpoint on confirmation,
- updates local row state to `admin` plus all three permissions on successful `User → Admin`,
- preserves permission booleans on successful `Admin → User`,
- restores the full pre-confirmation row snapshot on failure,
- keeps the existing root-admin gate for role control and the existing non-root restriction on the `manage_permissions` checkbox.

## TDD evidence

### RED

Command:

`npm test -- admin-users-table.test.tsx`

Relevant failing output before production changes:

```text
❯ components/admin-users-table.test.tsx (8 tests | 4 failed)
× opens confirmation before changing a role and does not call the API immediately
× restores the original role and permissions when the confirmation is cancelled
× confirms user to admin through the role endpoint and synchronizes all permissions locally
× restores the previous role and permissions when the confirmed role request fails

FAIL ... Unable to find an element with the text: Confirm role change.
FAIL ... Unable to find role="button" and name "Cancel"
FAIL ... Unable to find role="button" and name "Confirm"
```

### GREEN

Command:

`npm test -- admin-users-table.test.tsx`

Relevant passing output after implementation:

```text
Test Files  1 passed (1)
Tests  8 passed (8)
```

### Full web verification

Command:

`npm test`

Relevant passing output:

```text
Test Files  2 passed (2)
Tests  9 passed (9)
```

## Files changed

- `apps/web/components/admin-users-table.tsx`
- `apps/web/components/admin-users-table.test.tsx`

## Implementation notes

- Added `pendingRoleChange` state carrying the requested role plus a full snapshot of the pre-confirmation row.
- Switched the role selector from immediate mutation to dialog-driven confirmation.
- Used the existing shadcn `AlertDialog` components required by the brief.
- Kept permission checkbox behavior unchanged outside the synchronized local updates after successful role confirmation.
- Avoided any extra permission API calls for `User → Admin`; only `/api/admin/users/:id/role` is called.

## Self-review

Checked the final diff against the brief and confirmed:

- only the requested table and test files changed,
- the confirmation copy includes user identity plus old/new role information,
- `User → Admin` grants all three permission booleans locally after a successful role response,
- `Admin → User` only changes the role locally,
- failed role requests restore the complete original row state and show the existing role error toast,
- root-admin-only role access and `manage_permissions` checkbox restrictions remain intact.

## Concerns

- The focused tests mock the select and alert-dialog shells to make the state-transition assertions deterministic; the production component still uses the real shared UI primitives.
