extern crate clap;

use std::path::Path;

use clap::ArgMatches;
use kube::Client;

mod args;
mod k8s;
#[cfg(test)]
mod tests;

fn main() {
    let args: ArgMatches = args::parse_arguments();
    if let Some(deploy_args) = args.subcommand_matches("deploy"){
        deploy(deploy_args);
    }

}

fn deploy(deploy_args: &ArgMatches){
    let client : Client;
    if let Some(kubeconfig) = deploy_args.value_of("kubeconfig"){
        println!("Using kubeconfig at the following location: {}", kubeconfig);
        client = k8s::from_kubeconfig(Path::new(kubeconfig));
    } else{
        client = k8s::try_default();
    }
    k8s::deploy_h2o(client, deploy_args.value_of("namespace").unwrap_or("default"));
}