use std::io::ErrorKind;

fn create_symlink(plugins_dir: &str, out_dir: &str) {
    match symlink::symlink_dir(&plugins_dir, &out_dir) {
        Ok(_) => (),
        Err(e) => {
            if e.kind() == ErrorKind::AlreadyExists {
                symlink::remove_symlink_dir(&out_dir)
                    .expect("To update the symlink we should remove the old one");
                symlink::symlink_dir(&plugins_dir, &out_dir)
                    .expect("Now we should be able to create the symlink");
            } else {
                // Any other error should result in a panic!
                panic!("Failed to create symlink to plugins {:?}", e);
            }
        }
    }
}

fn main() {
    let plugins_dir = format!(
        "{}/{}",
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        "plugins"
    );
    let out_dir = format!(
        "{}/target/{}/{}",
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        std::env::var("PROFILE").unwrap(),
        "plugins"
    );
    create_symlink(&plugins_dir, &out_dir);

    if std::env::var("PROFILE").unwrap() == "debug" {
        let out_dir = format!(
            "{}/target/debug/deps/{}",
            std::env::var("CARGO_MANIFEST_DIR").unwrap(),
            "plugins"
        );
        create_symlink(&plugins_dir, &out_dir);
    }
}
