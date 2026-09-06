//! Compile-time guard: editor module must be reachable from the domain crate.

#[test]
fn editor_module_is_reachable() {
    assert_eq!(crate::editor::MODULE_STUB, "editor");
}
