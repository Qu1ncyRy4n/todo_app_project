# Todo Ledger Event Protocol v14

Status: ledger-local protocol for explicit Git synchronization. It retains v13
and adds namespace fields, namespace links, roles on namespace assignments, and
the symmetric `relates-to` relation.

The pCID is `CIDv1(raw/0x55, sha2-256(specification bytes))` over this file.
The envelope and deterministic ten-field payload are exactly as specified by
v5. Historical v1-v13 events remain valid and replayable.

`namespace.field-set` has a namespace subject and carries `key` and `value` in
data. It sets one field on the namespace node. An empty `value` clears the
field. Keys use the namespace segment alphabet: lowercase letters, digits,
dash, and underscore. Keys are open: a new key never needs a protocol version.
When two events set the same key, the later in canonical order wins.

`namespace.linked` has a namespace subject, a `relation` field in data, and a
`target` link holding another namespace. `namespace.unlinked` has the same
shape and withdraws one link. The only relation name is `member-of`: an actor
belongs to another actor, such as a person to an organization. A namespace has
at most one link of a given name to a given target, and may not link to itself.

`namespace.assigned` and `namespace.removed` may carry a `role` field in data.
Without it, the assignment is a plain tag, exactly as before. With it, the item
is attached to the namespace in that role, such as `author`, `contributor`,
`assignee`, `requester`, or any other role name in the segment alphabet. A role
assignment and a plain assignment to the same namespace are independent, and an
item may hold several roles on one namespace.

`item.relation-added` and `item.relation-removed` accept `relates-to` as a
relation name. `relates-to` is symmetric: it is stored once, in the direction
it was written, and read in both directions. A relation from A to B is refused
while one from B to A exists, so the pair is never stored twice.

An actor is a namespace under the `actor` kind, such as `actor/quincy`. Its
`kind` field says whether it is a `person`, an `org`, or an `agent`. An agent
may nest under its one owner, such as `actor/quincy/agent/review-bot`.
