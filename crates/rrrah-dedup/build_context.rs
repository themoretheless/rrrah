//! Build inputs that can change native decoding or color conversion.
pub fn watched_keys(target: &str) -> Vec<String> {
    let mut keys = [
        "TARGET",
        "HOST",
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "CARGO_ENCODED_RUSTFLAGS",
        "CC",
        "CXX",
        "AR",
        "CFLAGS",
        "CXXFLAGS",
        "ARFLAGS",
        "CPPFLAGS",
        "HOST_CC",
        "HOST_CXX",
        "HOST_AR",
        "HOST_CFLAGS",
        "HOST_CXXFLAGS",
        "TARGET_CC",
        "TARGET_CXX",
        "TARGET_AR",
        "TARGET_CFLAGS",
        "TARGET_CXXFLAGS",
        "LIBCLANG_PATH",
        "BINDGEN_EXTRA_CLANG_ARGS",
        "SDKROOT",
        "MACOSX_DEPLOYMENT_TARGET",
        "IPHONEOS_DEPLOYMENT_TARGET",
        "PKG_CONFIG",
        "PKG_CONFIG_PATH",
        "PKG_CONFIG_LIBDIR",
        "PKG_CONFIG_SYSROOT_DIR",
        "PKG_CONFIG_ALLOW_CROSS",
        "PKG_CONFIG_ALL_STATIC",
        "PKG_CONFIG_ALL_DYNAMIC",
        "CC_FORCE_DISABLE",
        "CRATE_CC_NO_DEFAULTS",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for base in [
        "BINDGEN_EXTRA_CLANG_ARGS",
        "CC",
        "CXX",
        "AR",
        "CFLAGS",
        "CXXFLAGS",
        "ARFLAGS",
        "CPPFLAGS",
        "PKG_CONFIG",
        "PKG_CONFIG_PATH",
        "PKG_CONFIG_LIBDIR",
        "PKG_CONFIG_SYSROOT_DIR",
    ] {
        for suffix in [target.to_owned(), target.replace('-', "_")] {
            keys.push(format!("{base}_{suffix}"));
        }
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

pub fn context(target: &str, values: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let mut values = values.into_iter().collect::<std::collections::BTreeMap<_, _>>();
    let mut result = watched_keys(target)
        .into_iter()
        .map(|key| {
            let value = values.remove(&key).unwrap_or_default();
            (key, value)
        })
        .collect::<Vec<_>>();
    result.extend(
        values
            .into_iter()
            .filter(|(key, _)| key.starts_with("CARGO_FEATURE_")),
    );
    result.sort_unstable();
    result
}
