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
            .help("Path to 'kubeconfig' yaml file.")
        ).arg(Arg::with_name("namespace")
        .long("namespace")
        .short("n")
        .help("Kubernetes cluster namespace to connect to.")
        .number_of_values(1)
        .default_value("default")
    );
}