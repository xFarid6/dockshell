//! Deferred-feature test surface. Each stub names the GitHub issue that will
//! implement it — visible via `cargo test -- --ignored --list`.

#[test]
#[ignore = "not implemented — TLS client-cert auth for remote engines, see dockshell issue #7"]
fn connects_with_tls_client_cert() {}
