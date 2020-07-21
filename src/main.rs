extern crate clap;

use std::fs::{File, remove_file};
use std::io::Write;
use std::path::Path;

use clap::ArgMatches;
use kube::Client;
use names::Generator;

use crate::k8s::Deployment;

mod args;
mod k8s;
#[cfg(test)]
mod tests;

fn main() {
    let args: ArgMatches = args::parse_arguments();
    if let Some(deploy_args) = args.subcommand_matches("deploy") {
        deploy(deploy_args);
    } else if let Some(undeploy_args) = args.subcommand_matches("undeploy") {
        undeploy(undeploy_args);
    }
}

fn deploy(deploy_args: &ArgMatches) {
    let client: Client;
    if let Some(kubeconfig) = deploy_args.value_of("kubeconfig") {
        println!("Using kubeconfig at the following location: {}", kubeconfig);
        client = k8s::from_kubeconfig(Path::new(kubeconfig));
    } else {
        client = k8s::try_default();
    }
    let deployment_name: String = deployment_name(deploy_args);
    let nodes: i32 = deploy_args.value_of("cluster_size").unwrap().parse::<i32>().unwrap();
    let mut deployment: Deployment = k8s::deploy_h2o(&client, deployment_name.as_str(),
                                                     deploy_args.value_of("namespace").unwrap(), nodes);
    if let Some(kubeconfig) = deploy_args.value_of("kubeconfig") {
        deployment.kubeconfig_path = Option::Some(String::from(kubeconfig));
    }
    println!("Finished deployment of '{}' cluster", deployment.name);
    persist_deployment(&deployment);
}

fn deployment_name(deploy_args: &ArgMatches) -> String {
    return match deploy_args.value_of("name") {
        None => {
            let mut generator: Generator = Generator::default();
            format!("h2o-{}", generator.next().unwrap())
        }
        Some(name) => { String::from(name) }
    };
}

fn persist_deployment(deployment: &Deployment) {
    let mut file_name = format!("{}.h2ok", deployment.name);
    let mut path: &Path = Path::new(file_name.as_str());
    let mut duplicate_deployment_count: i64 = 0;
    while path.exists() {
        println!("Writing file");
        duplicate_deployment_count += 1;
        file_name = format!("{}({}).h2ok", deployment.name, duplicate_deployment_count);
        path = Path::new(file_name.as_str());
    }
    let mut file: File = match File::create(path) {
        Ok(file) => { file }
        Err(err) => {
            println!("Unable to write deployment file '{}' - skipping. Reason: {}", path.to_str().unwrap(), err);
            return;
        }
    };
    file.write_all(serde_json::to_string(deployment).unwrap().as_bytes()).unwrap();
}

fn undeploy(undeploy_args: &ArgMatches) {
    let file_path = match undeploy_args.value_of("file") {
        None => { panic!("Deployment file undefined.") }
        Some(file) => { file }
    };
    let deployment_file = File::open(file_path).unwrap();
    let deployment: Deployment = serde_json::from_reader(deployment_file).unwrap();
    let client: Client = k8s::from_kubeconfig(Path::new(deployment.kubeconfig_path.clone().unwrap().as_str()));
    k8s::undeploy_h2o(&client, &deployment).unwrap();
    println!("Removed deployment '{}'.", deployment.name);
    remove_file(file_path).unwrap();
}
