use std::{net::TcpListener, time::Duration};

use anyhow::Result;
use curl::easy::Easy;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::ListParams, config::KubeConfigOptions, Api, Client, Config, ResourceExt};
use rand::Rng;
use tokio::process::Command;

async fn which_binary(bianry_name: &str) -> Result<()> {
    println!(" >>> which {}", bianry_name);
    let mut cmd = Command::new("which");
    cmd.arg(bianry_name);
    let output = cmd.output().await;
    if output.is_err() {
        anyhow::bail!(format!("{} not found in PATH", bianry_name));
    }
    let output = output.unwrap();
    if !output.status.success() {
        anyhow::bail!(format!("{} not found in PATH", bianry_name));
    }
    Ok(())
}

pub async fn setup_test(test_ns: &str) -> Result<u16> {
    let res = setup_test_helper(test_ns).await;
    if res.is_err() {
        println!(" >>> setup test failed");
        teardown_test("my-test").await?;
        return res;
    }
    res
}

async fn setup_test_helper(test_ns: &str) -> Result<u16> {
    which_binary("k3d").await?;
    which_binary("cross").await?;
    which_binary("docker").await?;
    which_binary("kubectl").await?;

    let dockerfile_path = "deployments/k3d";
    let bin_path = "deployments/k3d/.tmp/";
    let slight_shim_path = "deployments/k3d/.tmp/containerd-shim-slight-v1";
    let spin_shim_path = "deployments/k3d/.tmp/containerd-shim-spin-v1";

    if which_binary(slight_shim_path).await.is_err() {
        println!(" >>> install containerd-shim-slight-v1");
        let mut cmd = Command::new("cross");
        cmd.arg("build")
            .arg("--target")
            .arg("x86_64-unknown-linux-musl")
            .arg("--release")
            .arg("--manifest-path")
            .arg("containerd-shim-slight-v1/Cargo.toml");
        let output = cmd.output().await?;
        if !output.status.success() {
            anyhow::bail!("failed to build containerd-shim-slight-v1");
        }
        let mut cmd = Command::new("sudo");
        cmd.arg("install")
            .arg("containerd-shim-slight-v1/target/x86_64-unknown-linux-musl/release/containerd-shim-slight-v1")
            .arg(bin_path);
        let output = cmd.output().await?;
        if !output.status.success() {
            anyhow::bail!("failed to install containerd-shim-slight-v1");
        }
    }

    if which_binary(spin_shim_path).await.is_err() {
        println!(" >>> install containerd-shim-spin-v1");
        let mut cmd = Command::new("cross");
        cmd.arg("build")
            .arg("--target")
            .arg("x86_64-unknown-linux-musl")
            .arg("--release")
            .arg("--manifest-path")
            .arg("containerd-shim-spin-v1/Cargo.toml");
        let output = cmd.output().await?;
        if !output.status.success() {
            anyhow::bail!("failed to build containerd-shim-spin-v1");
        }
        let mut cmd = Command::new("sudo");
        cmd.arg("install")
            .arg("containerd-shim-spin-v1/target/x86_64-unknown-linux-musl/release/containerd-shim-spin-v1")
            .arg(bin_path);
        let output = cmd.output().await?;
        if !output.status.success() {
            anyhow::bail!("failed to install containerd-shim-spin-v1");
        }
    }

    // build docker image
    let mut cmd = Command::new("docker");
    cmd.arg("build").arg("-t").arg(test_ns).arg(dockerfile_path);
    let output = cmd.output().await?;
    if !output.status.success() {
        // print out the error message to stderr
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        // print out the stdout
        println!("{}", String::from_utf8_lossy(&output.stdout));
        anyhow::bail!(format!("failed to build docker image {}", test_ns));
    }

    // create k3d cluster
    let cluster_name = format!("{}-cluster", test_ns);
    let image_name = test_ns;
    let _context_name = format!("k3d-{}", cluster_name);

    let host_port = get_available_port().expect("failed to get available port");
    // k3d cluster create $(CLUSTER_NAME) --image $(IMAGE_NAME) --api-port 6550 -p "8081:80@loadbalancer" --agents 1
    let mut cmd = Command::new("k3d");
    cmd.arg("cluster")
        .arg("create")
        .arg(&cluster_name)
        .arg("--image")
        .arg(&image_name)
        .arg("-p")
        .arg(format!("{}:80@loadbalancer", host_port))
        .arg("--agents")
        .arg("1");
    let output = cmd.output().await?;
    if !output.status.success() {
        // print out the error message to stderr
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        anyhow::bail!(format!("failed to create k3d cluster {}", cluster_name));
    }

    Ok(host_port)
}

pub async fn teardown_test(test_ns: &str) -> Result<()> {
    let cluster_name = format!("{}-cluster", test_ns);

    // delete docker image
    let mut cmd = Command::new("docker");
    cmd.arg("rmi").arg(test_ns);
    let output = cmd.output().await?;
    if !output.status.success() {
        // print out the error message to stderr
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        // print out the stdout
        println!("{}", String::from_utf8_lossy(&output.stdout));
        anyhow::bail!(format!("failed to delete docker image {}", test_ns));
    }

    // check docker image is deleted
    let mut cmd = Command::new("docker");
    cmd.arg("image").arg("inspect").arg(test_ns);
    let output = cmd.output().await?;
    if output.status.success() {
        anyhow::bail!(format!("failed to delete docker image {}", test_ns));
    }

    // delete k3d cluster
    let mut cmd = Command::new("k3d");
    cmd.arg("cluster").arg("delete").arg(cluster_name);
    let output = cmd.output().await?;
    if !output.status.success() {
        anyhow::bail!(format!("failed to delete k3d cluster {}", test_ns));
    }
    Ok(())
}

fn port_is_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn get_available_port() -> Option<u16> {
    let mut rng = rand::thread_rng();
    loop {
        let port: u16 = rng.gen_range(1025..65535);
        if port_is_available(port) {
            return Some(port);
        }
    }
}

pub async fn retry_curl(url: &str, retry_times: u32, interval_in_secs: u64) -> Result<()> {
    let mut i = 0;
    let mut handle = Easy::new();
    handle.url(url)?;
    handle
        .write_function(|data| {
            println!("{}", String::from_utf8_lossy(data));
            Ok(data.len())
        })
        .unwrap();

    loop {
        let res = handle.perform();
        if res.is_ok() {
            break;
        }
        i += 1;
        if i == retry_times {
            anyhow::bail!("failed to curl for hello");
        }
        tokio::time::sleep(Duration::from_secs(interval_in_secs)).await;
    }
    Ok(())
}

pub async fn list_pods(cluster_name: &str) -> Result<()> {
    let config = Config::from_kubeconfig(&KubeConfigOptions {
        context: Some(cluster_name.to_string()),
        ..Default::default()
    })
    .await?;

    let client = Client::try_from(config)?;

    let pods: Api<Pod> = Api::default_namespaced(client);
    for p in pods.list(&ListParams::default()).await? {
        println!("found pod {}", p.name_any());
    }
    Ok(())
}

pub async fn k_apply(path: &str) -> Result<()> {
    let mut cmd = Command::new("kubectl");
    cmd.arg("apply").arg("-f").arg(path);
    let output = cmd.output().await?;
    if !output.status.success() {
        // print out the error message to stderr
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        anyhow::bail!(format!("failed to apply {}", path));
    }
    Ok(())
}
