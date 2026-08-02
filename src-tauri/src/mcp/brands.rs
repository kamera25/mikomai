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

pub fn get_brand(input: &str) -> Option<&'static str> {
    let trimmed = input.trim().to_lowercase();
    match trimmed.as_str() {
        "cisco" | "cisco_ios" | "cisco_xe" | "cisco_xr" | "cisco_nxos" | "ios" => Some("cisco_ios"),
        "juniper" | "junos" | "juniper_junos" => Some("juniper_junos"),
        "arista" | "eos" | "arista_eos" => Some("arista_eos"),
        "yamaha" => Some("yamaha"),
        "furukawa" | "fitelnet" | "furukawa_fitelnet" => Some("furukawa_fitelnet"),
        "fortinet" | "fortigate" | "fortios" => Some("fortinet"),
        "a10" | "a10_ax" | "a10_vthreads" => Some("a10"),
        "paloalto" | "panos" | "paloalto_panos" => Some("paloalto_panos"),
        _ => None,
    }
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
