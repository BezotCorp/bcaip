# Launch Ecosystem

`launch_ecosystem` contains the shared concepts and rules required to select,
describe, or prepare an environment in which something will be launched.

Its scope is limited to the launch environment itself.

## Belongs here

A concept belongs in `launch_ecosystem` when it is required to select,
describe, or prepare the environment in which something will be launched.

Examples include:

- execution environment selection;
- generic environment resolution rules;
- shared launch-platform concepts if they become necessary.

`ExecutionEnvironment` belongs here because development and production
selection is shared by independent domains such as executables and servers.

## Does not belong here

`launch_ecosystem` does not own:

- executable definitions or executable lifecycle management;
- server definitions or server management;
- deployment orchestration;
- runtime supervision;
- process management;
- application-specific network protocols;
- systemd integration;
- Windows service integration;
- launchd integration;
- operating-system-specific provisioning logic.

Those concepts belong to their respective domains or implementations.

## Boundary

The library contains what is needed to choose, describe, or prepare a launch
environment.

It does not contain the domain model of what is launched, and it does not
manage that thing after launch.

## Current scope

The current public model contains only `ExecutionEnvironment`, with:

- `Development`
- `Production`

The library should grow only when another genuinely shared launch-environment
concept appears. Its name must not make it a generic utility crate.
