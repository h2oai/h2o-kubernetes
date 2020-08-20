extern crate clap;

use std::fs::{File, remove_file};
use std::io::Write;
use std::path::Path;

use kube::{Client, Error};

use crate::cli::{Command};
use crate::k8s::{Deployment, DeploymentSpecification};

mod cli;
mod k8s;
#[cfg(test)]
mod tests;

fn main() {
    let command = match cli::get_command(){
        Ok(cmd) => { cmd},
        Err(error) => {
            eprintln!("Unable to process user input: {:?}", error);
            std::process::exit(1);
        },
    };
    match command {
        Command::Deployment(deployment) => {
            deploy(deployment);
        }
        Command::Undeploy(deployment_path) => {
            undeploy(deployment_path.as_path())
        }
    };
}

fn deploy(deployment_specification: DeploymentSpecification) {
    let client: Client = if let Some(kubeconfig) = &deployment_specification.kubeconfig_path {
        k8s::from_kubeconfig(kubeconfig.as_path())
    } else {
        let default_client: Result<Client, Error> = k8s::try_default();
        match default_client {
            Ok(cl) => { cl }
            Err(_) => { panic!("No kubeconfig provided by the user and search in well-known kubeconfig locations failed") }
        }
    };

    let deployment: Deployment = match k8s::deploy_h2o_cluster(&client, deployment_specification) {
        Ok(successful_deployment) => { successful_deployment }
        Err(error) => {
            panic!("Unable to deploy H2O cluster. Error:\n{}", error);
        }
    };

    print!("{}.h2ok", deployment.specification.name);
    persist_deployment(&deployment);
}

fn undeploy(deployment_descriptor_path: &Path) {
    let deployment_file = File::open(deployment_descriptor_path).unwrap();
    let deployment: Deployment = serde_json::from_reader(deployment_file).unwrap();

    // Attempt to use the very same kubeconfig to undeploy as was used to deploy
    let client: Client = match &deployment.specification.kubeconfig_path {
        None => {
            // No kubeconfig specified means the one from the environment should be used.
            k8s::try_default().unwrap()
        }
        Some(kubeconfig_path) => {
            k8s::from_kubeconfig(kubeconfig_path)
        }
    };
    match k8s::undeploy_h2o(&client, &deployment) {
        Ok(_) => {}
        Err(deployment_errs) => {
            for undeployed in deployment_errs.iter() {
                print!("Unable to undeploy '{}' - skipping.", undeployed)
            }
        }
    }
    println!("Removed deployment '{}'.", deployment.specification.name);
    remove_file(deployment_descriptor_path).unwrap();
}

fn persist_deployment(deployment: &Deployment) {
    let mut file_name = format!("{}.h2ok", deployment.specification.name);
    let mut path: &Path = Path::new(file_name.as_str());
    let mut duplicate_deployment_count: i64 = 0;
    while path.exists() {
        println!("Writing file");
        duplicate_deployment_count += 1;
        file_name = format!("{}({}).h2ok", deployment.specification.name, duplicate_deployment_count);
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
