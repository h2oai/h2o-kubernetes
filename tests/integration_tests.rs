use assert_cmd::assert::Assert;
use assert_cmd::Command;

#[test]
fn test_general_help() {
    let mut cmd = Command::cargo_bin("h2ok").unwrap();
    let assert: Assert = cmd.arg("-h")
        .assert();

    let expected_output_pattern: &str = r#"H2O Kubernetes CLI \d+.\d+.\d+.*

USAGE:
    h2ok \[SUBCOMMAND\]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    deploy      Deploys an H2O cluster into Kubernetes. Once successfully deployed a deployment descriptor file with
                cluster name is saved.Such a file can be used to undeploy the cluster or built on top of by adding
                additional services.*
    help        Prints this message or the help of the given subcommand\(s\).*
    undeploy    Undeploys an existing H2O cluster from Kubernetes"#;

    assert.success()
        .code(0)
        .stdout(predicates::str::is_match(expected_output_pattern).unwrap());
}

#[test]
fn test_deployment_help() {
    let mut cmd = Command::cargo_bin("h2ok").unwrap();
    let assert: Assert = cmd.args(&["deploy", "-h"])
        .assert();

    let expected_output_pattern: &str = r#"h2ok-deploy.*
Deploys an H2O cluster into Kubernetes\. Once successfully deployed a deployment descriptor file with cluster name is
saved\.Such a file can be used to undeploy the cluster or built on top of by adding additional services\.

USAGE:
    h2ok deploy \[OPTIONS\] --cluster_size <cluster_size>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -s, --cluster_size <cluster_size>              Number of H2O Nodes in the cluster. Up to 2\^32.
        --cpus <cpus>                              Number of CPUs allocated for each H2O node. \[default: 1\]
    -k, --kubeconfig <kubeconfig>
            Path to 'kubeconfig' yaml file\. If not specified, well-known locations are scanned for kubeconfig\.

    -m, --memory <memory>
            Amount of memory allocated by each H2O node - in a format accepted by K8S, e.g. 4Gi. \[default: 1Gi\]

    -p, --memory_percentage <memory_percentage>
            Memory percentage allocated by H2O inside the container. <0,100>. Defaults to 50% to make space for XGBoost.
            \[default: 50\]
    -c, --cluster_name <name>
            Name of the H2O cluster deployment. Used as prefix for K8S entities. Generated if not specified.

    -n, --namespace <namespace>                    Kubernetes cluster namespace to connect to. \[default: default\]"#;

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
Undeploys an existing H2O cluster from Kubernetes

USAGE:
    h2ok undeploy --file <file>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f, --file <file>    H2O deployment descriptor file path\.
"#;

    assert.success()
        .code(0)
        .stdout(predicates::str::is_match(expected_output_pattern).unwrap());
}


#[test]
fn test_deploy_undeploy(){
        let mut deploy_cmd = Command::cargo_bin("h2ok").unwrap();
        let assert_deploy: Assert = deploy_cmd.args(&["deploy", "--cluster_size", "1", "--kubeconfig", env!("KUBECONFIG")])
            .assert();

        let output = assert_deploy.success()
            .code(0)
            .stdout(predicates::str::is_match(".*\\.h2ok").unwrap())
            .get_output().clone().stdout;

        let deployment_filename = format!("{}/{}",env!("CARGO_MANIFEST_DIR"),String::from_utf8(output).unwrap().trim());

        let mut undeploy_cmd = Command::cargo_bin("h2ok").unwrap();
        let assert_undeploy: Assert = undeploy_cmd.args(&["undeploy", "-f", &deployment_filename])
            .assert();

        assert_undeploy.success()
            .code(0)
            .stdout(predicates::str::is_match("Removed deployment 'h2o-\\.*").unwrap());

}
