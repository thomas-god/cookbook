# Reorderable and splittable recipe steps

- Status: Accepted
- Date: 2026-11-11

## Context

Recipe steps could only be appended to the end of the step list. Removing a step
used a tombstone, so removing or adding a step never changed the order of the
remaining rows.

Users commonly need to insert an intermediate step, move an existing step, or
split one step into two smaller ones. Splitting must duplicate the step's text
and its existing image references.

The form is a Topcoat shard. Its runtime expression language does not support
mutating a `Vec` with operations such as `insert`, `remove`, or `swap`.
ADR 0001 already anticipated this change: stable row IDs plus add/remove
operations exposed as procedures.

## Decision

### Stable step slots

Each step row has a stable slot key. The keys are allocated monotonically and
never reused. The form keeps two pieces of collected state:

- `step_order`: the slot keys in display order,
- `next_step_key`: the next slot key to allocate.

Rows are keyed by slot key, not by display position. Moving or inserting a row
therefore preserves the row's form signals and any text the user has typed.

The wire format does not change: field names still use
`steps[<display-position>][...]`. The shard renders rows in `step_order`, so the
display positions in the submitted form follow the visible order. The server
parser continues to sort rows by their numeric row key.

### Step operations

Order changes are exposed as small, pure procedures:

- insert a slot at a display position,
- remove a slot,
- move a slot up or down,
- remove an image reference from a step.

Procedures are used instead of `raw!` JavaScript so the operations stay inside
Topcoat's supported runtime model and can be tested as ordinary Rust.

A step's toolbar contains: add below, move up, move down, duplicate, and remove.
The existing bottom "Add step" button appends a new empty row.

### Splitting

Splitting inserts a new slot immediately below the source row and copies the
source row's current title, instruction, ingredients, and existing step image
IDs. The copied values are carried through pending parent signals and applied
only when the new row is created.

The duplicated row receives an empty hidden step ID, so the parser generates a
new identity when the form is submitted. Unsaved file selections are not
duplicated; only image IDs already attached to the step are copied.

### Step images

Step image IDs are held in a signal per row. Removing an image updates that
list, so a later split copies only the images that remain. Step images use a
dedicated row component; recipe-level image rows keep their existing tombstone
behavior.

## Consequences

### Benefits

- Steps can be inserted anywhere, moved, duplicated, and removed without losing
  typed content.
- Step removal compacts the list instead of leaving hidden tombstones.
- The form wire format and server parser remain unchanged.
- Order manipulation has no dependency on internal browser runtime shapes.
- The list helpers are unit-tested.

### Tradeoffs

- Each order-changing operation calls a procedure and then re-renders the
  shard, so it takes two server round-trips instead of one.
- Splitting copies existing image IDs but not files selected in the upload
  control.
- Step rows use stable keys for signal identity while their wire indices are
  display positions; code must keep the two concepts distinct.
- The procedure endpoints are pure UI helpers and are not a public API.

### When to revisit

Reconsider this decision if:

- the extra request per operation becomes noticeable,
- steps need drag-and-drop reordering,
- or the form moves to a different client-state model.
