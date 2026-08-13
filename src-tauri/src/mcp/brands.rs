use std::collections::HashMap;
use std::sync::LazyLock;
use serde::Deserialize;

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
}

