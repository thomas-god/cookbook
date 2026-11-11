# Recipe revision history

- Status: Accepted
- Date: 2026-11-09

## Context

Recipes are edited by any authenticated user allowed by the recipe's scope, not necessarily by the
original author. Authors can change a recipe's scope at any time. The app needs to keep a history of
recipe modifications with:

- the editor's user id (which may differ from the author),
- the modification time,
- an optional message, similar to a commit message.

The edit form should stay simple: the user edits the recipe, the browser posts the full updated
content, and the server builds the history. Editing must also detect stale forms so that one user
does not silently overwrite an edit made by someone else.

## Decisions

### Full snapshots instead of stored diffs

Each revision stores a complete `RecipeContent` snapshot. Diffs are not computed or persisted.
Historical content can be shown exactly as it was, and diff rendering can be added later as a
derived view without migrating stored data.

A diff-on-write design was rejected because it makes reconstruction fragile, complicates model
changes, and saves negligible space for a family cookbook. Duplicating unedited content accros
updates is deemed acceptable given the expected low volume of updates for a self-hosted application.

### Separate recipe identity from content

`Recipe` only holds stable identity and access metadata: `RecipeId`, `UserId` author, and
`RecipeScope`. The author never changes; the author alone can change the scope. Editable content
lives in `RecipeContent`.

`RecipeWithContent` pairs a `Recipe` with one of its revisions. Its constructor rejects a revision
whose `recipe_id` does not match the recipe. Repositories return it with the latest revision, so
callers never read content from a separate mutable field that could drift from stored history.

### Revisions

`RecipeRevision` holds:

- a revision id,
- the recipe id,
- a full `RecipeContent` snapshot,
- the editor user id,
- the edit time,
- an optional `RevisionMessage`.

Creation records the first revision, using the author as editor and no message. Revisions are
returned oldest first.

### Atomic persistence

The repository port saves the current recipe identity and appends the revision in one operation. A
recipe without revisions is treated as corrupt storage state.

The author is immutable: saving an existing recipe with a different author is rejected with
`SaveRecipeError::RecipeAuthorMismatch`. A revision whose recipe id does not match is rejected with
`SaveRecipeError::RecipeIdMismatch`.

### Stale edit detection

The edit form includes a hidden `latest_revision` field containing the latest revision id seen by
the editor. Update requests carry it as `latest_revision`. The service compares it with the current
latest revision and returns `UpdateRecipeError::Conflict` when they differ. A conflicting update is
not persisted.

## Consequences

### Benefits

- Every recipe always has a stored content snapshot.
- Current content is always derived from the latest revision.
- The author and editor are represented separately.
- Stale edits are rejected instead of silently overwriting newer content.
- History is append-only and independently readable.
- No diff format or migration is required.

### Tradeoffs

- Current content is resolved through the latest revision instead of being stored on `Recipe`.
- Listing and detail reads resolve the latest revision.
- Timestamp display currently uses server-local time.
- No-op edits still create a new revision. A TODO in `RecipeServiceImpl::update_recipe` marks the
  planned content comparison and skip.
- Duplication of the non-edited parts of a recipe's content accros updates.

### When to revisit

- Add no-op edit detection.
- Add display-time diffs or richer revision navigation.
- Introduce persistent storage with `recipes(id, author)` and
  `recipe_revisions(recipe_id, editor, edited_at, message, content)` tables.
- Add batch user lookup if history pages become large.
