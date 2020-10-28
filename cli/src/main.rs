extern crate clap;
extern crate deployment;
extern crate tokio;


use std::fs::File;
use std::io::Write;
use std::path::Path;

use atty::Stream;
use kube::Client;

use cli::{Command, UserDeploymentSpecification};
use deployment::{Deployment, DeploymentSpecification};

mod cli;

#[tokio::main]
async fn main() {
    let command: Command = match cli::get_command() {
        Ok(cmd) => { cmd }
        Err(error) => {
            eprintln!("Unable to process user input: {:?}", error);
            std::process::exit(1);
        }
    };
    match command {
        Command::Deployment(deployment) => {
            deploy(deployment).await;
        }
        Command::Undeploy(deployment_path) => {
            undeploy(deployment_path.as_path())
        }
        Command::Ingress(deployment_path) => {
            ingress(&deployment_path);
        }
    };
}

async fn deploy(user_deployment_spec: UserDeploymentSpecification) {
    let (client, namespace): (Client, String) = if let Some(kubeconfig) = &user_deployment_spec.kubeconfig_path {
        deployment::from_kubeconfig(kubeconfig.as_path())
    } else {
        match deployment::try_default() {
            Ok(client_namespace) => {
                client_namespace
            }
            Err(_) => { panic!("No kubeconfig provided by the user and search in well-known kubeconfig locations failed") }
        }
    };

    let deployment_spec: DeploymentSpecification = DeploymentSpecification::new(user_deployment_spec.name, namespace, user_deployment_spec.memory_percentage, user_deployment_spec.memory, user_deployment_spec.num_cpu, user_deployment_spec.num_h2o_nodes,
                                                                                user_deployment_spec.kubeconfig_path);
        let deployment: Deployment = match deployment::deploy_h2o_cluster(client.clone(), deployment_spec).await {
            Ok(successful_deployment) => { successful_deployment }
            Err(error) => {
                panic!("Unable to deploy H2O cluster. Error:\n{}", error);
            }
        };
    let persisted_filename = persist_deployment(&deployment, false).unwrap();

    if running_on_terminal() {
        println!("Deployment of '{}' completed successfully.", deployment.specification.name);
        println!("To undeploy, use the 'h2ok undeploy -f {}' command.", persisted_filename);
    } else {
        // If not running on a terminal, print only the deployment name.
        print!("{}.h2ok", deployment.specification.name);
    }
}

///Persists a Deployment into current workdir. Name of the resulting file is the name of the deployment name followed by `.h2ok` suffix.
fn persist_deployment(deployment: &Deployment, overwrite: bool) -> Result<String, std::io::Error> {
    let mut file_name = format!("{}.h2ok", deployment.specification.name);
    let mut path: &Path = Path::new(file_name.as_str());
    let mut duplicate_deployment_count: i64 = 0;

    if path.exists() {
        if overwrite {
            match std::fs::remove_file(path) {
                Ok(_) => {}
                Err(e) => { panic!("Unable to remove existing deployment file. Reason: \n{}", e) }
            }
        } else {
            while path.exists() {
                println!("Writing file");
                duplicate_deployment_count += 1;
                file_name = format!("{}({}).h2ok", deployment.specification.name, duplicate_deployment_count);
                path = Path::new(file_name.as_str());
            }
        }
    }
    let mut file: File = match File::create(path) {
        Ok(file) => { file }
        Err(err) => {
            println!("Unable to write deployment file '{}' - skipping. Reason: {}", path.to_str().unwrap(), err);
            return Err(err);
        }
    };
    file.write_all(serde_json::to_string(deployment).unwrap().as_bytes()).unwrap();
    return Ok(String::from(path.to_str().unwrap()));
}

fn undeploy(deployment_descriptor: &Path) {
    let (deployment, client): (Deployment, Client) = extract_existing_deployment(deployment_descriptor);
    match deployment::undeploy_h2o(&client, &deployment) {
        Ok(_) => {}
        Err(deployment_errs) => {
            for undeployed in deployment_errs.iter() {
                print!("Unable to undeploy '{}' - skipping.", undeployed)
            }
        }
    }
    println!("Removed deployment '{}'.", deployment.specification.name);
    std::fs::remove_file(deployment_descriptor).unwrap();
}

fn ingress(deployment_descriptor: &Path) {
    let (mut deployment, client): (Deployment, Client) = extract_existing_deployment(deployment_descriptor);

    match deployment::deploy_ingress(&client, &mut deployment) {
        Ok(_) => {
            let deployment_file_name: String = persist_deployment(&deployment, true).unwrap();
            if running_on_terminal() {
                println!("Ingress '{}' deployed successfully.", &deployment.specification.name);
                let ingress_ip: Option<String> = deployment::ingress::any_ip(deployment.ingresses.last().unwrap());
                let ingress_path: Option<String> = deployment::ingress::any_path(deployment.ingresses.last().unwrap());

                if ingress_ip.is_some() && ingress_path.is_some() {
                    println!("You may now use 'h2o.connect()' to connect to the H2O cluster:");
                    println!("Python: 'h2o.connect(url=\"http://{}:80{}\")'", ingress_ip.as_ref().unwrap(), ingress_path.as_ref().unwrap());
                    println!("R: 'h2o.connect(ip = \"{}\", context_path = \"{}\", port=80)'", ingress_ip.as_ref().unwrap(), ingress_path.unwrap().strip_prefix("/").unwrap())
                }
            } else {
                print!("{}", deployment_file_name);
            }
        }
        Err(e) => {
            panic!("Unable to create ingress for {} deployment. Reason: \n{}", &deployment.specification.name, e);
        }
    }
}

/// Extracts a deployment descriptor and a Client from a deployment descriptor file.
/// It is assumed the caller has verified the given file exists - panics otherwise.
/// If there is no Client described in the `deployment_descriptor`, it is assumed the one from the
/// environment as defined by `KUBECONFIG` environment variable or some well-known places should be used,
/// as such a kubeconfig was used to create the original deployment described in the file.
fn extract_existing_deployment(deployment_descriptor: &Path) -> (Deployment, Client) {
    let deployment_file = File::open(deployment_descriptor).unwrap();
    let deployment: Deployment = serde_json::from_reader(deployment_file).unwrap();

    // Attempt to use the very same kubeconfig to undeploy as was used to deploy
    let (client, _): (Client, String) = match &deployment.specification.kubeconfig_path {
        None => {
            // No kubeconfig specified means the one from the environment should be used.
            deployment::try_default().unwrap()
        }
        Some(kubeconfig_path) => {
            deployment::from_kubeconfig(kubeconfig_path)
        }
    };

    return (deployment, client);
}

/// Returns true if the CLI has been invoked from a TTY, otherwise false.
fn running_on_terminal() -> bool {
    atty::is(Stream::Stdout)
}