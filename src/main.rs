mod args;
mod k8s;

extern crate clap;

use clap::ArgMatches;
use std::path::Path;
use kube::Client;


fn main() {
    let args : ArgMatches = args::parse_arguments();
    let kubeconfig: &str = args.value_of("kubeconfig").unwrap();
    println!("Using kubeconfig at the following location: {}", kubeconfig);
    let client : Client = k8s::from_kubeconfig(Path::new(kubeconfig));
    k8s::deploy_h2o(client, args.value_of("namespace").unwrap_or("default"));

}