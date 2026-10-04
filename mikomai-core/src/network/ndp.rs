//! Read-only local IPv6 neighbor observations. No IPv4 ARP inference is made.

/// Explicit command form stays in FastRouter; prose uses the Agent planner.
pub fn is_command(goal: &str) -> bool {
    goal.split_whitespace().collect::<Vec<_>>() == ["ndp", "-a"]
}

/// Recognize a single request for this computer's IPv6 neighbor cache.
/// Anchoring prevents a compound investigation or a remote target being lost.
pub fn is_local_request(goal: &str) -> bool {
    if is_command(goal) { return true; }
    regex::Regex::new(
        r"(?ix)^(?:localhost|::1|自機|ローカル|このmac|このpc|この端末|このコンピュータ)\s*(?:の|で)?\s*(?:ipv6\s*(?:の\s*)?)?(?:ndp(?:\s*(?:テーブル|キャッシュ))?|近隣(?:テーブル|キャッシュ)|隣接(?:テーブル|キャッシュ)|neighbor\s*(?:table|cache))\s*(?:を\s*)?(?:確認|取得|表示|見せて|教えて|調べて|読み取って|一覧を見せて)(?:して|してください|して下さい|ください|下さい)?[？?。！!]*$"
    ).is_ok_and(|pattern| pattern.is_match(goal.trim()))
}

pub fn local_answer(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    let Some(header) = raw.lines().next() else {
        return Err("自機のNDP取得結果が空です。近隣エントリーの有無は判断できません。".into());
    };
    if !header.contains("Neighbor") || !header.contains("Linklayer Address") || !header.contains("Netif") {
        return Err("自機のNDP取得結果の形式を確認できませんでした。".into());
    }
    let masked_mac = raw.lines().skip(1).flat_map(str::split_whitespace).any(|token| {
        let octets = token.split(':').collect::<Vec<_>>();
        octets.len() == 6 && octets.iter().zip([2, 0, 0, 0, 0, 0]).all(|(octet, expected)| {
            !octet.is_empty() && octet.len() <= 2 && u8::from_str_radix(octet, 16) == Ok(expected)
        })
    });
    let note = if masked_mac {
        "\n取得したMACアドレスにマスク値（02:00:00:00:00:00）が含まれています。実際のMACアドレスや近隣機器の完全な一覧を確認できた結果としては扱えません。"
    } else if raw.lines().skip(1).all(|line| line.trim().is_empty()) {
        "\n近隣エントリーは取得できませんでした。キャッシュが空の場合と、取得が制限されている場合をこの結果だけでは区別できません。"
    } else { "" };
    Ok(format!("自機のNDPテーブル（IPv6近隣キャッシュ）です。{note}\n\n```text\n{raw}\n```"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_scope_and_single_operation_are_required() {
        assert!(is_command("ndp -a"));
        assert!(is_local_request("このMacのIPv6近隣キャッシュを見せてください"));
        for goal in ["R1のNDPテーブルを確認して", "ndp -d fe80::1", "ndp -a; whoami", "このMacのNDPを確認してからpingして", "NDPの仕組みを教えて"] {
            assert!(!is_local_request(goal), "{goal}");
        }
    }
    #[test]
    fn empty_or_header_only_output_does_not_prove_absence() {
        assert!(local_answer("").is_err());
        assert!(local_answer("permission denied").is_err());
        let header = "Neighbor Linklayer Address Netif Expire St Flgs Prbs";
        assert!(local_answer(header).unwrap().contains("区別できません"));
        let raw = format!("{header}\nfe80::1%en0 aa:bb:cc:dd:ee:ff en0 10s R");
        assert!(local_answer(&raw).unwrap().contains(&raw));
        for mac in ["2:0:0:0:0:0", "02:00:00:00:00:00"] {
            let masked = format!("{header}\nfe80::1%en0 {mac} en0 permanent R");
            assert!(local_answer(&masked).unwrap().contains("マスク値"));
        }
        let unmasked = format!("{header}\nfe80::1%en0 02:aa:bb:cc:dd:ee en0 10s R");
        assert!(!local_answer(&unmasked).unwrap().contains("マスク値"));
    }
}
