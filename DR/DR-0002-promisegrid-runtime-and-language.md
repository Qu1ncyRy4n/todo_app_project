# DR-0002: PromiseGrid Runtime and Implementation Language

Date: 2026-09-04
State: open
Question: Should the todo product remain Rust-native, bind to Grid through FFI,
use a Go Grid sidecar/service, or migrate the complete application to Go?
Why this does not block current dogfooding: the existing Rust CLI supports one
working local global ledger. Cross-language conformance and a sidecar prototype
can be evaluated before repository interaction or multi-user operation becomes
required.
Evidence needed: TE-0008's exact-byte vertical prototype, operational and
packaging comparison, failure recovery, cross-platform build evidence, and a
measured strict-Go toolchain configuration.

