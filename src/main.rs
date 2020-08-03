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
    let kubeconfig_location: Option<String> = kubeconfig_location(deploy_args);
    let client = match &kubeconfig_location {
        None => {
            match k8s::try_default() {
                Ok(client) => {client},
                Err(e) => {panic!("Unable to connect to Kubernetes cluster - no kubeconfig found. Reason:\n{}", e)},
            }
        },
        Some(kubeconfig) => {k8s::from_kubeconfig(Path::new(kubeconfig))},
    };

    let deployment_name: String = deployment_name(deploy_args);
    let namespace: String = String::from(deploy_args.value_of("namespace").unwrap());
    let nodes: u32 = deploy_args.value_of("cluster_size").unwrap().parse::<u32>().unwrap();
    let jvm_memory_percentage: u8 = deploy_args.value_of("memory_percentage").unwrap().parse::<u8>().unwrap();
    let memory: String = memory(deploy_args);
    let num_cpus: u32 = cpus(deploy_args);

    let mut deployment: Deployment = Deployment::new(deployment_name, namespace, kubeconfig_location,
                                                     jvm_memory_percentage, memory, num_cpus, nodes);
    k8s::deploy_h2o_cluster(&client, &mut deployment);

    println!("Finished deployment of '{}' cluster.", deployment.name);
    persist_deployment(&deployment);
}

fn kubeconfig_location(deploy_args: &ArgMatches) -> Option<String> {
    let kubeconfig_location: Option<String> = match deploy_args.value_of("kubeconfig") {
        None => {
            println!("No kubeconfig provided explicitly. Searching well-known locations.");
            k8s::try_openshift_kubeconfig()
        }
        Some(kubeconfig) => {
            println!("Using user-provided kubeconfig at the following location: {}", kubeconfig);
            Some(String::from(kubeconfig))
        }
    };

    return kubeconfig_location;
}

fn cpus(deploy_args: &ArgMatches) -> u32 {
    return match deploy_args.value_of("cpus") {
        None => {
            1
        }
        Some(cpus) => { cpus.parse::<u32>().unwrap() }
    };
}

fn memory(deploy_args: &ArgMatches) -> String {
    return match deploy_args.value_of("memory") {
        None => {
            String::from("4Gi")
        }
        Some(memory) => { String::from(memory) }
    };
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
    match k8s::undeploy_h2o(&client, &deployment) {
        Ok(_) => {}
        Err(deployment_errs) => {
            for undeployed in deployment_errs.iter() {
                println!("Unable to undeploy '{}' - skipping.", undeployed)
            }
        }
    }
    println!("Removed deployment '{}'.", deployment.name);
    remove_file(file_path).unwrap();
}
