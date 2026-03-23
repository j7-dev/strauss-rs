use anyhow::Result;
use std::fs;
use std::path::Path;

/// Add an include line for the Strauss autoloader to vendor/autoload.php.
///
/// Port of PHP IncludeAutoloaderCommand
pub fn run_include_autoloader(working_dir: &Path) -> Result<()> {
    let autoload_path = working_dir.join("vendor/autoload.php");

    if !autoload_path.exists() {
        anyhow::bail!("vendor/autoload.php not found");
    }

    let contents = fs::read_to_string(&autoload_path)?;

    // Check if already included
    if contents.contains("vendor-prefixed/autoload.php") {
        log::info!("Autoloader already included in vendor/autoload.php");
        return Ok(());
    }

    // Add the include line before the return statement
    let include_line = "\n// Load Strauss autoloader\nif (file_exists(__DIR__ . '/../vendor-prefixed/autoload.php')) {\n    require_once __DIR__ . '/../vendor-prefixed/autoload.php';\n}\n";

    let updated = if let Some(pos) = contents.rfind("return ") {
        let (before, after) = contents.split_at(pos);
        format!("{}{}{}", before, include_line, after)
    } else {
        format!("{}{}", contents, include_line)
    };

    fs::write(&autoload_path, updated)?;
    log::info!("Added Strauss autoloader include to vendor/autoload.php");

    Ok(())
}
