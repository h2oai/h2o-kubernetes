use assert_cmd::assert::Assert;
use assert_cmd::Command;

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
    let mut deploy_cmd = Command::cargo_bin("h2ok").unwrap();
    let assert_deploy: Assert = deploy_cmd.args(&["deploy", "--cluster_size", "1", "--kubeconfig", env!("KUBECONFIG")])
        .assert();

    let output: Vec<u8> = assert_deploy.success()
        .code(0)
        .stdout(predicates::str::is_match(".*\\.h2ok").unwrap())
        .get_output().clone().stdout;

    let deployment_filename = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), String::from_utf8(output).unwrap().trim());

    let mut ingress_cmd: Command = Command::cargo_bin("h2ok").unwrap();
    let assert_ingress = ingress_cmd.args(&["ingress", "-f", &deployment_filename]).assert();

    assert_ingress.code(0)
        .success();

    let mut undeploy_cmd : Command = Command::cargo_bin("h2ok").unwrap();
    let assert_undeploy: Assert = undeploy_cmd.args(&["undeploy", "-f", &deployment_filename])
        .assert();

    assert_undeploy.success()
        .code(0)
        .stdout(predicates::str::is_match("Removed deployment 'h2o-\\.*").unwrap());
}

/// Test if output of `deploy` command is properly accepted by the `undeploy` command.
/// Output of `deploy` command (if successful) is filename of the deployment descriptor persisted.
#[test]
fn test_undeploy_piping() {
    let mut deploy_cmd = Command::cargo_bin("h2ok").unwrap();
    let assert_deploy: Assert = deploy_cmd.args(&["deploy", "--cluster_size", "1", "--kubeconfig", env!("KUBECONFIG")])
        .assert();

    let output = assert_deploy.success()
        .code(0)
        .stdout(predicates::str::is_match(".*\\.h2ok").unwrap())
        .get_output().clone().stdout;

    let deployment_filename = String::from_utf8(output).unwrap();

    let mut undeploy_cmd = Command::cargo_bin("h2ok")
        .unwrap();
    undeploy_cmd.write_stdin(deployment_filename);

    let assert_undeploy: Assert = undeploy_cmd.args(&["undeploy"])
        .assert();

    assert_undeploy.success()
        .code(0)
        .stdout(predicates::str::is_match("Removed deployment 'h2o-\\.*").unwrap());
}

/// Test if output of `deploy` command is properly accepted by the `undeploy` command.
/// Output of `deploy` command (if successful) is filename of the deployment descriptor persisted.
#[test]
fn test_undeploy_missing_deployment_descriptor() {
    let mut undeploy_cmd = Command::cargo_bin("h2ok").unwrap();
    undeploy_cmd.write_stdin("nonexistent_file");

    let assert_undeploy: Assert = undeploy_cmd.args(&["undeploy"]).assert();

    assert_undeploy.failure()
        .code(1)
        .stderr(predicates::str::is_match(r#"Unable to process user input: UserInputError \{ kind: UnreachableDeploymentDescriptor \}"#).unwrap());
}
