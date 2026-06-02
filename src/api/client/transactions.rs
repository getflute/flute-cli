//! `ApiClient` endpoint methods for the Transactions API group
//! (`/pay-api/v1/transactions/*`).
//!
//! Each method is an `impl ApiClient` extension that calls
//! `self.send(…)` / `self.send_no_body(…)` from the core in `mod.rs`.
//! Populated in Tasks 1.4+; the module is wired here so the module tree
//! compiles from the outset.
