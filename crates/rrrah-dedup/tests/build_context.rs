#[path = "../build_context.rs"]
mod build_context;

#[test]
fn native_build_inputs_and_previously_absent_target_overrides_change_context() {
    let target = "aarch64-apple-darwin";
    let baseline = build_context::context(target, []);
    for key in [
        "CC",
        "CXXFLAGS",
        "SDKROOT",
        "LIBCLANG_PATH",
        "BINDGEN_EXTRA_CLANG_ARGS",
        "BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_darwin",
        "PKG_CONFIG_LIBDIR",
        "CC_aarch64-apple-darwin",
        "CFLAGS_aarch64_apple_darwin",
        "CARGO_FEATURE_DECODE",
    ] {
        let changed = build_context::context(target, [(key.to_owned(), "distinct-input".to_owned())]);
        assert_ne!(changed, baseline, "{key}");
    }
    let watched = build_context::watched_keys(target);
    assert!(watched.contains(&"CC_aarch64-apple-darwin".to_owned()));
    assert!(watched.contains(&"CC_aarch64_apple_darwin".to_owned()));
}

#[test]
fn context_is_order_independent_and_excludes_unrelated_environment() {
    let entries = [
        ("CC".to_owned(), "clang".to_owned()),
        ("SDKROOT".to_owned(), "sdk".to_owned()),
    ];
    let forward = build_context::context("target", entries.clone());
    let reversed = build_context::context("target", entries.into_iter().rev());
    assert_eq!(forward, reversed);
    assert_eq!(
        build_context::context(
            "target",
            [("UNRELATED_PRIVATE_VALUE".to_owned(), "ignored".to_owned())]
        ),
        build_context::context("target", [])
    );
}
