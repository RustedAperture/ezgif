# Task 4 report: page-guard and route-composition slice

## Summary

Implemented the admin users route guard split by adding `RequireManagePermissions`, switched the users page to that guard, and added route-level tests that keep the page content inside the page-specific guards without re-wrapping the shared admin layout.

## Changed files

- `apps/web/components/require-manage-permissions.tsx`
- `apps/web/components/require-manage-permissions.test.tsx`
- `apps/web/app/admin/users/page.tsx`
- `apps/web/app/admin/users/page.test.tsx`
- `apps/web/app/admin/stats/page.test.tsx`

## Commands, outputs, and results

### Red: guard test before implementation

Command:

```bash
cd apps/web
npm test -- components/require-manage-permissions.test.tsx
```

Output:

```text
> web@0.2.10 test
> vitest run components/require-manage-permissions.test.tsx

RUN  v4.1.10 /Users/cameronvarley/projects/ezgif/.worktrees/admin-area-layout/apps/web

❯ components/require-manage-permissions.test.tsx (0 test)

FAIL  components/require-manage-permissions.test.tsx [ components/require-manage-permissions.test.tsx ]
Error: Failed to resolve import "@/components/require-manage-permissions" from "components/require-manage-permissions.test.tsx". Does the file exist?

Test Files  1 failed (1)
Tests       no tests
Duration    872ms
```

Result: expected failure because the new guard file did not exist yet.

### Red: users page route test before implementation

Command:

```bash
cd apps/web
npm test -- app/admin/users/page.test.tsx
```

Output:

```text
> web@0.2.10 test
> vitest run app/admin/users/page.test.tsx

RUN  v4.1.10 /Users/cameronvarley/projects/ezgif/.worktrees/admin-area-layout/apps/web

❯ app/admin/users/page.test.tsx (1 test | 1 failed)
× renders the page title, description, and table inside the manage-permissions guard without re-wrapping the shared layout

FAIL  app/admin/users/page.test.tsx > AdminUsersPage > renders the page title, description, and table inside the manage-permissions guard without re-wrapping the shared layout
Error: invariant expected app router to be mounted

Test Files  1 failed (1)
Tests       1 failed (1)
Duration    864ms
```

Result: expected failure because the page still used `RequireAdmin`.

### Green: focused guard test after implementation

Command:

```bash
cd apps/web
npm test -- components/require-manage-permissions.test.tsx
```

Output:

```text
Test Files  1 passed (1)
Tests       4 passed (4)
Duration    963ms
```

### Green: focused users page route test after implementation

Command:

```bash
cd apps/web
npm test -- app/admin/users/page.test.tsx
```

Output:

```text
Test Files  1 passed (1)
Tests       1 passed (1)
Duration    1.01s
```

### Green: exact affected web test command from the brief

Command:

```bash
cd apps/web
npm test -- components/require-manage-permissions.test.tsx components/require-admin-stats.test.tsx app/admin/stats/page.test.tsx app-shell.test.tsx admin-area-layout.test.tsx
```

Output:

```text
Test Files  5 passed (5)
Tests       16 passed (16)
Duration    1.30s
```

## Concerns

- No functional concerns from this slice.
- `apps/web/app/admin/stats/page.tsx` did not need a code change because the shared admin layout already owns the shell composition in this worktree state; the route-level test now asserts the page does not re-wrap that layout.

## Commit

- `2c7c3e4` — `refactor: guard admin users route composition`
