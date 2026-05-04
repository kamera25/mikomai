pub const BRANDS: &[&str] = &[
    "Cisco",
    "Yamaha",
    "Juniper",
    "Arista",
    "Fitelnet",
    "Fortigate",
    "A10",
    "PaloAlto",
];

pub fn get_brand(input: &str) -> Option<&'static str> {
    let trimmed = input.trim();
    BRANDS.iter().find(|&&b| b.eq_ignore_ascii_case(trimmed)).copied()
}
