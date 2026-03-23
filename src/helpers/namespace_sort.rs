/// Sort namespaces longest-first to avoid partial replacement issues.
pub fn sort_namespaces_longest_first(namespaces: &mut Vec<String>) {
    namespaces.sort_by(|a, b| b.len().cmp(&a.len()));
}

/// Sort namespaces shortest-first (used in some replacement contexts).
pub fn sort_namespaces_shortest_first(namespaces: &mut Vec<String>) {
    namespaces.sort_by(|a, b| a.len().cmp(&b.len()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_longest_first() {
        let mut namespaces = vec![
            "Monolog".to_string(),
            "Monolog\\Handler\\StreamHandler".to_string(),
            "Monolog\\Handler".to_string(),
        ];
        sort_namespaces_longest_first(&mut namespaces);
        assert_eq!(namespaces[0], "Monolog\\Handler\\StreamHandler");
        assert_eq!(namespaces[1], "Monolog\\Handler");
        assert_eq!(namespaces[2], "Monolog");
    }
}
