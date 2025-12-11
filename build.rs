fn main() {
    #[cfg(feature = "generate-html")]
    {
        if !std::process::Command::new("cargo")
            .args([
                "build",
                "--target-dir",
                "../target_wasm",
                "--target",
                "wasm32-unknown-unknown",
            ])
            .current_dir("busperf_web")
            .status()
            .unwrap()
            .success()
            || !std::process::Command::new("wasm-bindgen")
                .args([
                    "--target",
                    "web",
                    "--out-dir",
                    "target_wasm",
                    "target_wasm/wasm32-unknown-unknown/debug/busperf_web.wasm",
                ])
                .status()
                .unwrap()
                .success()
        {
            panic!("Failed to compile wasm target");
        }
    }
}
