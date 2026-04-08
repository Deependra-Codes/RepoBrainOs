# Scripts

Scripts are for operator convenience and repo automation.

Rules:

- scripts must not become the hidden home of business-critical logic
- production logic belongs in `src/`
- scripts should wrap or invoke stable package surfaces
- prefer `cargo xtask ...` for repo-native automation
- bootstrap should install quality tooling and configure git hooks, not silently redefine quality policy

Current scripts:

- [bootstrap.ps1](d:/RepoBrainOS/scripts/bootstrap.ps1)
- [install-alive2-windows.cmd](d:/RepoBrainOS/scripts/install-alive2-windows.cmd)

Primary automation lives in:

- `cargo xtask doctor`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`
- `cargo xtask install-git-hooks`
