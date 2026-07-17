//! Deferred-feature test surface. Each stub names the GitHub issue that will
//! implement it — visible via `cargo test -- --ignored --list`.

#[test]
#[ignore = "not implemented — exec-into-container terminal, see dockshell issue #2"]
fn exec_shell_in_container() {}

#[test]
#[ignore = "not implemented — image management, see dockshell issue #3"]
fn lists_pulls_and_removes_images() {}

#[test]
#[ignore = "not implemented — volume management, see dockshell issue #4"]
fn lists_and_removes_volumes() {}

#[test]
#[ignore = "not implemented — network management, see dockshell issue #5"]
fn lists_and_inspects_networks() {}

#[test]
#[ignore = "not implemented — compose file support, see dockshell issue #6"]
fn compose_up_from_file() {}

#[test]
#[ignore = "not implemented — TLS client-cert auth for remote engines, see dockshell issue #7"]
fn connects_with_tls_client_cert() {}
