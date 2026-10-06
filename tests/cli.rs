use std::io::Write;
use std::process::{Command, Stdio};

fn convert(target: &str, input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ct"))
        .args([target, "-i", "-", "-o"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn unicode_markdown_can_be_prepared_for_a_rich_editor() {
    let html = convert("rich", "# Réunion ☕\n\n**Important**\n\n- Premier\n- Second\n");
    assert!(html.contains("Réunion ☕</h1>"));
    assert!(html.contains("<strong>Important</strong>"));
    assert!(html.contains("<li>Premier</li>"));
}

#[test]
fn plain_and_text_are_distinct_and_keep_signature_line_breaks() {
    let input = "**Bonjour**\\\nJohn\\\nParis\n";
    assert_eq!(convert("text", input), input);
    assert_eq!(convert("plain", input), "Bonjour\nJohn\nParis\n");
}
