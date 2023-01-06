# Wasm Shim Versioning in Kubernetes
Wasm workloads in Kubernetes will likely need to lock into a specific version of a Wasm runtime. This proposal is intended to address this problem.


## <a name='TableofContents'></a>Table of Contents

<!-- vscode-markdown-toc -->
* [Table of Contents](#TableofContents)
* [Background](#Background)
* [Problem Statement](#ProblemStatement)
	* [Goals](#Goals)
	* [Non Goals](#NonGoals)
* [Proposal](#Proposal)
	* [Versioning of both shim binary and RuntimeClass name](#VersioningofbothshimbinaryandRuntimeClassname)
	* [Introducing a new version of a shim](#Introducinganewversionofashim)
* [Alternatives](#Alternatives)
	* [Alternative #1](#Alternative1)
		* [Pros](#Pros)
		* [Cons](#Cons)
		* [Conclusion](#Conclusion)
* [Additional Details](#AdditionalDetails)
	* [Test Plan](#TestPlan)
* [Implementation History](#ImplementationHistory)

<!-- vscode-markdown-toc-config
	numbering=false
	autoSave=false
	/vscode-markdown-toc-config -->
<!-- /vscode-markdown-toc -->

## <a name='Background'></a>Background

It is likely behaviors in a shim may change over time as new features are introduced or as existing defects are resolved. Pod workloads that target a version N of a shim may not be able to execute on version N+1. This proposal describes a solution to ensure Pod workloads targeting a specific version of a shim will always be scheduled on a node that contains a safe version of a shim for the workload.

## <a name='ProblemStatement'></a>Problem Statement
The containerd shims in this repository are installed on Kubernetes nodes. The containerd config on each Kubernetes node has a handler registered for each shim, a handler name mapped to binary on the PATH. A RuntimeClass is registered in Kubernetes describing the containerd handler needed to run a Pod workload, as well as label selectors to inform the Kubernetes scheduler to only schedule workloads on particular nodes.

1. A shim binary is installed on a Kubernetes node at `/bin/containerd-shim-spin-v1`.
2. The following shows the containerd config.toml that maps the runtime type to the shim binary from step 1.
      ```toml
        [plugins.cri.containerd.runtimes.spin]
          runtime_type = "io.containerd.spin.v1"
      ```
3. A runtime class that contains a handler that matches the "spin" config runtime name from step 2.
      ```yaml
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-spin
      handler: spin
      ```
    
    **NOTE: if no scheduling configuration is specified, the Kubernetes scheduler expects every node is able to run the workload.**

4. A Wasm workload with the specified runtime class name matching the "wasmtime-spin" runtime class from step 3.
      ```yaml
      apiVersion: apps/v1
      kind: Deployment
      metadata:
        name: wasm-spin
      spec:
        replicas: 1
        selector:
          matchLabels:
            app: wasm-spin
        template:
          metadata:
            labels:
              app: wasm-spin
          spec:
            runtimeClassName: wasmtime-spin
            containers:
            - name: spin-hello
              image: ghcr.io/deislabs/containerd-wasm-shims/examples/spin-rust-hello:latest
              command: ["/"]
      ```

The above configuration will work well if there are no breaking changes introduced to the shim binary or if no new features are added that a given workload depends upon. However, this scenario is unlikely given the speed of innovation in this space.

To safe introduce new nodes and new versions of shims, we must have a versioning scheme for both the shim binary and the RuntimeClasses.

### <a name='Goals'></a>Goals
1) A user should be able to add new nodes to the cluster with new versions of the shims and not disturb existing workloads.
2) A user should be able to target a major, major + minor, or major+minor+patch semantic version of a shim.

### <a name='NonGoals'></a>Non Goals

## <a name='Proposal'></a>Proposal
Now that we have a better understanding of the problem, let's evaluate a solution proposal and possible alternatives.

### <a name='VersioningofbothshimbinaryandRuntimeClassname'></a>Versioning of both shim binary and RuntimeClass name
We will introduce both a version for the shim binary and a version in the RuntimeClass name. The subsequent structure should provide an illustration of the proposed solution.

1. A shim binary is installed on a Kubernetes node at `/bin/containerd-shim-spin-v1.2.3-v1`. **Note:** the semantic version was added to the file name for the bin.
2. The following shows the containerd config.toml that maps the runtime type to the shim binary from step 1.
      ```toml
        [plugins.cri.containerd.runtimes.spin-v1]
          runtime_type = "io.containerd.spin-v1.2.3.v1"
        [plugins.cri.containerd.runtimes.spin-v1_2]
          runtime_type = "io.containerd.spin-v1.2.3.v1"
        [plugins.cri.containerd.runtimes.spin-v1_2_3]
          runtime_type = "io.containerd.spin-v1.2.3.v1"
      ```
3. A runtime class that contains a handler that matches the "spin" config runtime name from step 2.
      ```yaml
      ---
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-spin-v1.2.3
      handler: spin-v1_2_3
      scheduling:
        nodeSelector:
          wasmtime-spin-v1.2.3: "true"
      ---
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-spin-v1.2
      handler: spin-v1_2
      scheduling:
        nodeSelector:
          wasmtime-spin-v1.2: "true"
      ---
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-spin-v1
      handler: spin-v1
      scheduling:
        nodeSelector:
          wasmtime-spin-v1: "true"
      ```

    **NOTE: node labels would be expected for both `wasmtime-spin-v1: "true"` and `wasmtime-spin-v1.2.3: "true"` on nodes that have the spin shim v1.2.3 installed.**

4. A Wasm workload with the specified RuntimeClass name matching the "wasmtime-spin-v1" runtime class from step 3. The "wasmtime-spin-v1" runtimeClassName would map to the spin-v1.2.3 handler.
      ```yaml
      apiVersion: apps/v1
      kind: Deployment
      metadata:
        name: wasm-spin
      spec:
        replicas: 1
        selector:
          matchLabels:
            app: wasm-spin
        template:
          metadata:
            labels:
              app: wasm-spin
          spec:
            runtimeClassName: wasmtime-spin-v1
            containers:
            - name: spin-hello
              image: ghcr.io/deislabs/containerd-wasm-shims/examples/spin-rust-hello:latest
              command: ["/"]
      ```

### <a name='Introducinganewversionofashim'></a>Introducing a new version of a shim
Now that we have described the versioning scheme, let's describe what happens when we introduce a new node that contains a new version of a shim. In this example, we will introduce v1.2.4 of the Spin shim.

For this example, let's imagine we have a cluster with 1 node configured as described in [Versioning of both shim binary and RuntimeClass name](#versioning-of-both-shim-binary-and-runtimeclass-name). We introduce a new node to the cluster with `/bin/containerd-shim-spin-v1.2.4-v1` installed and the following containerd config section.

```toml
[plugins.cri.containerd.runtimes.spin-v1]
runtime_type = "io.containerd.spin-v1.2.4.v1"
[plugins.cri.containerd.runtimes.spin-v1_2]
runtime_type = "io.containerd.spin-v1.2.4.v1"
[plugins.cri.containerd.runtimes.spin-v1_2_4]
runtime_type = "io.containerd.spin-v1.2.4.v1"
```

This new node running v1.2.4 would be decorated with the following node labels, `wasmtime-spin-v1`, `wasmtime-spin-v1.2`, and `wasmtime-spin-v1.2.4`. 

**NOTE:** `wasmtime-spin-v1` and `wasmtime-spin-v1.2` node labels are shared between the node running v1.2.3 and the node running v1.2.4. This means the RuntimeClasses of the same names will match scheduling labels on both the new node and the old node.

To be able to schedule workloads specifically for `wasmtime-spin-v1.2.4`, a new RuntimeClass for that specific version will need to be added to the cluster.

## <a name='Alternatives'></a>Alternatives

### <a name='Alternative1'></a>Only offer major semantic versions for shim handlers

#### <a name='Pros'></a>Pros
- Versioning is much simpler. A new handler / RuntimeClass is introduced only with major semantic version.

#### <a name='Cons'></a>Cons
- In an ecosystem that is changing quickly, it will be more difficult for users to ensure their workloads are stable.
- Shims would need to increment major versions each time a new feature is added, so that workloads would be able to ensure they are scheduled on the appropriate nodes.

#### <a name='Conclusion'></a>Conclusion
To ensure selectivity of workloads and nodes while providing a path for graceful upgrade of shims through the introduction of new nodes to a cluster, I believe the versioning strategy proposed offering differing levels of selectivity by RuntimeClass name and semantic version is the best balance of behavior and complexity.

## <a name='AdditionalDetails'></a>Additional Details
None.

### <a name='TestPlan'></a>Test Plan
- Multiple nodes with differing shim semantic versions
  - Build K3d cluster with 3 nodes; 1 node (A) with vX.Y.Z of a shim and associated node labels, 1 node (B) with vX.Y.Z+1 and associated node labels, and 1 node (C) with both vX.Y.Z and vX.Y.Z+1 and associated node labels
  - Apply the RuntimeClasses with the appropriate node labels
  - Create a deployment of 3 or more pods that targets vX.Y.Z and has some anti-affinity for itself. Verify the pods are running only on nodes A and C.
  - Create a deployment of 3 or more pods that targets vX.Y.Z+1 and has some anti-affinity for itself. Verify the pods are running only on nodes B and C.
  - Create a deployment of 3 or more pods that targets vX.Y and has some anti-affinity for itself. Verify the pods are running on all nodes.
  - Create a deployment of 1 pod that targets vX+1. Verify the pod is not scheduled.

## <a name='ImplementationHistory'></a>Implementation History

- 2022/09/14: Initial proposal
