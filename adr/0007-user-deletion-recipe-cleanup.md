# User deletion and recipe cleanup

- Status: Accepted
- Date: 2026-11-16

## Context

Deleting a user only removes it from the `users` table. Recipe rows reference their author by id
without a foreign key, so recipes survive user deletion. This is intentional for public recipes,
which remain useful to the household. Private recipes, however, are only visible to their author so
once the author is gone they can never be viewed or edited again, and their rows, revisions, and
asset files become unreachable storage.

## Decision

The auth service remains unaware of recipes. A background task in the recipe service periodically
finds private recipes whose author is missing and deletes them:

- the repository query joins `recipes` to `users` and selects `scope = 'private'` rows with no
  matching user,
- the recipe service deletes each recipe row, which cascades to revisions, image metadata, and
  source-document metadata,
- the document repository then removes the recipe's asset directory,
- asset cleanup failures are logged; the database deletion is not rolled back.

The sweep runs once at startup, which also repairs orphans created before this feature existed, and
then hourly. It is idempotent. Public recipes with a missing author are left untouched.

## Consequences

### Benefits

- No coupling between the auth and recipe services.
- Existing and future orphans are handled by the same path.
- Delayed deletion has no user-visible effect because private recipes are already invisible when
  their author no longer exists.

### Tradeoffs

- Cleanup is eventual rather than part of the user deletion transaction.
- A crash between row deletion and asset deletion can leave an orphan directory, which is logged and
  harmless.
- The repository query spans recipes and users because the recipe domain cannot determine user
  existence on its own.

### When to revisit

- If cleanup must be immediate, publish a durable user-deletion event or move the operation into the
  user deletion flow.
- Add a filesystem sweep for asset directories without a database row.
- Apply the same approach to orphaned sessions if they become a concern.
