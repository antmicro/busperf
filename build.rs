fn main() {
    #[cfg(feature = "generate-html")]
    {
        match std::process::Command::new("cargo")
            .args([
                "build",
                "--release",
                "--target-dir",
                "../target_wasm",
                "--target",
                "wasm32-unknown-unknown",
            ])
            .current_dir("busperf_web")
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    panic!(
                        "WASM compile failed\n{}",
                        String::from_utf8(output.stderr).unwrap()
                    );
                }
            }
            Err(e) => {
                panic!("Cargo could not be run {e}")
            }
        }
        match std::process::Command::new("wasm-bindgen")
            .args([
                "--target",
                "web",
                "--out-dir",
                "target_wasm",
                "target_wasm/wasm32-unknown-unknown/release/busperf_web.wasm",
            ])
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    panic!(
                        "Wasm bindgen failed: {}",
                        String::from_utf8(output.stderr).unwrap()
                    );
                }
            }
            Err(e) => {
                panic!("Failed to run wasm bindgen: {e}");
            }
        }
        println!("cargo::rerun-if-changed=target_wasm")
    }
}
