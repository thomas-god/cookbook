# Break-glass administration CLI

- Status: Accepted
- Date: 2026-11-16

## Context

Authentication has no password-recovery flow because there is no email delivery. If the only
administrator forgets their password, the web application cannot recover the account, and every
administrative function becomes unavailable. The application also has no way to promote another user
without an existing administrator.

The database is SQLite and lives in a local data directory. Anyone with a shell in the application
container can already read and write to it, so an operator can repair an account with raw SQL. That
path is error prone: the password must be hashed with Argon2 in the expected PHC format, and role
changes must preserve the invariant that at least one administrator exists.

## Decisions

### Separate operator binary

A `cookbook-admin` binary is shipped next to `cookbook-app` and `cookbook-backup`. It supports:

- resetting an existing user's password,
- promoting a user to administrator,
- demoting an administrator to a regular user.

Users are selected with exactly one of `--id` or `--email`, and the command reads and writes the
SQLite database directly. The application can keep running; WAL mode allows the CLI to write while
the server reads.

### Shell access is the authorization boundary

The CLI does not authenticate. Possession of shell access to the container, or write access to the
data volume, is the authority to use it. No HTTP endpoint is added, so the tool is not reachable
over the network. Running it from a remote deployment is an explicit operator action rather than an
application feature.

This adds no privilege beyond what shell access already grants. It replaces hand-written SQL with
the same password hashing and role rules as the application.

### A separate port, not a forged administrator

The web layer's `AdminActor` is proof that the acting user was authenticated and has the admin role.
The CLI must not fabricate one. Instead, `UserMaintenanceService` is a separate inbound port whose
operations take only the target user and the desired change. The implementation shares its
password-reset and role-change helpers with `UserAdminService`, so the last-admin rule is identical
in both paths.

### Safeguards

- Demoting the last administrator is refused. The operator can promote another user first.
- The CLI refuses to run when `cookbook.sqlite3` does not exist, so a mistyped data directory cannot
  silently create an empty database.
- Interactive password entry is hidden and confirmed. Non-interactive entry uses `--password-stdin`.
  There is no command-line password flag, avoiding shell history and process list exposure.
- Password resets do not revoke existing sessions in this iteration.

## Consequences

### Benefits

- A forgotten admin password is recoverable without manual SQL.
- Password hashing and role invariants match the web application.
- The security boundary is explicit: shell access is required, and the web authorization model is
  unchanged.
- The CLI works while the application is running.

### Tradeoffs

- The tool is only usable by operators with container or filesystem access.
- There is no remote recovery flow and no email-based reset.
- Existing sessions remain valid after a password reset.
- The last-admin check is read-then-write, like the web path. Two concurrent demotions could
  theoretically race which is deemed acceptable for a single-operator emergency tool.
- The operator must run the CLI as a user that can write the database and its WAL files.

### When to revisit

- Add session revocation for password resets, or a dedicated "revoke sessions" command.
- Add an audit log if break-glass usage needs a durable trail beyond container logs.
- Make the last-admin check transactional if concurrent administrative writes become a concern.
