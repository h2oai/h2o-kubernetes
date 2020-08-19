use std::path::{Path, PathBuf};

use clap::{App, Arg, ArgMatches, SubCommand};
use names::Generator;
use num::Num;
use regex::Regex;

use crate::k8s::DeploymentSpecification;
use std::str::FromStr;

const APP_NAME: &str = "H2O Kubernetes CLI";
const APP_VERSION: &str = "0.1.0";

/// Extracts user-provided arguments and builds a `Command` out of user input.
pub fn get_command() -> Command {
    let app: App = build_app();
    let args: ArgMatches = app.get_matches();

    if let Some(deploy_args) = args.subcommand_matches("deploy") {
        let deployment_name: String = extract_string(deploy_args, "name").unwrap_or_else(|| {
            let mut generator: Generator = Generator::default();
            return format!("h2o-{}", generator.next().unwrap());
        });
        // Args below have defaults, it is therefore safe to unwrap.
        let namespace: String = extract_string(deploy_args, "namespace").unwrap();
        let cluster_size: u32 = extract_num(deploy_args, "cluster_size").unwrap();
        let jvm_memory_percentage: u8 = extract_num(deploy_args, "memory_percentage").unwrap();
        let memory: String = extract_string(deploy_args, "memory").unwrap();
        let num_cpus: u32 = extract_num(deploy_args, "cpus").unwrap();
        let kubeconfig_path: Option<PathBuf> = match extract_string(deploy_args, "kubeconfig") {
            None => { Option::None }
            Some(kubeconfig) => { Some(PathBuf::from(kubeconfig)) }
        };

        let deployment: DeploymentSpecification = DeploymentSpecification::new(deployment_name, namespace, jvm_memory_percentage,
                                                                               memory, num_cpus, cluster_size, kubeconfig_path);
        return Command::Deployment(deployment);
    } else if let Some(undeploy_args) = args.subcommand_matches("undeploy") {
        match undeploy_args.value_of("file") {
            None => { panic!("Deployment file undefined.") }
            Some(file) => { return Command::Undeploy(PathBuf::from(file)); }
        };
    } else {
        panic!("Unknown command.");
    }
}

/// Commands issuable by the user.
pub enum Command {
    Deployment(DeploymentSpecification),
    Undeploy(PathBuf),
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
                .help("Kubernetes cluster namespace to connect to.")
                .number_of_values(1)
                .default_value("default")
            )
            .arg(Arg::with_name("name")
                .long("cluster_name")
                .short("c")
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
        )
        .subcommand(SubCommand::with_name("undeploy")
            .about("Undeploys an existing H2O cluster from Kubernetes")
            .arg(Arg::with_name("file")
                .long("file")
                .short("f")
                .number_of_values(1)
                .required(true)
                .help("H2O deployment descriptor file path.")
                .validator(self::validate_path)
            ));
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
    use clap::{App, ArgMatches};

    use crate::tests::kubeconfig_location_panic;

    #[test]
    fn test_kubeconfig_path() {
        let kubeconfig_location: String = kubeconfig_location_panic();

        // Existing kubeconfig
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok", "deploy", "--kubeconfig", kubeconfig_location.as_str(), "--cluster_size", "1"];
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

    #[test]
    fn test_namespace() {
        // No namespace provided - use "default" default :)
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok", "deploy", "--cluster_size", "1"];
        let matches: ArgMatches = app.get_matches_from(args_with_kubeconfig);
        let deploy: &ArgMatches = matches.subcommand_matches("deploy").unwrap();
        assert_eq!("default", deploy.value_of("namespace").unwrap());

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