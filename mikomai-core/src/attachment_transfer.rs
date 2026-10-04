//! Intent comes from the user's request, never from attachment contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadProtocol {
    Ftp,
    Tftp,
}
impl UploadProtocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ftp => "FTP",
            Self::Tftp => "TFTP",
        }
    }
    pub fn port(self) -> u16 {
        match self {
            Self::Ftp => 21,
            Self::Tftp => 69,
        }
    }
    pub fn tool(self) -> &'static str {
        match self {
            Self::Ftp => "network_ftp_upload",
            Self::Tftp => "network_tftp_upload",
        }
    }
}
pub fn upload_protocol(message: &str) -> Option<UploadProtocol> {
    let text = message.to_lowercase();
    let explicit = ["添付", "attached", "attachment"]
        .iter()
        .any(|word| text.contains(word))
        && ["アップロード", "upload", "転送", "送って", "送信"]
            .iter()
            .any(|word| text.contains(word))
        && ![
            "方法",
            "手順",
            "コマンド",
            "やり方",
            "解析",
            "分析",
            "説明",
            "読み取",
            "how ",
            "analyz",
            "describe",
        ]
        .iter()
        .any(|word| text.contains(word));
    if !explicit {
        return None;
    }
    let mut found = None;
    for word in text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_') {
        let protocol = match word {
            "tftp" => UploadProtocol::Tftp,
            "ftp" => UploadProtocol::Ftp,
            _ => continue,
        };
        if found.is_some_and(|previous| previous != protocol) {
            return None;
        }
        found = Some(protocol);
    }
    found
}

/// Missing or ambiguous destinations require a clarification, not inference.
pub fn upload_target(message: &str) -> Option<String> {
    let pattern = regex::Regex::new(r"(?:[0-9]{1,3}\.){3}[0-9]{1,3}").ok()?;
    let addresses: Vec<_> = pattern.find_iter(message).collect();
    if addresses.len() == 1 {
        let found = addresses[0];
        let adjacent = |ch: char| ch.is_ascii_alphanumeric() || ".-_".contains(ch);
        if message[..found.start()]
            .chars()
            .next_back()
            .is_some_and(adjacent)
            || message[found.end()..].chars().next().is_some_and(adjacent)
        {
            return None;
        }
        let host = found.as_str();
        return host
            .parse::<std::net::Ipv4Addr>()
            .ok()
            .map(|_| host.to_owned());
    }
    if !addresses.is_empty() {
        return None;
    }
    let pattern =
        regex::Regex::new(r"(?i)(?:を|to\s+)([a-z0-9][a-z0-9.-]{0,254})(?:へ|に|\s|$)").ok()?;
    let targets: Vec<_> = pattern.captures_iter(message).collect();
    if targets.len() != 1
        || ["tftp", "ftp"]
            .iter()
            .any(|word| targets[0][1].eq_ignore_ascii_case(word))
    {
        return None;
    }
    Some(targets[0][1].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachment_upload_is_distinct_from_image_analysis_or_instructions() {
        let prompt = "添付を192.168.50.200へTFTPへアップロード";
        assert_eq!(upload_protocol(prompt), Some(UploadProtocol::Tftp));
        assert_eq!(upload_target(prompt).as_deref(), Some("192.168.50.200"));
        assert_eq!(
            upload_protocol("添付画像をTFTPで192.168.50.200に送って"),
            Some(UploadProtocol::Tftp)
        );
        assert_eq!(
            upload_protocol("添付を192.168.50.200へFTPへアップロード"),
            Some(UploadProtocol::Ftp)
        );
        assert_eq!(
            upload_target("添付をserver.exampleへTFTPアップロード").as_deref(),
            Some("server.example")
        );
        assert!(upload_target("添付を192.0.2.1と192.0.2.2へTFTP転送").is_none());
        assert!(upload_target("添付を192.0.2.1.99へTFTP転送").is_none());
        for prompt in [
            "添付画像の内容を説明して",
            "添付画像を解析してTFTPで送って",
            "添付のTFTPアップロード方法を教えて",
            "TFTPとは",
            "添付を192.168.50.200へSFTPアップロード",
            "添付を192.168.50.200へFTPSアップロード",
            "添付をFTPまたはTFTPでアップロード",
            "添付をFTP/TFTPでアップロード",
        ] {
            assert!(upload_protocol(prompt).is_none(), "{prompt}");
        }
    }
}
