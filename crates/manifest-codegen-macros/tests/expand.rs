//! End-to-end macro expansion: invoke include_manifest! on a
//! fixture TOML manifest and verify the generated constants are
//! in scope + carry the expected values.

manifest_codegen_macros::include_manifest!("tests/fixtures/simple.toml");

#[test]
fn header_constants_resolve() {
    assert_eq!(MANIFEST_SCHEMA, "1");
    assert_eq!(MANIFEST_PLATFORM, "acme-test");
}

#[test]
fn capabilities_list_resolves() {
    assert_eq!(ALL_CAPABILITIES.len(), 1);
    let auth = &ALL_CAPABILITIES[0];
    assert_eq!(auth.id, "auth");
    assert_eq!(auth.summary, "user auth");
    assert_eq!(auth.ownership, "forge");
    assert_eq!(auth.handlers, &["forge-phases::auth"]);
    assert_eq!(auth.ui, &["cms-admin::auth"]);
    assert_eq!(auth.tests, &["forge-phases::auth::tests::ok"]);
    assert_eq!(auth.docs, &["docs/auth.md"]);
}

#[test]
fn phases_list_resolves() {
    assert_eq!(ALL_PHASES.len(), 1);
    let p = &ALL_PHASES[0];
    assert_eq!(p.id, "p-auth");
    assert_eq!(p.implements, "auth");
    assert_eq!(p.default_severity, "strict");
}

#[test]
fn backends_list_is_empty_when_unset() {
    assert!(ALL_BACKENDS.is_empty());
}

#[test]
fn find_capability_by_id_returns_matching_record() {
    let c = find_capability_by_id("auth").expect("auth capability");
    assert_eq!(c.summary, "user auth");
    assert!(find_capability_by_id("missing").is_none());
}

#[test]
fn find_phase_by_id_returns_matching_record() {
    let p = find_phase_by_id("p-auth").expect("p-auth phase");
    assert_eq!(p.implements, "auth");
    assert!(find_phase_by_id("nope").is_none());
}

#[test]
fn capability_exists_is_true_when_declared() {
    assert!(capability_exists("auth"));
    assert!(!capability_exists("missing"));
}

#[test]
fn phases_for_capability_filters_correctly() {
    let phases: Vec<_> = phases_for_capability("auth").collect();
    assert_eq!(phases.len(), 1);
    assert_eq!(phases[0].id, "p-auth");
    let none: Vec<_> = phases_for_capability("nope").collect();
    assert!(none.is_empty());
}

#[test]
fn backend_with_route_returns_none_when_no_backends() {
    assert!(backend_with_route("GET", "/anywhere").is_none());
}
