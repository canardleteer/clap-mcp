#[test]
fn macro_ui_cases_compile() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass/*.rs");
}
