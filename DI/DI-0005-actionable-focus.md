# DI-0005: Explicit Readiness and Explainable Next Selection

Date: 2026-08-28
State: accepted
Source: TE-0005

v0.2 adds priority, optional due and defer dates, and explicit blocking
relationships. Structural parenthood remains independent of blocking.

`ready` excludes done/canceled items, future-deferred items, and items with any
incomplete blocker. `next` uses the ordered policy in TE-0005 and can explain
the winning fields. Changes remain append-only events with Turso-backed views.
