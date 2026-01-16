fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    #[cfg(feature = "build_wasm")]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        match std::process::Command::new("cargo")
            .args([
                "build",
                "--release",
                "--target-dir",
                &out_dir,
                "--target",
                "wasm32-unknown-unknown",
                "--no-default-features",
            ])
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

        let mut bindgen = wasm_bindgen_cli_support::Bindgen::new();
        bindgen
            .input_path(format!(
                "{}/wasm32-unknown-unknown/release/busperf_web.wasm",
                out_dir
            ))
            .web(true)
            .expect("We are setting only one target, so this should not fail")
            .typescript(false);
        if let Err(e) = bindgen.generate(format!("{}/", out_dir)) {
            panic!("wasm-bindgen failed: {e}");
        }
    }
}
