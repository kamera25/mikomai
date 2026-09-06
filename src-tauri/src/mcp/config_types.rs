use std::fmt;

/// Vendors supported by the configuration converter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetVendor {
    Juniper,
    Arista,
}

impl TargetVendor {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "juniper" => Ok(Self::Juniper),
            "arista" => Ok(Self::Arista),
            _ => Err(format!("Unsupported target vendor: '{}'. Supported: 'juniper', 'arista'", value)),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Juniper => "juniper",
            Self::Arista => "arista",
        }
    }
}

impl fmt::Display for TargetVendor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_vendor_case_insensitively() {
        assert_eq!(TargetVendor::parse(" JUNIPER ").unwrap(), TargetVendor::Juniper);
        assert_eq!(TargetVendor::parse("arista").unwrap().as_str(), "arista");
    }

    #[test]
    fn rejects_unknown_vendor() {
        assert!(TargetVendor::parse("cisco").is_err());
    }
}
