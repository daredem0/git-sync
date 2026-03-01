//! Integration test that executes the receive behavior matrix script.

use std::path::PathBuf;
use std::process::Command;

fn output_text(stdout: &[u8], stderr: &[u8]) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
}

// Verifies that the scripted receive matrix passes end-to-end and prints the summary marker.
#[test]
fn integration_receive_matrix_script_passes() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir.join("scripts/test-receive-integration-matrix.sh");
    assert!(
        script_path.exists(),
        "receive matrix script must exist at {}",
        script_path.display()
    );

    let output = Command::new("bash")
        .arg(script_path.to_string_lossy().to_string())
        .env("KEEP_TMP", "0")
        .current_dir(&manifest_dir)
        .output()
        .expect("failed to execute receive matrix script");

    if !output.status.success() {
        panic!(
            "receive matrix script failed\nexit={}\n{}",
            output.status,
            output_text(&output.stdout, &output.stderr)
        );
    }

    let text = output_text(&output.stdout, &output.stderr);
    assert!(
        text.contains("[PASS] All receive integration matrix cases completed"),
        "script should print final PASS summary marker"
    );
}
