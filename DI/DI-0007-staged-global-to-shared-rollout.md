# DI-0007: Staged Global-to-Shared Rollout

Date: 2026-09-04
State: accepted
Source: TE-0008

Develop and validate the product in three capability stages:

1. one working local global todo ledger for one user;
2. that global ledger interacting incrementally with one, two, then three
   repositories; and
3. distinct personal and shared spaces with multiple users.

The current phase is stage 1 dogfooding. Stage 1 must support durable capture of
large, long-lived project roots and progressive subnode refinement. It need not
prematurely select Grid identity, synchronization, or shared-space semantics.

Stage 2 must preserve one global ledger: repositories supply context and
interaction boundaries, not separate todo databases. Stage 3 requires explicit
writer identity, agent-owned histories, local acceptance, concurrent-event,
privacy, retention, and reconciliation decisions before implementation.

The implementation language remains open under DR-0002. Editable Markdown
identity remains deferred under DR-0003. Neither question blocks stage 1.
