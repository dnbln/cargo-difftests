#![feature(exit_status_error)]

mod test_support;
use test_support::*;

#[test]
fn simple_test() -> R {
    let project = create_cargo_project("simple_test", CargoProjectConfig::default())?;

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

#[test]
fn test_git_diff_files() -> R {
    let project = create_cargo_project(
        "test_git_diff_files",
        CargoProjectConfig {
            init_git: true,
            ..CargoProjectConfig::default()
        },
    )?;
    let repo = project.load_git_repo()?;

    project.edit("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }")?;
    project.edit(
        "tests/tests.rs",
        DifftestsSetupCode(
            r#"
    use test_git_diff_files::add;

    #[test]
    fn test_add() {
        let _env = setup_difftests("test_add");
        assert_eq!(add(1, 2), 3);
    }
    "#,
        ),
    )?;
    let strategy = TestAnalysisStrategyInfo {
        algo: AnalysisAlgo::git_diff_files_with_head(),
        ..TestAnalysisStrategyInfo::default()
    };

    let _commit = project.commit(&repo, "Commit 2", ["src/lib.rs", "tests/tests.rs"].iter())?;

    project.run_all_tests_difftests()?;
    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    // edit file, so it is different from HEAD
    project.edit(
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b + 1 }",
    )?;

    // now the algorithm should tell us that it's dirty
    project
        .analyze_test("test_add", &strategy)?
        .assert_is_dirty()?;

    // but if we go back to the old version of the file

    project.edit("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }")?;

    // it should be clean again
    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    Ok(())
}

#[test]
fn test_git_diff_files_with_commit() -> R {
    let project = create_cargo_project(
        "test_git_diff_files",
        CargoProjectConfig {
            init_git: true,
            ..CargoProjectConfig::default()
        },
    )?;
    let repo = project.load_git_repo()?;

    project.edit("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }")?;
    project.edit(
        "tests/tests.rs",
        DifftestsSetupCode(
            r#"
    use test_git_diff_files::add;

    #[test]
    fn test_add() {
        let _env = setup_difftests("test_add");
        assert_eq!(add(1, 2), 3);
    }
    "#,
        ),
    )?;

    let commit = project.commit(&repo, "Commit 2", ["src/lib.rs", "tests/tests.rs"].iter())?;

    project.run_all_tests_difftests()?;

    project.edit("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 {a+b}")?;

    let commit2 = project.commit(&repo, "Commit 3", ["src/lib.rs"].iter())?;

    let strategy = TestAnalysisStrategyInfo {
        algo: AnalysisAlgo::git_diff_files_with_commit(commit),
        ..TestAnalysisStrategyInfo::default()
    };

    let strategy2 = TestAnalysisStrategyInfo {
        algo: AnalysisAlgo::git_diff_files_with_commit(commit2),
        ..TestAnalysisStrategyInfo::default()
    };

    project
        .analyze_test("test_add", &strategy2)?
        .assert_is_clean()?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_dirty()?;

    project.edit("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }")?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    project
        .analyze_test("test_add", &strategy2)?
        .assert_is_dirty()?;

    Ok(())
}

#[test]
fn test_git_diff_hunks() -> R {
    let project = create_cargo_project(
        "test_git_diff_hunks",
        CargoProjectConfig {
            init_git: true,
            ..CargoProjectConfig::default()
        },
    )?;

    project.edit(
        "src/lib.rs",
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn sub(a: i32, b: i32) -> i32 {
    a - b
}

pub fn extra() {
    println!("extra");
}

"#,
    )?;

    project.edit(
        "tests/tests.rs",
        DifftestsSetupCode(
            r#"
use test_git_diff_hunks::{add, sub};

#[test]
fn test_add() {
    let _env = setup_difftests("test_add");
    assert_eq!(add(1, 2), 3);
}

#[test]
fn test_sub() {
    let _env = setup_difftests("test_sub");
    assert_eq!(sub(3, 2), 1);
}

"#,
        ),
    )?;

    let _commit = project.commit(
        &project.load_git_repo()?,
        "Commit 2",
        ["src/lib.rs", "tests/tests.rs"].iter(),
    )?;

    project.run_all_tests_difftests()?;

    let strategy = TestAnalysisStrategyInfo {
        algo: AnalysisAlgo::git_diff_hunks_with_head(),
        ..TestAnalysisStrategyInfo::default()
    };

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_clean()?;

    project.edit(
        "src/lib.rs",
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    // add a comment here
    a + b
}

pub fn sub(a: i32, b: i32) -> i32 {
    a - b
}

pub fn extra() {
    println!("extra");
}

"#,
    )?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_dirty()?;
    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_clean()?;

    project.edit(
        "src/lib.rs",
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn sub(a: i32, b: i32) -> i32 {
    // add a comment here
    a - b
}

pub fn extra() {
    println!("extra");
}

"#,
    )?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_dirty()?;

    project.edit("src/lib.rs", r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn sub(a: i32, b: i32) -> i32 {
    a - b
}

pub fn extra() {
    println!("extra");
    // add a few comments to parts unused by both tests
    // add a few comments to parts unused by both tests
}

    "#)?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_clean()?;

    // go back to original state
    project.edit(
        "src/lib.rs",
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn sub(a: i32, b: i32) -> i32 {
    a - b
}

pub fn extra() {
    println!("extra");
}

"#,
    )?;

    project
        .analyze_test("test_add", &strategy)?
        .assert_is_clean()?;

    project
        .analyze_test("test_sub", &strategy)?
        .assert_is_clean()?;

    Ok(())
}
