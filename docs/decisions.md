# Decisions

This page lists the rulings of the operator (the owner of the project) and the decisions of the planning session. The newest row is first in each section.

- Do not change a ruling without the operator.
- When a ruling changes, edit its row and move the old text to "Superseded". Do not keep two versions as current.
- A planning decision holds until the operator overrides it. The override is a new row in "Product", and the old planning row moves to "Superseded".
- An entry of category `decision` adds its row here when it is done.
- [architecture.md](architecture.md) gives the design that follows from these rows. When the two pages disagree, fix the defect in the same change.

The ids D1 to D14 and P1 to P4 come from the planning brief of 2026-10-14. P5 and P6 come from the main session of the same day. Other documents cite a row by its id.

## Product

| Date | Decision | Entries |
| --- | --- | --- |
| 2026-10-14 | **The planning session prepares only the dev tooling (xtask), the scripts, the documents and the plan** (D14). Product implementation is the work of later sessions. No product crate exists, and T-033 creates them first. No reason is recorded. | T-001 to T-012, T-033 |
| 2026-10-14 | **MXC (microsoft/mxc) and LiteBox (microsoft/litebox) are not engines** (D13). MXC is a source of design patterns only. Reason: MXC has no persistent Linux-guest backend with its own kernel, and LiteBox is not a security boundary on userland platforms. | none; [research/engines-2026-10.md](research/engines-2026-10.md) |
| 2026-10-14 | **Linux engine: Firecracker v1.17.0 with its jailer, set up for persistent instances** (D12). Reason: Firecracker has the smallest device model. File permissions control access to its hybrid-vsock Unix sockets. The adversarial review INFERRED that with in-kernel vhost-vsock, any local process can open AF_VSOCK to a guest. | T-015, T-080 to T-094 |
| 2026-10-14 | **Windows engine: native Hyper-V through the Host Compute Service (computecore.dll), driven by a privileged helper service** (D11). Reason: only the Hyper-V stack (HCS, or VMMS through WMI) has Microsoft run the VMM in a hardened per-instance worker. Of the two, HCS boots the kernel directly and needs no registry entries. The operator first asked for one engine native on both OSes. The sweep found four (crosvm, OpenVMM, libkrun, QEMU), and each one weakens isolation. So the split by OS stands. | T-014, T-100 to T-111 |
| 2026-10-14 | **The whole result, the rewritten plan and the Rust tooling, is one squashed commit on main, pushed once** (D10). The operator authorized the move of the repository from Llaberry/agent-box to github.com/Azathothas/agent-box. No reason is recorded. | T-007 |
| 2026-10-14 | **The references branch is dropped after one relevance pass** (D9). A pins table and `cargo xtask refs fetch` replace it. Reason: the findings that still apply move onto main as text with full upstream citations. A clone at the pinned commit checks them again. | T-011, T-013 |
| 2026-10-14 | **The chat gateway, the web UI, and the provider-slot pool and queue stay in the plan as late milestones** (D8). In the chat flow, a Discord message starts a session, the agent works, and the instance is deleted. Reason: the parity ruling of 2026-09-15 still holds. | T-220 to T-226, T-240 to T-242, T-250 to T-255 |
| 2026-10-14 | **Agents run unattended by default** (D7). The operator can attach a terminal when they want. No reason is recorded. | T-160, T-171, T-172 |
| 2026-10-14 | **The guest base OS is Debian 13 slim, with glibc** (D6). No reason is recorded. | T-062, T-064 |
| 2026-10-14 | **The guest has a virtual NIC** (D5). Its packets go over vsock (hvsocket on Windows) to a userspace network stack in the host daemon. Policy applies per flow. DNS, ssh and any TCP or UDP work in the guest. Inbound port forwards expose guest dev servers on host loopback. Reason: with a proxy only, every protocol that the broker does not terminate is unavailable. | T-045, T-120 to T-127 |
| 2026-10-14 | **Project files move only through a git forge** (D4). Clone and push go through the broker. No filesystem is shared between host and guest. Reason: the host then holds no project bytes, so R-HOST-NO-TOOLS holds by construction. | T-030, T-146 |
| 2026-10-14 | **Admin or root rights are acceptable for a one-time install of a privileged helper** (D3). The CLI itself runs unprivileged. Reason: the jailer needs root capabilities, and HCS refuses a caller outside Administrators or Hyper-V Administrators. [status.md](status.md) holds both measurements. | T-081, T-100, T-174 |
| 2026-10-14 | **kage is the harness inside each instance, and agent-box does not write a harness** (D2). Gaps go to the kage maintainer through the operator. Agents write each request into [TODO/upstream.md](../TODO/upstream.md). They never open an issue or a pull request in another repository. | T-046, T-160, T-161, T-169 |
| 2026-10-14 | **The product is one CLI on Windows and Linux hosts that manages persistent microVM instances** (D1). The verbs are create, list, start, pause, resume, stop, delete and attach. Each instance has about 30 GB of storage. Reason: the goals of the operator are isolation and security first, then performance, then footprint. | T-020, T-033, T-170 |
| 2026-09-15 | **Full parity with errand, the project that agent-box replaces, then more**. Securing the agent does not mean removing what it can do. | T-220 to T-226, T-240 to T-242 |
| 2026-09-15 | **agent-box drives an existing harness and owns the protocol boundary**. Today that is kage over ACP. Reason: the harness has no sandbox on purpose, so isolation comes from outside it. | T-046, T-161 |
| 2026-09-15 | **Users of the chat gateway are mutually untrusted**. This applies from the chat milestone. Each user gets a separate instance, separate bindings and nonces, and no shared writable cache. The cost is disk and warm-cache time. | T-186, T-221, T-225 |
| 2026-09-15 | **A logged-in web user drives exactly the sessions that they started**. The operator role is configuration, never a chat-service role. | T-240, T-241 |
| 2026-09-15 | **Provider subscriptions are a small pool with a queue in front**. This is a late milestone. | T-250, T-251, T-252 |
| 2026-09-15 | **Usage shows in three places: a dashboard, the web UI and chat commands**. This is a late milestone. Every number comes from broker accounting, and a view shows a user only their own rows. | T-224, T-242, T-254 |
| 2026-09-15 | **Whether a guest may run containers is measured before it is decided**. A positive result is a pins change, not a setting. | T-185 |
| 2026-09-15 | **The licence is 0BSD**. Reason: each licence condition is one more reason for an agent to decline the reuse of a file. 0BSD is OSI-approved and SPDX-listed. | none; [LICENSE](../LICENSE) |
| 2026-09-15 | **The work model is the todo model**. [TODO/PROGRESS.md](../TODO/PROGRESS.md) holds the only work order, and [TODO/INDEX.md](../TODO/INDEX.md) holds the entries with the counts. Reason: the work is a set of independent items, and the useful question is what matters most. | T-003 |
| 2026-09-15 | **The checks are Rust, in xtask, not shell**. Reason: one implementation runs on every host OS, and two implementations drift. | T-002 to T-012 |

## Planning decisions

The planning session made these decisions on 2026-10-14. The operator may override each one.

| Date | Decision | Entries |
| --- | --- | --- |
| 2026-10-14 | **Unsafe code is `deny` in the workspace, and only listed files allow it** (P6). Each listed file starts with the inner attribute `#![allow(unsafe_code)]`, and each `unsafe` block in it has a comment that says why it is sound. The list: crates/box-transport/src/windows.rs (T-025), crates/box-vmm/src/hcs/computecore.rs (T-102), crates/box-vmm/src/hcs/virtdisk.rs (T-103) and crates/box-helper/src/windows/sys.rs (T-100). A file joins the list only with a change to this row. `cargo xtask check unsafe` holds the list (T-259). Reason: a `forbid` lint cannot be relaxed in one module, and the Windows engine needs FFI. | T-025, T-100, T-102, T-103, T-259 |
| 2026-10-14 | **Pause is an in-memory vCPU freeze, stop is a guest poweroff, and hibernate is a later opt-in verb** (P5). A pause keeps guest RAM on the host and does not survive the loss of the VMM. Hibernate writes the guest state to disk (T-184). Reason: the adversarial review found the meanings of pause and hibernate unconfirmed. Question Q7 in [TODO/PROGRESS.md](../TODO/PROGRESS.md) asks the operator to confirm them. | T-089, T-106, T-170, T-184 |
| 2026-10-14 | **Credentialed traffic goes to explicit plain-HTTP broker endpoints on the guest gateway, not through TLS interception** (P4). Reason: kage trusts only the webpki roots compiled into it, so a planted CA fails. That fact is READ: `QaidVoid/kage@f8df98929f642866932d507b75afac0978e60964:Cargo.lock:4039`. With an explicit endpoint, the guest never names the upstream host. | T-142, T-143, T-149, T-160 |
| 2026-10-14 | **The commit-msg hook stays a self-contained POSIX sh script** (P3). Reason: the hook must work in a fresh clone before any build. `cargo xtask check attribution` reads `git log` and backs it. | T-006, T-008 |
| 2026-10-14 | **The podssh record checker (podssh-todo) is copied into xtask and adapted, not shared** (P2). The drift risk of two copies is an entry of its own. | T-003, T-016 |
| 2026-10-14 | **One async runtime, tokio, in the daemon, the broker, the netstack and box-guest** (P1). The CLI is synchronous, except for its daemon client. Reason: the broker, the netstack and the late surfaces hold many long-lived connections with long idle periods. | T-028, T-029, T-040, T-121, T-142 |

## Superseded

| Date | Decision | Replaced by |
| --- | --- | --- |
| 2026-10-14 | **Egress: the guest has no NIC, and a loopback CONNECT proxy in the guest forwards over vsock to the broker**. The research document of the operator specified this model. The engine evaluation kept the CONNECT proxy. | D5: a virtual NIC, a userspace netstack in the daemon, and policy per flow (T-045, T-120 to T-127). |
| 2026-10-14 | **The broker terminates credentialed TLS with a CA planted in the guest trust store**. The research document specified this. | P4: explicit plain-HTTP broker endpoints. TLS interception stays a later option (T-149). |
| 2026-10-14 | **Open question of the engine evaluation: kage reaches the broker endpoint through a guest loopback forwarder, or through CONNECT to a broker-only name**. | D5 and P4: the netstack DNS resolves every broker name to the gateway (T-122, T-142). |
| 2026-10-14 | **Project files: a per-session workspace image from a seed, with put and get over vsock**. The research document specified this. | D4: files move only through a git forge. The per-instance data disk holds the work, and the host never reads it (T-043, T-087, T-146). |
| 2026-10-14 | **The vsock ports of the research document: 5001 harness stdio, 5002 put and get, 5003 events, 5004 egress**. A finding of the relevance pass also put the nonce on 5003 and the harness stream on 5001. | The v1 ports of [protocol.md](protocol.md): 5000 control, 5001 PTY and exec, 5002 events, 5003 network, 5004 kage bridge (T-022), and 5005 SSH agent (T-257). |
| 2026-09-15 | **The rulings on parity, the harness, untrusted users and the web login named the sessions and the channel of the old plan**. | The same rulings, reworded on 2026-10-14 to fit the pivot, in "Product". |
| 2026-09-15 | **Two or three provider subscriptions, a small number of concurrent sessions each, automatic fallback or manual selection, and a queue**. | A small pool with a queue in front, as a late milestone (T-250, T-251). |
| 2026-09-15 | **The product is a multi-tenant chat bot that confines agents on the shared host kernel**. | D1: one CLI that manages persistent microVM instances. The chat gateway is a late milestone (D8). |
| 2026-09-15 | **bailey and podman are the session backends**. | D11 and D12: HCS on Windows, and Firecracker with the jailer on Linux. The backend entries of the old plan are retired. |
| 2026-09-15 | **The broker is one CONNECT proxy at a link-local address, and it is the only route off the host**. | D5 and P4: the netstack carries all guest traffic, and credentialed traffic goes to explicit broker endpoints. |
| 2026-09-15 | **The corpus is kept on its own `references` branch**. | D9: a pins table in [references.md](references.md) and `cargo xtask refs fetch` (T-011, T-013). |
| 2026-09-15 | **"Upstreaming a patch is not a topic"**. | D2: the operator asks the kage maintainer. Agents write the request into [TODO/upstream.md](../TODO/upstream.md) (T-169). |
| 2026-09-15 | **Async or synchronous: the first recommendation split the work between an async broker and a synchronous rest**. A later revision proposed one async runtime throughout. | P1: one async runtime, tokio, with a synchronous CLI. |
| 2026-09-15 | **"Bootstrap is shell; every check is Rust"**. | `cargo xtask setup` is the bootstrap (T-008). Only the commit-msg hook stays POSIX sh (P3). |
| 2026-09-15 | **Open question: the default of the destination-address check, on or off**. | T-123: the netstack refuses a flow to an address that it did not answer for an allowed name. The check has no off state. |
| 2026-09-15 | **Open question: the extension language, if there is one**. | agent-box has no plugin surface, because kage has plugins. The plugin entries of the old plan are retired. |
| 2026-09-15 | **Open question: the first vendor for a provider sandbox that runs a vendor CLI**. | kage uses provider API keys only (T-250). A request for provider login goes to the kage maintainer (T-169). |
| 2026-09-15 | **Open question: the approval mode of a confined session**. The old recommendation was a prompt for each execution, routed to the chat thread. | T-160: approval mode allow. The instance and the host egress policy are the boundary. |
| 2026-09-15 | **Open question: gVisor or Kata as a backend**. | D1 and D12: the microVM is the product. [research/engines-2026-10.md](research/engines-2026-10.md) gives the reasons that Kata is rejected. |
