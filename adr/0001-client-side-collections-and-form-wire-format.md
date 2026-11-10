# Client-side collection state, forma data and wire format

- Status: Accepted
- Date: 2026-11-09

## Context

The recipe creation form contains variable-length collections: ingredients and recipe's steps. Each
row has its own fields, and users must be able to add rows and remove a specific row within the
collection.

The form is implemented with Topcoat's runtime:

- form values are held in runtime signals,
- inputs are bound with `:value` and `@input`,
- the form is a shard, so adding a row rerenders only the form,
- the form is submitted as a native `application/x-www-form-urlencoded` POST.

Topcoat's runtime expression language is intentionally limited: it does not support mutating a `Vec`
with operations such as `push`, `remove`, or `retain`. A signal holding a row collection cannot be
updated directly from a runtime event handler.

A native HTML form also cannot submit `application/json`. Building JSON on the client would require
a separate JavaScript serializer, moving the form contract from the server parser into client code.
The endpoint is currently internal to the browser UI and is not a public API. Introducing JSON
solely for the form would add a client-side serialization layer without providing an integration
benefit.

## Decision

### Client-side collection state

We use a tombstone pattern to emulate removing an item from a collection. Each ingredient and step
row owns a `removed: Signal<bool>`. Removing a row sets that signal to `true`. The row's
`<fieldset>` binds both `hidden` and `disabled` to the signal:

```text
:hidden=$(removed.get())
:disabled=$(removed.get())
```

Disabled controls are not included in a native form submission, so removed rows are automatically
excluded from the POST payload.

Rows are not compacted on the client. Adding a row increments a count signal; removed rows remain as
tombstones until the page is left. There is no hardcoded maximum row count.

### Form wire format

The URL-encoded wire format uses structural row keys rather than repeated flat field names:

```text
ingredients[<row>][name]
ingredients[<row>][amount_type]   # "measured" or "text"; missing means "measured"
ingredients[<row>][quantity]      # measured mode
ingredients[<row>][unit]          # measured mode
ingredients[<row>][unit_other]    # measured mode, when unit = "other"
ingredients[<row>][quantity_text] # text mode

steps[<row>][title]
steps[<row>][instruction]
steps[<row>][ingredients]
```

Top-level fields remain (e.g. `title`, `preparation`, etc.).

Field names are generated through shared helpers (`IngredientField::key` and `StepField::key`) so
the form and parser use the same definitions.

### Server-side parsing

The server parses the flat URL-encoded form-data pairs into typed intermediate form types:

- `RecipeFormData`
- `IngredientFormData`
- `StepFormData`

Rows are collected into `BTreeMap<usize, ...>`, which handles sparse row IDs left by removed rows
and preserves step ordering by row ID.

The parser rejects:

- unknown fields,
- malformed indexed keys,
- duplicate fields,
- missing required values,
- invalid amount modes,
- invalid quantities and units,
- empty or overlong custom unit labels,
- missing free-text amounts.

The typed form data is then converted into `CreateRecipeRequest`. Missing `amount_type` values are
treated as `measured`, so older form payloads remain valid.

No client-side JSON serializer and no JSON endpoint are introduced.

## Consequences

### Benefits

- Removing any item from a collection without a server request.
- Removed items are excluded from submission through native browser behavior.
- The form keeps native submission and works with the existing server pipeline.
- The wire format is explicit and structural instead of relying on field order or marker fields.
- The view and parser share the same field-name definitions.
- No new dependency or client serialization layer is required.

### Tradeoffs

- Removed rows remain in the DOM and in the row count as tombstones.
- Row slots are not reclaimed: adding rows after many removals leaves hidden rows in the page
  (hidden DOM nodes and signal state).
- The parser is still hand-written, though it now produces typed form data and validates the
  contract explicitly.
- The form/parser contract is enforced by shared helpers and tests, not by a serde-derived schema.
- Ingredient rows carry an explicit amount mode, so the form has one more control per row.
- Step ingredient quantities remain numeric; free-text quantities are not represented there yet.
- The URL-encoded format remains internal to this UI and is not suitable as a public API contract.

### When to revisit

Reconsider this decision if:

- the endpoint becomes a public or machine-consumed API,
- rows need to be reordered or compacted after removal,
- row count becomes large enough that tombstoned DOM/signal state matters,
- we need to reclaim removed slots,
- or we want a serde-derived request contract.

A likely migration path would be stable row IDs plus a server-managed row-ID collection, with
add/remove operations exposed as procedures. A JSON API could be added later alongside the existing
form endpoint if external consumers appear.
