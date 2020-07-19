use std::path::Path;

use clap::{App, Arg, ArgMatches, SubCommand};

const APP_NAME: &str = "H2O Kubernetes CLI";
const APP_VERSION: &str = "0.1.0";

pub fn parse_arguments<'a>() -> ArgMatches<'a> {
    let app: App = build_app();
    return app.get_matches();
}

fn build_app<'a>() -> App<'a, 'a> {
    return App::new(APP_NAME)
        .version(APP_VERSION)
        .subcommand(SubCommand::with_name("deploy")
            .arg(Arg::with_name("cluster_size")
                .required(true)
                .long("cluster_size")
                .short("s")
                .help("Number of H2O Nodes in the cluster. Up to 2^32.")
                .number_of_values(1)
                .validator(self::validate_greater_than_zero))
            .arg(Arg::with_name("kubeconfig")
                .long("kubeconfig")
                .short("k")
                .number_of_values(1)
                .validator(self::validate_path)
                .help("Path to 'kubeconfig' yaml file.")
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
                .help("Name of the H2O cluster deployment. Used as prefix for K8S entities.")
                .number_of_values(1)))
        .subcommand(SubCommand::with_name("undeploy")
            .arg(Arg::with_name("file")
                .long("file")
                .short("f")
                .number_of_values(1)
                .required(true)
                .help("File with H2O deployment details to undeploy.")
                .validator(self::validate_path)
            ));
}

fn validate_path(user_provided_path: String) -> Result<(), String> {
    return if Path::new(&user_provided_path).is_file() {
        Result::Ok(())
    } else {
        Result::Err(String::from(format!("Invalid file path: '{}'", user_provided_path)))
    };
}

fn validate_greater_than_zero(input: String) -> Result<(), String> {
    let number: i32 = input.parse::<i32>().unwrap();
    if number < 1 {
        return Result::Err("".to_string());
    } else {
        return Result::Ok(());
    }
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
}
