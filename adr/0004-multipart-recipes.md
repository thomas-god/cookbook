# Multipart recipes

- Status: Accepted
- Date: 2026-11-12

## Context

Some recipes, especially in pastry, are assembled from several sub-recipes. Flattening their
ingredients and steps into a single list mixes unrelated instructions and produces a long recipe
that is hard to follow.

A multipart recipe needs:

- a title, description, servings and category for the whole recipe,
- the recipe total duration derived from the parts,
- one or more titled parts,
- each part with its own description, durations, ingredients, images and steps,
- switching between simple and multipart recipes while editing.

## Decisions

### Recipe structure

`RecipeContent` holds recipe-wide metadata and a `RecipeStructure`:

```text
RecipeContent
  title
  description
  servings
  category
  structure
    Simple(RecipeBody)
    Multipart(Vec<RecipePart>)
```

`RecipeBody` is the content shared by a simple recipe and a sub-recipe:

```text
RecipeBody
  durations
  ingredients
  images
  steps
```

`RecipePart` adds an id, a required title and an optional description to a `RecipeBody`.

### Validity

Every `RecipeContent` represents a valid recipe. Construction goes through fallible constructors:

- `new_simple`
- `new_multipart`

They reject an empty recipe title, multipart content without parts, blank part titles, and duplicate
part ids. There is no separate validation pass, and no code path constructs an invalid value
directly.

### Ingredients

Ingredients are an ordered `Vec<Ingredient>` rather than a map keyed by name. This lets a recipe
have conflicting quantities for the same ingredient (e.g. when converting a multipart recipe into a
simple recipe).

`RecipeBody::new` normalizes ingredients based on a few heuristics:

- same name and same measured unit: sum (e.g. "100g of flour" + "200g of flour" = "300g of flour"),
- identical quantity: de-duplicate,
- otherwise: keep every entry (e.g. "100g of flour" and "2tbsp of flour" are not merged).

A duplicate ingredient name after normalization appears as a conflict and is left to the user to
resolve by editing the recipe. Conflicts do not block the conversion or edition of a recipe (but are
explicitly displayed to users).

### Conversions

Simple to multipart wraps the current body in the first part of the new recipe and uses the recipe
title as the part title.

Multipart to simple concatenates part bodies in order. Ingredients are normalized, images are
de-duplicated, steps are concatenated, and durations are summed. Part descriptions are appended to
the recipe description as `Part title: description` blocks. Remaining ingredient conflicts stay in
the content so the user can resolve them while editing.

### Editing and wire format

Simple forms keep their flat fields. Multipart forms nest part fields:

```text
parts[0][id]
parts[0][title]
parts[0][description]
parts[0][preparation]
parts[0][images]
parts[0][ingredients][0][name]
parts[0][steps][0][instruction]
```

Part rows use stable slot keys for reordering, like step rows. Type switching is an explicit action
on the edit page, posting the current revision to a conversion endpoint. The form itself edits
exactly one structure.

## Consequences

### Benefits

- Sub-recipes keep their own ingredients, steps, images and timings.
- The recipe total duration cannot drift from the parts.
- Conflict information is visible instead of being rejected or silently lost.
- Simple and multipart recipes share one body model and conversion path.

### Tradeoffs

- The form wire format is nested for multipart recipes.
- Type switching creates a revision before the user continues editing.
- The edit-page conflict warning reflects loaded content and clears only after a save; it is not
  updated live while typing.
