#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    t.pass("tests/try/ok/basic.rs");
    t.pass("tests/try/ok/default_table.rs");
    t.pass("tests/try/ok/option_types.rs");

    t.compile_fail("tests/try/err/missing_fromrow.rs");
    t.compile_fail("tests/try/err/not_public.rs");
    t.compile_fail("tests/try/err/not_struct.rs");
    t.compile_fail("tests/try/err/unnamed_fields.rs");
    t.compile_fail("tests/try/err/wrong_form.rs");
    t.compile_fail("tests/try/err/unknown_key.rs");
    t.compile_fail("tests/try/err/duplicate_key.rs");
    t.compile_fail("tests/try/err/wrong_literal.rs");
}
