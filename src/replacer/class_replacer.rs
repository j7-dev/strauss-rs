use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

/// Handles classname replacement in PHP files.
/// Uses regex crate with word boundaries for O(n) matching.
pub struct ClassReplacer {
    cache: Mutex<HashMap<String, Regex>>,
}

impl ClassReplacer {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Replace a global classname. Uses \b word boundary for fast, accurate matching.
    pub fn replace_classname(
        &self,
        contents: &str,
        original_classname: &str,
        classmap_prefix: &str,
    ) -> Result<String> {
        if original_classname.is_empty() {
            return Ok(contents.to_string());
        }

        let pattern = self.get_or_compile(original_classname)?;
        let replacement = format!("{}{}", classmap_prefix, original_classname);

        // Replace all word-boundary matches
        let result = pattern.replace_all(contents, replacement.as_str());
        Ok(result.to_string())
    }

    fn get_or_compile(&self, classname: &str) -> Result<Regex> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(re) = cache.get(classname) {
            return Ok(re.clone());
        }

        // Use word boundary matching - simple and fast
        let pattern = format!(r"\b{}\b", regex::escape(classname));
        let re = Regex::new(&pattern)?;

        cache.insert(classname.to_string(), re.clone());
        Ok(re)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_classname_replacement() {
        let replacer = ClassReplacer::new();
        let contents = "$obj = new FPDF();\n$pdf = FPDF::create();\n";
        let result = replacer
            .replace_classname(contents, "FPDF", "MyPrefix_")
            .unwrap();

        assert!(result.contains("new MyPrefix_FPDF()"));
        assert!(result.contains("MyPrefix_FPDF::create()"));
    }
}
