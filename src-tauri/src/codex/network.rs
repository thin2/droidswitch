//! Read the user's existing macOS proxy; do not modify system networking.

fn parse_system_proxy(raw: &str) -> Result<Option<String>, String> {
    let values: std::collections::HashMap<_, _> = raw
        .lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once(" : ")?;
            Some((key, value.trim()))
        })
        .collect();
    for (enabled, host, port, scheme) in [
        ("HTTPSEnable", "HTTPSProxy", "HTTPSPort", "http"),
        ("HTTPEnable", "HTTPProxy", "HTTPPort", "http"),
        ("SOCKSEnable", "SOCKSProxy", "SOCKSPort", "socks5"),
    ] {
        if values.get(enabled) != Some(&"1") {
            continue;
        }
        let host = values.get(host).ok_or("系统代理缺少服务器地址")?;
        let port: u16 = values
            .get(port)
            .and_then(|s| s.parse().ok())
            .filter(|p| *p != 0)
            .ok_or("系统代理端口无效")?;
        let mut url = reqwest::Url::parse(&format!("{scheme}://127.0.0.1")).unwrap();
        url.set_host(Some(host))
            .map_err(|_| "系统代理服务器地址无效")?;
        url.set_port(Some(port)).map_err(|_| "系统代理端口无效")?;
        return Ok(Some(url.to_string().trim_end_matches('/').to_owned()));
    }
    if values.get("ProxyAutoConfigEnable") == Some(&"1")
        || values.get("ProxyAutoDiscoveryEnable") == Some(&"1")
    {
        return Err(
            "当前只支持系统手动代理，无法解析 PAC；请在代理软件中启用 HTTP 或 SOCKS 系统代理"
                .into(),
        );
    }
    Ok(None)
}

pub(super) fn system_proxy() -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/sbin/scutil")
            .arg("--proxy")
            .output()
            .map_err(|_| "无法读取 macOS 系统代理设置")?;
        if !output.status.success() {
            return Err("读取 macOS 系统代理失败".into());
        }
        parse_system_proxy(&String::from_utf8_lossy(&output.stdout))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_enabled_https_proxy_and_ignores_disabled_entries() {
        assert_eq!(parse_system_proxy("HTTPSEnable : 1\nHTTPSProxy : 127.0.0.1\nHTTPSPort : 7897\nHTTPEnable : 1\nHTTPProxy : other\nHTTPPort : 8080").unwrap().as_deref(), Some("http://127.0.0.1:7897"));
        assert_eq!(
            parse_system_proxy("HTTPSEnable : 0\nHTTPSProxy : stale\nHTTPSPort : 7897").unwrap(),
            None
        );
        assert_eq!(
            parse_system_proxy("SOCKSEnable : 1\nSOCKSProxy : 127.0.0.1\nSOCKSPort : 1080")
                .unwrap()
                .as_deref(),
            Some("socks5://127.0.0.1:1080")
        );
    }
    #[test]
    fn rejects_invalid_and_unsupported_proxy_instead_of_silent_direct() {
        for raw in [
            "HTTPEnable : 1\nHTTPProxy : 127.0.0.1\nHTTPPort : 0",
            "HTTPEnable : 1",
            "ProxyAutoConfigEnable : 1",
        ] {
            assert!(parse_system_proxy(raw).is_err());
        }
        assert_eq!(parse_system_proxy("<dictionary> {\n}").unwrap(), None);
    }
}
