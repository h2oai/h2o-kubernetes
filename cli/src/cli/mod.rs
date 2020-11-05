use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use names::Generator;
use num::Num;
use regex::Regex;


const APP_NAME: &str = "H2O Kubernetes CLI";
const APP_VERSION: &str = "0.1.0";

/// Extracts user-provided arguments and builds a `Command` out of user input.
pub fn get_command() -> Result<Command, UserInputError> {
    let app: App = build_app();
    let args: ArgMatches = app.get_matches();

    return if let Some(deploy_args) = args.subcommand_matches("deploy") {
        Ok(Command::Deployment(new_deployment(deploy_args)))
    } else if let Some(undeploy_args) = args.subcommand_matches("undeploy") {
        Ok(Command::Undeploy(existing_deployment(undeploy_args)))
    } else if let Some(ingress_args) = args.subcommand_matches("ingress") {
        Ok(Command::Ingress(existing_deployment(ingress_args)))
    } else {
        Result::Err(UserInputError::new(CommandErrorKind::UnknownCommand))
    };
}

fn new_deployment(deploy_args: &ArgMatches) -> NewDeploymentSpecification {
    let deployment_name: String = extract_string(deploy_args, "name").unwrap_or_else(|| {
        // If no name is provided by the user, generate one
        let mut generator: Generator = Generator::default();
        return format!("h2o-{}", generator.next().unwrap());
    });
    let namespace: Option<String> = extract_string(deploy_args, "namespace");
    // Args below have defaults, it is therefore safe to unwrap.
    let cluster_size: u32 = extract_num(deploy_args, "cluster_size").unwrap();
    let jvm_memory_percentage: u8 = extract_num(deploy_args, "memory_percentage").unwrap();
    let memory: String = extract_string(deploy_args, "memory").unwrap();
    let num_cpus: u32 = extract_num(deploy_args, "cpus").unwrap();
    let kubeconfig_path: Option<PathBuf> = match extract_string(deploy_args, "kubeconfig") {
        None => { Option::None }
        Some(kubeconfig) => { Some(PathBuf::from(kubeconfig)) }
    };
    let version: Option<String> = extract_string(deploy_args, "version");
    let custom_image: Option<String> = extract_string(deploy_args, "image");
    let custom_command: Option<String> = extract_string(deploy_args, "command");

    NewDeploymentSpecification::new(deployment_name, namespace, version, jvm_memory_percentage,
                                    memory, num_cpus, cluster_size, kubeconfig_path, custom_image, custom_command)
}

fn existing_deployment(args: &ArgMatches) -> ExistingDeploymentSpecification {
    let name = extract_string(args, "name").unwrap_or_else(|| {
        panic!("Name of the H2O deployment must be provided.");
    });
    let namespace = extract_string(args, "namespace");
    let kubeconfig_path: Option<PathBuf> = match extract_string(args, "kubeconfig") {
        None => { Option::None }
        Some(kubeconfig) => { Some(PathBuf::from(kubeconfig)) }
    };

    ExistingDeploymentSpecification::new(name, namespace, kubeconfig_path)
}

/// Commands issuable by the user.
pub enum Command {
    Deployment(NewDeploymentSpecification),
    Undeploy(ExistingDeploymentSpecification),
    Ingress(ExistingDeploymentSpecification),
}

pub struct NewDeploymentSpecification {
    /// Name of the deployment. If not provided by the user, the value is randomly generated.
    pub name: String,
    /// Namespace to deploy to - if not provided, an attempt to search in well-known locations is made.
    pub namespace: Option<String>,
    /// Memory percentage to allocate by the JVM running H2O inside the docker container.
    pub memory_percentage: u8,
    /// Total memory for each H2O node. Effectively a pod memory request and limit.
    pub memory: String,
    /// Number of CPUs allocated for each H2O node. Effectively a pod CPU request and limit.
    pub num_cpu: u32,
    /// Total count of H2O nodes inside the cluster created.
    pub num_h2o_nodes: u32,
    /// Kubeconfig - provided optionally. There are well-known standardized locations to look for Kubeconfig, therefore optional.
    pub kubeconfig_path: Option<PathBuf>,
    /// H2O version to use, if not custom Docker image is defined.
    pub version: Option<String>,
    /// Custom docker image to deploy
    pub custom_image: Option<String>,
    /// Custom command for a custom Docker image, if defined. Otherwise noop.
    pub custom_command: Option<String>,
}

impl NewDeploymentSpecification {
    pub fn new(name: String, namespace: Option<String>, version: Option<String>, memory_percentage: u8, memory: String, num_cpu: u32, num_h2o_nodes: u32, kubeconfig_path: Option<PathBuf>, custom_image: Option<String>, custom_command: Option<String>) -> Self {
        NewDeploymentSpecification { name, namespace, version, memory_percentage, memory, num_cpu, num_h2o_nodes, kubeconfig_path, custom_image, custom_command }
    }
}

pub struct ExistingDeploymentSpecification {
    /// Name of the existing deployment.
    pub name: String,
    /// Optional namespace to look in for the deployment. If not specified, the default namespace from Kubeconfig will be used.
    pub namespace: Option<String>,
    /// Optional path to kubeconfig. If not specified, the `KUBECONFIG` env var is looked for + several other well known locations might be searched.
    pub kubeconfig_path: Option<PathBuf>,
}

impl ExistingDeploymentSpecification {
    pub fn new(name: String, namespace: Option<String>, kubeconfig_path: Option<PathBuf>) -> Self {
        ExistingDeploymentSpecification { name, namespace, kubeconfig_path }
    }
}


/// Error while processing user input.
#[derive(Debug)]
pub struct UserInputError {
    kind: CommandErrorKind,
}

impl UserInputError {
    pub fn new(kind: CommandErrorKind) -> Self {
        UserInputError { kind }
    }
}

#[derive(Debug)]
pub enum CommandErrorKind {
    UnknownCommand
}

/// Attempts to extract/parse a number from user-given argument. If the user did not provide
/// any value or the value has not default, returns Option::None. Panics if the argument can not be parsed.
fn extract_num<T: Num + FromStr>(args: &ArgMatches, arg_name: &str) -> Option<T> {
    return match args.value_of(arg_name) {
        None => {
            Option::None
        }
        Some(value) => {
            if let Ok(result) = value.parse::<T>() {
                Option::Some(result)
            } else {
                panic!("Unable to parse argument '{}'. Given value: '{}'", arg_name, value)
            }
        }
    };
}

/// Attempts to extract/parse a string from user-given argument. If the user did not provide
/// any value or the value has not default, returns Option::None. Panics if the argument can not be parsed.
fn extract_string(args: &ArgMatches, arg_name: &str) -> Option<String> {
    return match args.value_of(arg_name) {
        None => {
            Option::None
        }
        Some(value) => {
            Some(value.to_string())
        }
    };
}

/// Contains definition of all commands, arguments, flags and the respective default values and descriptions
/// This is the only source of truth for user-facing CLI.
fn build_app<'a>() -> App<'a, 'a> {
    return App::new(APP_NAME)
        .version(APP_VERSION)
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(SubCommand::with_name("deploy")
            .about("Deploys an H2O cluster into Kubernetes. Once successfully deployed a deployment descriptor file with cluster name is saved.\
             Such a file can be used to undeploy the cluster or built on top of by adding additional services.")
            .arg(Arg::with_name("cluster_size")
                .required(true)
                .long("cluster_size")
                .short("s")
                .help("Number of H2O Nodes in the cluster. Up to 2^32.")
                .number_of_values(1)
                .validator(self::validate_int_greater_than_zero))
            .arg(Arg::with_name("kubeconfig")
                .long("kubeconfig")
                .short("k")
                .number_of_values(1)
                .validator(self::validate_path)
                .help("Path to 'kubeconfig' yaml file. If not specified, well-known locations are scanned for kubeconfig.")
            )
            .arg(Arg::with_name("namespace")
                .long("namespace")
                .short("n")
                .help("Kubernetes cluster namespace to connect to. If not specified, kubeconfig default is used.")
                .number_of_values(1)
            )
            .arg(Arg::with_name("name")
                .long("name")
                .help("Name of the H2O cluster deployment. Used as prefix for K8S entities. Generated if not specified.")
                .number_of_values(1))
            .arg(Arg::with_name("memory_percentage")
                .long("memory_percentage")
                .short("p")
                .default_value("50")
                .help("Memory percentage allocated by H2O inside the container. <0,100>. Defaults to 50% to make space for XGBoost.")
                .validator(self::validate_percentage))
            .arg(Arg::with_name("memory")
                .long("memory")
                .short("m")
                .number_of_values(1)
                .default_value("1Gi")
                .help("Amount of memory allocated by each H2O node - in a format accepted by K8S, e.g. 4Gi.")
                .validator(self::validate_memory))
            .arg(Arg::with_name("cpus")
                .long("cpus")
                .number_of_values(1)
                .default_value("1")
                .help("Number of CPUs allocated for each H2O node.")
            )
            .arg(Arg::with_name("version")
                .short("v")
                .long("version")
                .number_of_values(1)
                .required_unless("image")
                .conflicts_with("image")
                .conflicts_with("command")
                .help("H2O version to deploy. Noop if custom image is defined.")
            )
            .arg(Arg::with_name("image")
                .short("i")
                .long("image")
                .number_of_values(1)
                .help("H2O version to deploy. Noop if custom image is defined.")
            )
            .arg(Arg::with_name("command")
                .long("command")
                .number_of_values(1)
                .help("Custom command for to use for the custom docker image on startup.")
            )
        )
        .subcommand(SubCommand::with_name("undeploy")
            .about("Undeploys an existing H2O cluster from Kubernetes")
            .arg(Arg::with_name("kubeconfig")
                .long("kubeconfig")
                .short("k")
                .number_of_values(1)
                .validator(self::validate_path)
                .help("Path to 'kubeconfig' yaml file. If not specified, well-known locations are scanned for kubeconfig.")
            )
            .arg(Arg::with_name("namespace")
                .long("namespace")
                .short("n")
                .help("Kubernetes cluster namespace to connect to. If not specified, kubeconfig default is used.")
                .number_of_values(1)
            )
            .arg(Arg::with_name("name")
                .index(1)
                .help("Name of the H2O cluster deployment. Used as prefix for K8S entities. Generated if not specified.")
                .number_of_values(1)))
        .subcommand(SubCommand::with_name("ingress")
            .about("Creates an ingress pointing to the given H2O K8S deployment")
            .arg(Arg::with_name("kubeconfig")
                .long("kubeconfig")
                .short("k")
                .number_of_values(1)
                .validator(self::validate_path)
                .help("Path to 'kubeconfig' yaml file. If not specified, well-known locations are scanned for kubeconfig.")
            )
            .arg(Arg::with_name("namespace")
                .long("namespace")
                .short("n")
                .help("Kubernetes cluster namespace to connect to. If not specified, kubeconfig default is used.")
                .number_of_values(1)
            )
            .arg(Arg::with_name("name")
                .index(1)
                .help("Name of the H2O cluster deployment. Used as prefix for K8S entities. Generated if not specified.")
                .number_of_values(1)));
}

/// Validates whether a file under a user-provided path exists.
fn validate_path(user_provided_path: String) -> Result<(), String> {
    return if Path::new(&user_provided_path).is_file() {
        Result::Ok(())
    } else {
        Result::Err(String::from(format!("Invalid file path: '{}'", user_provided_path)))
    };
}

/// Validates user input to be an integer greater than zero.
/// Returns Result::Ok if given String  contains an integer greater than zero, otherwise Err with error message.
fn validate_int_greater_than_zero(input: String) -> Result<(), String> {
    let number: i64 = input.parse::<i64>().unwrap();
    return if number < 1 {
        Result::Err("Error: The number provided must be greater than zero.".to_string())
    } else {
        Result::Ok(())
    };
}

/// Validates if user's input is a number in an expected range.
///
/// # Arguments
///  * `input` User's input in String
///
fn validate_percentage(input: String) -> Result<(), String> {
    let number: i64 = input.parse::<i64>().unwrap();
    return if number < 0 || number > 100 {
        Result::Err(format!("Error: The number must be withing range <{},{}>.", 0, 100))
    } else {
        Result::Ok(())
    };
}

const MEMORY_PATTERN: &str = "^([+-]?[0-9.]+)([eEinumkKMGTP]*[-+]?[0-9]*)$";

/// Validates memory input from user. The pattern the input is matched against is the same pattern K8S uses.
fn validate_memory(input: String) -> Result<(), String> {
    let memory_regexp = Regex::new(MEMORY_PATTERN).unwrap();

    return if memory_regexp.is_match(&input) {
        Result::Ok(())
    } else {
        Result::Err(format!("Memory requirement must match the following pattern: {}. For example 1Gi or 1024Mi.", MEMORY_PATTERN))
    };
}


#[cfg(test)]
mod tests {
    extern crate tests_common;

    use std::path::PathBuf;
    use clap::{App, ArgMatches};
    use tests_common::kubeconfig_location_panic;

    #[test]
    fn test_kubeconfig_path() {
        let kubeconfig_location: PathBuf = kubeconfig_location_panic();
        let kubeconfig_location: &str = kubeconfig_location.to_str().unwrap();

        // Existing kubeconfig
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok", "deploy", "--kubeconfig", kubeconfig_location, "--cluster_size", "1"];
        let matches: ArgMatches = app.get_matches_from(args_with_kubeconfig);
        let deploy: &ArgMatches = matches.subcommand_matches("deploy").unwrap();
        assert!(deploy.is_present("kubeconfig"));

        // No kubeconfig provided - default value provided
        let app: App = super::build_app();
        let args_no_kubeconfig: Vec<&str> = vec!["h2ok", "deploy", "--cluster_size", "1"];
        let matches: ArgMatches = app.get_matches_from(args_no_kubeconfig);
        let deploy: &ArgMatches = matches.subcommand_matches("deploy").unwrap();
        assert!(!deploy.is_present("kubeconfig"));
    }

    // Defining version and custom image does not play well together, as custom image overrides custom version of the official image.
    // CLI should force user to only declare one of those.
    #[test]
    fn test_version_custom_image_conflict() {
        let kubeconfig_location: PathBuf = kubeconfig_location_panic();
        let kubeconfig_location: &str = kubeconfig_location.to_str().unwrap();

        // Existing kubeconfig
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok", "deploy", "--kubeconfig", kubeconfig_location, "--cluster_size", "1", "--version", "3.32.0.1", "--image", "nonexisting-image:3.32.0.1"];
        let matches = app.get_matches_from_safe(args_with_kubeconfig);
        assert!(matches.is_err());
        let erroneous_fields: Vec<String> = matches.err().unwrap().info.unwrap();
        assert_eq!(2, erroneous_fields.len());
        assert_eq!("image", erroneous_fields.get(0).unwrap());
        assert_eq!("--version <version>", erroneous_fields.get(1).unwrap());
    }

    #[test]
    fn test_namespace() {
        // No namespace provided - use "default" default :)
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok", "deploy", "--cluster_size", "1"];
        let matches: ArgMatches = app.get_matches_from(args_with_kubeconfig);
        let deploy: &ArgMatches = matches.subcommand_matches("deploy").unwrap();
        assert!(deploy.value_of("namespace").is_none());

        // Custom namespace provided
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok", "deploy", "--namespace", "non-default", "--cluster_size", "1"];
        let matches: ArgMatches = app.get_matches_from(args_with_kubeconfig);
        let deploy: &ArgMatches = matches.subcommand_matches("deploy").unwrap();
        assert_eq!("non-default", deploy.value_of("namespace").unwrap())
    }

    #[test]
    fn validate_number_range() {
        assert!(super::validate_percentage("10".to_string()).is_ok());
        assert!(super::validate_percentage("101".to_string()).is_err());
    }
}