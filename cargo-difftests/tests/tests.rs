#![feature(exit_status_error)]

mod test_support;
use test_support::*;

#[test]
fn simple_test() -> R {
    let project = create_cargo_project("simple_test")?;

    project.edit("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }")?;
    project.edit(
        "tests/tests.rs",
        DifftestsSetupCode(
            r#"
    use simple_test::add;

    #[test]
    fn test_add() {
        let _env = setup_difftests("test_add");
        assert_eq!(add(1, 2), 3);
    }
    "#,
        ),
    )?;

    project.run_all_tests_difftests()?;

    let strategy = TestAnalysisStrategyInfo::default();

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    project.touch_file("src/lib.rs")?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_dirty()?;

    project.run_test_difftests("test_add")?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    Ok(())
}

#[test]
fn sample_project_test() -> R {
    let project = init_sample_project("sample_project_test")?;

    project.run_all_tests_difftests()?;

    project.cargo_difftests()?.args(&["analyze-all"]).run()?;

    project.touch_file("src/lib.rs")?;

    let strategy = TestAnalysisStrategyInfo::default();

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_dirty()?;
    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_dirty()?;
    project
        .analyze_test("test_mul", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_div", &strategy)?
        .assert_is_clean()?;

    project.run_test_difftests("test_add")?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_dirty()?;

    project.run_test_difftests("test_sub")?;

    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_clean()?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_mul", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_div", &strategy)?
        .assert_is_clean()?;

    project.touch_file("src/advanced_arithmetic.rs")?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_mul", &strategy)?
        .assert_is_dirty()?;
    project
        .analyze_test("test_div", &strategy)?
        .assert_is_dirty()?;

    project.run_test_difftests("test_mul")?;

    project
        .analyze_test("test_mul", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_div", &strategy)?
        .assert_is_dirty()?;

    project.run_test_difftests("test_div")?;

    project
        .analyze_test("test_div", &strategy)?
        .assert_is_clean()?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_clean()?;
    project
        .analyze_test("test_mul", &strategy)?
        .assert_is_clean()?;

    Ok(())
}
