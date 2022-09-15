# Wasm Shim Versioning in Kubernetes
Wasm workloads in Kubernetes will likely need to lock into a major.minor version of a Wasm shim runtime. This proposal is intended to address this problem.


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

It is likely behaviors in a shim may change over time as new features are introduced or as existing defects are resolved. Pod workloads that target a version N of a shim may not be able to execute on version N-1, there may be features in N not available in N-1 (forward-compatability). Also, with as fast moving as the Wasm runtime and tooling ecosystem is, there may be a period of time when a workload that targets N will not be able to execute on N+1, a major breaking change occurred between N and N+1 (backward-compatability). This proposal describes a solution to ensure Pod workloads targeting a specific version of a shim will always be scheduled on a node that contains a safe version of a shim for the workload.

## <a name='ProblemStatement'></a>Problem Statement
The containerd shims in this repository are installed on Kubernetes nodes. The containerd config on each Kubernetes node has a handler registered for each shim, a handler name mapped to binary on the PATH. A RuntimeClass is registered in Kubernetes describing the containerd handler needed to run a Pod workload, as well as label selectors to inform the Kubernetes scheduler to only schedule workloads on particular nodes.

1. A shim binary is installed on a Kubernetes node at, for example:
    ```
        /bin/containerd-shim-slight-v1
    ```
2. The following shows the containerd config.toml that maps the runtime type to the shim binary from step 1.
      ```toml
        [plugins.cri.containerd.runtimes.slight]
          runtime_type = "io.containerd.slight.v1"
      ```
3. A runtime class that contains a handler that matches the "slight" config runtime name from step 2.
      ```yaml
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-slight
      handler: slight
      ```
    
    **NOTE: if no scheduling configuration is specified, the Kubernetes scheduler assumes every node is able to run the workload.**

4. A Wasm workload with the specified runtime class name matching the "wasmtime-slight" runtime class from step 3.
      ```yaml
      apiVersion: apps/v1
      kind: Deployment
      metadata:
        name: wasm-slight
      spec:
        replicas: 1
        selector:
          matchLabels:
            app: wasm-slight
        template:
          metadata:
            labels:
              app: wasm-slight
          spec:
            runtimeClassName: wasmtime-slight
            containers:
            - name: slight-hello
              image: ghcr.io/deislabs/containerd-wasm-shims/examples/slight-rust-hello:latest
              command: ["/"]
      ```

The above configuration will work well if there are no breaking changes introduced to the shim binary or if no new features are added that a given workload depends upon. However, this scenario is unlikely given the speed of innovation in this space.

To safely introduce new nodes and new versions of shims, we must have a versioning scheme for both the shim binary and the RuntimeClasses.

### <a name='Goals'></a>Goals
1) A user should be able to add new nodes to the cluster with new versions of the shims and not disturb existing workloads.
2) A user should be able to target a workload to nodes that have the shim installed.
3) A user should be able to target a workload to nodes that have a specific version of the shim installed.

### <a name='NonGoals'></a>Non Goals

## <a name='Proposal'></a>Proposal
Now that we have a better understanding of the problem, let's evaluate a solution proposal and possible alternatives.

### <a name='VersioningofbothshimbinaryandRuntimeClassname'></a>Versioning of both shim binary and RuntimeClass name
We will introduce both a version for the shim binary and a version in the RuntimeClass name. The subsequent structure should provide an illustration of the proposed solution.

1. A shim binary is installed on a Kubernetes node at `/bin/containerd-shim-slight-v1-2-3-v1`. **Note:** the semantic version was added to the file name for the bin.
2. The following shows the containerd config.toml that maps the runtime type to the shim binary from step 1.
    ```toml
    [plugins.cri.containerd.runtimes.slight-v1-2-3]
      runtime_type = "io.containerd.slight-v1-2-3.v1"         # must use "-" rather than "."
    [plugins.cri.containerd.runtimes.slight-v1-0-0-beta1]
      runtime_type = "io.containerd.slight-v1-0-0-beta1.v1"   # must use "-" rather than "."
    [plugins.cri.containerd.runtimes.slight-v0-1-0-alpha1]
      runtime_type = "io.containerd.slight-v0-1-0-alpha1.v1"   # must use "-" rather than "."
    [plugins.cri.containerd.runtimes.slight-v0-2-0-alpha1]
      runtime_type = "io.containerd.slight-v0-2-0-alpha1.v1"   # must use "-" rather than "."
    ```
3. The following shows example runtime classes that map handler names to the config runtime names from step 2. It also describes 3 types of nodeSelectors which can be used to provide differing levels of workload selectivity, specific version, major version, and presence of shim regardless of version.
      ```yaml
      # This runtime class maps "plugins.cri.containerd.runtimes.slight-v0-1-0-alpha1" to
      # runtime_type = "io.containerd.slight-v0-1-0-alpha1.v1".
      # 
      # This node selector is intended to select only nodes that have the slight v0.1.0-alpha
      # version of the slight shim. This is critical to enable a workload to target a specific
      # shim version during nascent stages of shim development when forward and / or backward
      # compatibility can not be guaranteed.
      ---
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-slight-v0-1-0-alpha1
      handler: slight-v0-1-0-alpha1
      scheduling:
        nodeSelector:
          wasmtime-slight-v0-1-0-alpha1: "true"
      ---
      # This runtime class maps "plugins.cri.containerd.runtimes.slight-v1-2-3" to
      # runtime_type = "io.containerd.slight-v1-2-3.v1".
      # 
      # This node selector is intended to select any node that has a slight shim enabled
      # within the v1.2 major+minor version. As new nodes are introduced with newer versions in the
      # v1.2 series (e.g. v1.2.4, v1.2.6, etc) a workload could specify a runtime class of 
      # "wasmtime-slight-v1-2" and run on any node with a v1.2 series shim version installed.
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-slight-v1-2
      handler: slight-v1-2-3
      scheduling:
        nodeSelector:
          wasmtime-slight-v1-2: "true"
      ---
      # This runtime class maps "plugins.cri.containerd.runtimes.slight-v1-2-3" to
      # runtime_type = "io.containerd.slight-v1-2-3.v1".
      # 
      # This node selector is intended to select any node that has a slight shim enabled
      # within the v1 major version. As new nodes are introduced with newer versions in the
      # v1 series (e.g. v1.3.4, v1.5.6, etc) a workload could specify a runtime class of 
      # "wasmtime-slight-v1" and run on any node with a v1 series shim version installed.
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-slight-v1
      handler: slight-v1-2-3
      scheduling:
        nodeSelector:
          wasmtime-slight-v1: "true"
      ---
      # This runtime class maps "plugins.cri.containerd.runtimes.slight-v1" to
      # runtime_type = "io.containerd.slight-v1-2-3.v1".
      # 
      # This node selector is intended select any node that has the slight shim enabled
      # regardless of the version of the shim. As new versions are introduced, this
      # node selector would allow for new versions of the shim to be introduced without
      # having to update the pod workload to target a new shim version.
      apiVersion: node.k8s.io/v1
      kind: RuntimeClass
      metadata:
        name: wasmtime-slight
      handler: slight-v1-2-3      # Note: slight-v1-2-3 is the latest available shim version
      scheduling:
        nodeSelector:
          wasmtime-slight-enabled: "true"
      ```

    **NOTE: node labels would be expected for `wasmtime-slight-enabled: true`, `wasmtime-slight-v1: "true"`, `wasmtime-slight-v1-2: true` and `wasmtime-slight-v1-2-3: "true"` on nodes that have the slight shim v1.2.3 installed.**

4. A Wasm workload with the specified RuntimeClass name matching the "wasmtime-slight-v1" runtime class from step 3. The "wasmtime-slight-v1" runtimeClassName would map to the slight v1.2.3 handler.
      ```yaml
      apiVersion: apps/v1
      kind: Deployment
      metadata:
        name: wasm-slight
      spec:
        replicas: 1
        selector:
          matchLabels:
            app: wasm-slight
        template:
          metadata:
            labels:
              app: wasm-slight
          spec:
            runtimeClassName: wasmtime-slight-v1
            containers:
            - name: slight-hello
              image: ghcr.io/deislabs/containerd-wasm-shims/examples/slight-rust-hello:latest
              command: ["/"]
      ```

### <a name='Introducinganewversionofashim'></a>Introducing a new version of a shim
Now that we have described the versioning scheme, let's describe what happens when we introduce a new node that contains a new version of a shim. In this example, we will introduce v1.2.4 of the slight shim.

For this example, let's imagine we have a cluster with 1 node configured as described in [Versioning of both shim binary and RuntimeClass name](#versioning-of-both-shim-binary-and-runtimeclass-name). We introduce a new node to the cluster with `/bin/containerd-shim-slight-v1-2-4-v1` installed and the following containerd config section.

```toml
[plugins.cri.containerd.runtimes.slight-v1-2-4]
    runtime_type = "io.containerd.slight-v1-2-4.v1" 
```

This new node running v1.2.4 would be decorated with the following node labels, `wasmtime-slight-v1: true`, `wasmtime-slight-v1-2: true`, `wasmtime-slight-enabled: true`, and `wasmtime-slight-v1.2.4: true`. The addition of the `wasmtime-slight-v1.2.4: true` label would also require a new runtime class to be introduced to map workloads to that label.

**NOTE:** `wasmtime-slight-v1` and `wasmtime-slight-v1-2` node label is shared between the node running v1.2.3 and the node running v1.2.4. This means the RuntimeClasses of the same names will match scheduling labels on both the new node and the old node.

#### Indicating Stability
Indicating shim stability / maturity during nascent stages of the shim and Wasm ecosystem will be critical to adoption. Versions of shims which may have breaking changes should be indicated with an alpha or beta addition to their version, or have a major version in the v0 series (e.g. v0.1.0, v0.9.8, etc).

#### Version Deprecation
A cluster operator should provide guidance on version deprecation. Upon introducing a new version of a shim to a cluster, the operator should provide at least the new version and the previous version (N and N-1) to ensure a shim dependent workload can gracefully upgrade.

## <a name='Alternatives'></a>Alternatives

### <a name='Alternative1'></a>Only offer major semantic versions for shim handlers

#### <a name='Pros'></a>Pros
- This versioning strategy is simple, but provides low version selectivity.
- No indication of alpha, beta, or v0 versions of shims provides an indication of possible breaking changes between versions.

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
- 2023/03/07: Updated based on feedback from initial PR
