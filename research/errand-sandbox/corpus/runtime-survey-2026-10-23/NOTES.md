# Raw notes index (pass 2, 2026-10-23)
- s/*.json + searches1-3.txt : 57 distinct web searches (GET and POST; rank=reranked and rank=provider)
- gh/q_*.json : 8 GitHub repo searches (topic:sandbox, container-runtime, oci-runtime, system-containers, microvm, agent-sandbox ...)
- gh/*.json, gh/*.rel.json : repo metadata + latest release per candidate (46 repos)
- rd/*.md : 86 pages read via read.web.pkgforge.dev (READMEs, docs, issues, PRs)
- gh/iss_*.json + sysbox_issues.json : 6 issue lists (state=all, per_page=50); ~30 issue searches printed inline
Key facts: Incus 7.0 (PR 3235 merged 2026-04-27), LXC 7.0 (PR 4671), systemd v258, Podman v6.0.0 removed cgroup v1; Docker 29 deprecates v1 (supported to >= May 2029).
PVM: virt-pvm/linux not mainline (AgentENV maintainer, 2026-07); pvm.c needs FSGSBASE, RDTSCP, CX16, no host KPTI; `nested_pvm` string not found in virt-pvm pvm.c.
