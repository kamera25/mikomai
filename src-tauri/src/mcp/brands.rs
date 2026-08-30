use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

pub const BRANDS: &[&str] = &[
    "cisco_ios",
    "juniper_junos",
    "arista_eos",
    "yamaha",
    "furukawa_fitelnet",
    "fortinet",
    "a10",
    "paloalto_panos",
];

#[derive(Debug, Deserialize)]
struct BrandConfig {
    aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BrandsYaml {
    brands: HashMap<String, BrandConfig>,
}

static BRAND_MAP: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    let yaml_str = include_str!("config/brands.yaml");
    let parsed: BrandsYaml = serde_yaml::from_str(yaml_str).unwrap_or_else(|e| {
        log::error!("Failed to parse brands.yaml: {}", e);
        BrandsYaml {
            brands: HashMap::new(),
        }
    });

    let mut map = HashMap::new();
    for brand_name in BRANDS {
        if let Some(config) = parsed.brands.get(*brand_name) {
            for alias in &config.aliases {
                map.insert(alias.to_lowercase(), *brand_name);
            }
        }
    }
    map
});

pub fn get_brand(input: &str) -> Option<&'static str> {
    let trimmed = input.trim().to_lowercase();
    BRAND_MAP.get(&trimmed).copied()
}

static BRAND_ALIASES: LazyLock<Vec<(String, &'static str)>> = LazyLock::new(|| {
    let mut list: Vec<(String, &'static str)> =
        BRAND_MAP.iter().map(|(k, v)| (k.clone(), *v)).collect();
    // Sort by alias length descending so longer aliases match first (e.g., "cisco_ios" before "cisco")
    list.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    list
});

pub fn detect_brand_in_text(text: &str) -> Option<(&'static str, String)> {
    let lower_text = text.to_lowercase();
    for (alias, brand) in BRAND_ALIASES.iter() {
        if lower_text.contains(alias) {
            return Some((*brand, alias.clone()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_brand_furukawa() {
        assert_eq!(get_brand("furukawa"), Some("furukawa_fitelnet"));
        assert_eq!(get_brand("Furukawa"), Some("furukawa_fitelnet"));
        assert_eq!(get_brand("fitelnet"), Some("furukawa_fitelnet"));
        assert_eq!(get_brand("cisco"), Some("cisco_ios"));
        assert_eq!(get_brand("juniper"), Some("juniper_junos"));
        assert_eq!(get_brand("unknown_vendor"), None);
    }

    #[test]
    fn test_detect_brand_in_text() {
        let res = detect_brand_in_text("Cisco 841Jのインターフェース設定方法");
        assert!(res.is_some());
        let (brand, alias) = res.unwrap();
        assert_eq!(brand, "cisco_ios");
        assert_eq!(alias, "cisco");

        let res2 = detect_brand_in_text("Fortigateのポリシールーティング");
        assert!(res2.is_some());
        let (brand2, alias2) = res2.unwrap();
        assert_eq!(brand2, "fortinet");
        assert_eq!(alias2, "fortigate");
    }
}
