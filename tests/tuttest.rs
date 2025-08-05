use std::process::Command;

#[test]
fn tuttest() {
    let out = Command::new("tuttest")
        .args(["README.md", "example*"])
        .output()
        .unwrap();
    for line in String::from_utf8(out.stdout).unwrap().lines() {
        if line != "" {
            let words: Vec<&str> = line.split_whitespace().collect();
            println!("{line}");
            let out = Command::new(words[0]).args(&words[1..]).output().unwrap();
            if !out.status.success() {
                panic!("Failed to run {}\n {out:?}", line);
            }
        }
    }
}
