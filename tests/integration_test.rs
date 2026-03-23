use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Recursively copy a directory tree from `src` to `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Set up an isolated copy of the `simple-project` fixture in a temp directory.
/// Returns the `TempDir` handle (keeps the directory alive) and the working path.
fn setup_fixture() -> (TempDir, PathBuf) {
    let temp = TempDir::new().expect("failed to create temp dir");
    let fixture_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("simple-project");

    copy_dir_recursive(&fixture_src, temp.path()).expect("failed to copy fixture");

    // Remove any stale vendor-prefixed directory that might have leaked into the fixture.
    let stale_target = temp.path().join("vendor-prefixed");
    if stale_target.exists() {
        fs::remove_dir_all(&stale_target).expect("failed to remove stale vendor-prefixed");
    }

    let working_dir = temp.path().to_path_buf();
    (temp, working_dir)
}

/// Run the strauss pipeline against the given working directory.
fn run_strauss(working_dir: &Path) {
    let mut pipeline =
        strauss::pipeline::Pipeline::new(working_dir.to_path_buf()).expect("Pipeline::new failed");
    pipeline.run().expect("Pipeline::run failed");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_namespace_prefixing() {
    let (_tmp, working_dir) = setup_fixture();
    run_strauss(&working_dir);

    let hello_php = working_dir
        .join("vendor-prefixed")
        .join("acme")
        .join("greeter")
        .join("src")
        .join("Hello.php");
    let content = fs::read_to_string(&hello_php).expect("failed to read Hello.php");

    // The original `namespace Acme\Greeter` must be replaced with the prefixed variant.
    assert!(
        content.contains(r"namespace Test\SimpleProject\Deps\Acme\Greeter"),
        "Hello.php should have the prefixed namespace.\nActual content:\n{content}"
    );

    // The original unprefixed namespace must NOT appear.
    assert!(
        !content.contains("namespace Acme\\Greeter;"),
        "Hello.php should not contain the original unprefixed namespace"
    );
}

#[test]
fn test_use_statement_prefixing() {
    let (_tmp, working_dir) = setup_fixture();
    run_strauss(&working_dir);

    let formatter_php = working_dir
        .join("vendor-prefixed")
        .join("acme")
        .join("greeter")
        .join("src")
        .join("Formatter.php");
    let content = fs::read_to_string(&formatter_php).expect("failed to read Formatter.php");

    // The `use` statement should be prefixed (the replacer may insert a leading backslash).
    assert!(
        content.contains(r"Test\SimpleProject\Deps\Acme\Greeter\Hello"),
        "Formatter.php should have the prefixed use statement.\nActual content:\n{content}"
    );

    // The original unprefixed use must NOT appear.
    assert!(
        !content.contains(r"use Acme\Greeter\Hello"),
        "Formatter.php should not contain the original unprefixed use statement"
    );
}

#[test]
fn test_interface_preserved() {
    let (_tmp, working_dir) = setup_fixture();
    run_strauss(&working_dir);

    let formatter_php = working_dir
        .join("vendor-prefixed")
        .join("acme")
        .join("greeter")
        .join("src")
        .join("Formatter.php");
    let content = fs::read_to_string(&formatter_php).expect("failed to read Formatter.php");

    // The interface declaration must still be present.
    assert!(
        content.contains("interface FormatterInterface"),
        "Formatter.php should still declare FormatterInterface.\nActual content:\n{content}"
    );

    // The implements clause must still reference it.
    assert!(
        content.contains("implements FormatterInterface"),
        "Formatter.php should still have 'implements FormatterInterface'.\nActual content:\n{content}"
    );
}

#[test]
fn test_type_hints_preserved() {
    let (_tmp, working_dir) = setup_fixture();
    run_strauss(&working_dir);

    let formatter_php = working_dir
        .join("vendor-prefixed")
        .join("acme")
        .join("greeter")
        .join("src")
        .join("Formatter.php");
    let content = fs::read_to_string(&formatter_php).expect("failed to read Formatter.php");

    // The property type hint `Hello $hello` should be preserved (class name Hello is local
    // within the same namespace, so the short name should remain).
    assert!(
        content.contains("Hello $hello"),
        "Formatter.php should preserve the Hello type hint.\nActual content:\n{content}"
    );
}

#[test]
fn test_files_copied_to_target() {
    let (_tmp, working_dir) = setup_fixture();
    run_strauss(&working_dir);

    let target = working_dir.join("vendor-prefixed");
    assert!(target.exists(), "vendor-prefixed directory should exist");

    // The directory structure should mirror vendor/acme/greeter/src/
    let expected_files = vec![
        target.join("acme").join("greeter").join("src").join("Hello.php"),
        target.join("acme").join("greeter").join("src").join("Formatter.php"),
        target.join("acme").join("greeter").join("src").join("functions.php"),
    ];

    for path in &expected_files {
        assert!(
            path.exists(),
            "Expected file not found: {}",
            path.display()
        );
    }
}

#[test]
fn test_license_copied() {
    let (_tmp, working_dir) = setup_fixture();
    run_strauss(&working_dir);

    let license = working_dir
        .join("vendor-prefixed")
        .join("acme")
        .join("greeter")
        .join("LICENSE");
    assert!(
        license.exists(),
        "LICENSE file should be copied to vendor-prefixed/acme/greeter/"
    );

    let content = fs::read_to_string(&license).expect("failed to read LICENSE");
    assert!(
        content.contains("MIT License"),
        "LICENSE content should be intact"
    );
}

#[test]
fn test_autoloader_generated() {
    let (_tmp, working_dir) = setup_fixture();
    run_strauss(&working_dir);

    let target = working_dir.join("vendor-prefixed");

    // autoload.php entry point
    let autoload_php = target.join("autoload.php");
    assert!(
        autoload_php.exists(),
        "vendor-prefixed/autoload.php should exist"
    );

    // composer/autoload_psr4.php
    let psr4 = target.join("composer").join("autoload_psr4.php");
    assert!(
        psr4.exists(),
        "vendor-prefixed/composer/autoload_psr4.php should exist"
    );

    // composer/autoload_classmap.php
    let classmap = target.join("composer").join("autoload_classmap.php");
    assert!(
        classmap.exists(),
        "vendor-prefixed/composer/autoload_classmap.php should exist"
    );
}

#[test]
fn test_non_php_files_copied_unchanged() {
    let (_tmp, working_dir) = setup_fixture();
    run_strauss(&working_dir);

    let target_composer_json = working_dir
        .join("vendor-prefixed")
        .join("acme")
        .join("greeter")
        .join("composer.json");

    assert!(
        target_composer_json.exists(),
        "Non-PHP file composer.json should be copied to vendor-prefixed"
    );

    // It should be byte-identical to the original.
    let original = fs::read_to_string(
        working_dir
            .join("vendor")
            .join("acme")
            .join("greeter")
            .join("composer.json"),
    )
    .expect("failed to read original composer.json");
    let copied = fs::read_to_string(&target_composer_json).expect("failed to read copied composer.json");

    assert_eq!(
        original, copied,
        "Non-PHP files should be copied without modification"
    );
}

#[test]
fn test_original_vendor_unchanged() {
    let (_tmp, working_dir) = setup_fixture();

    // Snapshot original contents before running the pipeline.
    let original_hello = fs::read_to_string(
        working_dir
            .join("vendor")
            .join("acme")
            .join("greeter")
            .join("src")
            .join("Hello.php"),
    )
    .expect("failed to read original Hello.php");

    let original_formatter = fs::read_to_string(
        working_dir
            .join("vendor")
            .join("acme")
            .join("greeter")
            .join("src")
            .join("Formatter.php"),
    )
    .expect("failed to read original Formatter.php");

    run_strauss(&working_dir);

    // After the pipeline, the original vendor files should remain untouched.
    let after_hello = fs::read_to_string(
        working_dir
            .join("vendor")
            .join("acme")
            .join("greeter")
            .join("src")
            .join("Hello.php"),
    )
    .expect("original Hello.php should still exist");

    let after_formatter = fs::read_to_string(
        working_dir
            .join("vendor")
            .join("acme")
            .join("greeter")
            .join("src")
            .join("Formatter.php"),
    )
    .expect("original Formatter.php should still exist");

    assert_eq!(
        original_hello, after_hello,
        "Original vendor/acme/greeter/src/Hello.php should NOT be modified (delete_vendor_packages=false)"
    );
    assert_eq!(
        original_formatter, after_formatter,
        "Original vendor/acme/greeter/src/Formatter.php should NOT be modified (delete_vendor_packages=false)"
    );
}

#[test]
fn test_dry_run_no_changes() {
    let (_tmp, working_dir) = setup_fixture();

    let mut pipeline =
        strauss::pipeline::Pipeline::new(working_dir.to_path_buf()).expect("Pipeline::new failed");
    pipeline.config_mut().dry_run = true;
    pipeline.run().expect("Pipeline::run (dry_run) failed");

    let target = working_dir.join("vendor-prefixed");
    assert!(
        !target.exists(),
        "vendor-prefixed directory should NOT be created when dry_run=true"
    );
}
