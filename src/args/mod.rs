use std::path::Path;

use clap::{App, Arg, ArgMatches};

const APP_NAME: &str = "H2O Kubernetes CLI";
const APP_VERSION: &str = "0.1.0";

pub fn parse_arguments<'a>() -> ArgMatches<'a> {
    let app: App = build_app();
    return app.get_matches();
}

fn build_app<'a>() -> App<'a, 'a> {
    return App::new(APP_NAME)
        .version(APP_VERSION)
        .arg(Arg::with_name("kubeconfig")
            .long("kubeconfig")
            .short("k")
            .number_of_values(1)
            .validator(self::validate_kubeconfig_path)
            .help("Path to 'kubeconfig' yaml file.")
        ).arg(Arg::with_name("namespace")
        .long("namespace")
        .short("n")
        .help("Kubernetes cluster namespace to connect to.")
        .number_of_values(1)
        .default_value("default")
    );
}

fn validate_kubeconfig_path(user_provided_path: String) -> Result<(), String> {
    return if Path::new(&user_provided_path).is_file() {
        Result::Ok(())
    } else {
        Result::Err(String::from(format!("Invalid kubeconfig file: '{}'", user_provided_path)))
    }
}


#[cfg(test)]
mod tests {
    use std::env;

    use clap::{App, ArgMatches};

    const TEST_KUBECONFIG_ENVVAR: &str = "KUBECONFIG";

    #[test]
    fn test_kubeconfig_path() {
        let kubeconfig_location: String = match env::var(TEST_KUBECONFIG_ENVVAR) {
            Ok(var) => { var },
            Err(err) => {
                panic!("Environment variable {} not defined.\
            Unable to construct Kubernetes client. Error: {}", TEST_KUBECONFIG_ENVVAR, err);
            },
        };

        // Existing kubeconfig
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok", "--kubeconfig", kubeconfig_location.as_str()];
        let matches: ArgMatches = app.get_matches_from(args_with_kubeconfig);
        assert!(matches.is_present("kubeconfig"));

        // No kubeconfig provided - default value provided
        let app: App = super::build_app();
        let args_no_kubeconfig: Vec<&str> = vec!["h2ok"];
        let matches: ArgMatches = app.get_matches_from(args_no_kubeconfig);
        assert!(!matches.is_present("kubeconfig"));
    }

    #[test]
    fn test_namespace() {
        // No namespace provided - use "default" default :)
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok"];
        let matches: ArgMatches = app.get_matches_from(args_with_kubeconfig);
        assert_eq!("default", matches.value_of("namespace").unwrap());

        // Custom namespace provided
        let app: App = super::build_app();
        let args_with_kubeconfig: Vec<&str> = vec!["h2ok", "--namespace", "non-default"];
        let matches: ArgMatches = app.get_matches_from(args_with_kubeconfig);
        assert_eq!("non-default", matches.value_of("namespace").unwrap())
    }
}