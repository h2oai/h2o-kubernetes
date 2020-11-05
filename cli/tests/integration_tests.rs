extern crate tests_common;
use std::path::PathBuf;

use assert_cmd::assert::Assert;
use assert_cmd::Command;
use names::Generator;

const EXPECTED_GENERAL_HELP: &str = r#"H2O Kubernetes CLI \d+.\d+.\d+.*"#;

#[test]
fn test_general_help() {
    let mut cmd = Command::cargo_bin("h2ok").unwrap();
    let assert: Assert = cmd.arg("-h")
        .assert();
    assert.success()
        .code(0)
        .stdout(predicates::str::is_match(EXPECTED_GENERAL_HELP).unwrap());
}

/// If no `-h` or `-help` or any subcommand is provided to `h2ok`, general help should be displayed.
#[test]
fn test_general_help_no_flag() {
    let mut cmd = Command::cargo_bin("h2ok").unwrap();
    let assert: Assert = cmd.assert();

    assert.failure()
        .stderr(predicates::str::is_match(EXPECTED_GENERAL_HELP).unwrap());
}

#[test]
fn test_deployment_help() {
    let mut cmd = Command::cargo_bin("h2ok").unwrap();
    let assert: Assert = cmd.args(&["deploy", "-h"])
        .assert();

    let expected_output_pattern: &str = r#"h2ok-deploy.*"#;

    assert.success()
        .code(0)
        .stdout(predicates::str::is_match(expected_output_pattern).unwrap());
}

#[test]
fn test_undeploy_help() {
    let mut cmd = Command::cargo_bin("h2ok").unwrap();
    let assert: Assert = cmd.args(&["undeploy", "-h"])
        .assert();

    let expected_output_pattern: &str = r#"h2ok-undeploy.*
Undeploys an existing H2O cluster from Kubernetes.*"#;

    assert.success()
        .code(0)
        .stdout(predicates::str::is_match(expected_output_pattern).unwrap());
}


#[test]
fn test_deploy_undeploy() {
    let kubeconfig_location: PathBuf = tests_common::kubeconfig_location_panic();
    let kubeconfig_location_str: &str = kubeconfig_location.to_str().unwrap();
    let name: String = Generator::default().next().unwrap();
    let mut deploy_cmd = Command::cargo_bin("h2ok").unwrap();
    let assert_deploy: Assert = deploy_cmd.args(&["deploy","--name", &name, "--cluster_size", "1", "--kubeconfig", kubeconfig_location_str, "--version", "latest"])
        .assert();

    assert_deploy.success()
        .code(0)
        .stdout(predicates::str::is_match(format!("To undeploy, use the 'h2ok undeploy {}' command.", &name)).unwrap());


    let mut ingress_cmd: Command = Command::cargo_bin("h2ok").unwrap();
    let assert_ingress = ingress_cmd.args(&["ingress", &name]).assert();

    assert_ingress.code(0)
        .success();

    let mut undeploy_cmd : Command = Command::cargo_bin("h2ok").unwrap();
    let assert_undeploy: Assert = undeploy_cmd.args(&["undeploy", &name])
        .assert();

    assert_undeploy.success()
        .code(0)
        .stdout(predicates::str::is_match(format!("Removed deployment '{}'", &name)).unwrap());
}
