use fancy_regex::Regex as FancyRegex;
use std::collections::HashMap;
use std::sync::Mutex;

/// A thread-safe cache for compiled regex patterns.
/// Patterns are compiled once and reused across all files,
/// eliminating the massive overhead of PHP's per-file regex compilation.
pub struct RegexCache {
    cache: Mutex<HashMap<String, FancyRegex>>,
}

impl RegexCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get a compiled regex, or compile and cache it.
    pub fn get_or_compile(&self, key: &str, pattern: &str) -> anyhow::Result<FancyRegex> {
        let mut cache = self.cache.lock().unwrap();

        if let Some(regex) = cache.get(key) {
            return Ok(regex.clone());
        }

        let compiled = FancyRegex::new(pattern)
            .map_err(|e| anyhow::anyhow!("Failed to compile regex '{}': {}", key, e))?;

        cache.insert(key.to_string(), compiled.clone());
        Ok(compiled)
    }

    /// Pre-compile multiple patterns at once.
    pub fn precompile(&self, patterns: &[(&str, &str)]) -> anyhow::Result<()> {
        let mut cache = self.cache.lock().unwrap();

        for (key, pattern) in patterns {
            if !cache.contains_key(*key) {
                let compiled = FancyRegex::new(pattern)
                    .map_err(|e| anyhow::anyhow!("Failed to compile regex '{}': {}", key, e))?;
                cache.insert(key.to_string(), compiled);
            }
        }

        Ok(())
    }

    /// Get the number of cached patterns.
    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().unwrap().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_get_or_compile() {
        let cache = RegexCache::new();
        let regex1 = cache.get_or_compile("test", r"\d+").unwrap();
        let regex2 = cache.get_or_compile("test", r"\d+").unwrap();

        // Both should work
        assert!(regex1.is_match("123").unwrap());
        assert!(regex2.is_match("456").unwrap());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_precompile() {
        let cache = RegexCache::new();
        cache
            .precompile(&[("digits", r"\d+"), ("words", r"\w+")])
            .unwrap();

        assert_eq!(cache.len(), 2);
    }
}
