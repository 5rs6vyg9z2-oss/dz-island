pub const REPO: &str = "5rs6vyg9z2-oss/dz-island";
pub const ASSET: &str = "DZ-Island-portable-windows-x64.zip";
pub const LEGACY_ASSET: &str = "DZ-Island-windows-x64.zip";
pub const MAX_PACKAGE: usize = 150 * 1024 * 1024;
pub fn version(raw: &str) -> Option<[u64; 3]> {
    let parts: Vec<_> = raw.strip_prefix('v').unwrap_or(raw).split('.').collect();
    if parts.len() != 3 { return None; }
    let mut result = [0; 3];
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) { return None; }
        result[i] = part.parse().ok()?;
    }
    Some(result)
}
pub fn valid_package(name: &str, url: &str, digest: Option<&str>, size: usize) -> bool {
    matches!(name, ASSET | LEGACY_ASSET) && size > 0 && size <= MAX_PACKAGE
        && url.starts_with(&format!("https://github.com/{REPO}/releases/download/"))
        && digest.and_then(|d| d.strip_prefix("sha256:"))
            .is_some_and(|d| d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn numeric_ordering() { assert!(version("v1.11.10") > version("1.11.9")); assert!(version("1.12.0") > version("1.11.999")); }
    #[test] fn unsafe_and_prerelease_versions_rejected() {
        for value in ["1.2", "1.2.3-beta", "1.2.3/../x", "1.-2.3", "999999999999999999999999.2.3"] { assert!(version(value).is_none()); }
    }
    #[test] fn other_repositories_and_plain_http_rejected() {
        let digest=format!("sha256:{}", "a".repeat(64));
        for url in ["https://github.com/other/repo/releases/download/v1/x.zip", "https://github.com.evil.example/5rs6vyg9z2-oss/dz-island/releases/download/v1/x.zip", "http://github.com/5rs6vyg9z2-oss/dz-island/releases/download/v1/x.zip"] { assert!(!valid_package(ASSET,url,Some(&digest),123)); }
    }
    #[test] fn digest_and_size_required() {
        let url=format!("https://github.com/{REPO}/releases/download/v1.11.1/{ASSET}");
        assert!(!valid_package(ASSET,&url,None,123));
        assert!(!valid_package(ASSET,&url,Some("sha256:bad"),123));
        let digest=format!("sha256:{}", "a".repeat(64));
        assert!(valid_package(ASSET,&url,Some(&digest),123));
        assert!(!valid_package(ASSET,&url,Some(&digest),MAX_PACKAGE+1));
        assert!(!valid_package(ASSET,&url,Some(&digest),0));
    }
}
