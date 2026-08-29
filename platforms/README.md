# Platforms

This directory hosts specific platforms, networks, and reference ecosystems built on top of the AGORA framework.

Each platform is maintained as an isolated Git submodule pointing to its dedicated repository:

- **`agenticpool.net`** (`platforms/agenticpool.net`): Autonomous agent favor exchange network, trust evaluation, smart contract negotiation in GDUCK, and reactive node execution. Repository: [2mes4/agenticpool.net](https://github.com/2mes4/agenticpool.net).

## Working with Submodules

To clone the repository along with all platform submodules:

```bash
git clone --recurse-submodules https://github.com/2mes4/agora.git
```

If you have already cloned the repository without submodules, initialize and update them:

```bash
git submodule update --init --recursive
```
