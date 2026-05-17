//! End-to-end macro expansion: invoke include_manifest! on a
//! fixture TOML manifest and verify the generated constants are
//! in scope + carry the expected values.

manifest_codegen_macros::include_manifest!("tests/fixtures/simple.toml");

#[test]
fn header_constants_resolve() {
    assert_eq!(MANIFEST_SCHEMA, "1");
    assert_eq!(MANIFEST_PLATFORM, "plausiden-test");
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
